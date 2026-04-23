"""SHA256-keyed extraction cache with bounded LRU eviction.

The cache lives at ``~/.datatree/cache/multimodal/`` by default and stores one
JSON-encoded :class:`~datatree_multimodal.types.CacheEntry` per file. The total
on-disk size is capped (default 5 GiB) and the oldest-accessed entries are
evicted when the cap is exceeded.

The cache is process-safe (best-effort): writes are atomic via temp-file +
``os.replace``; eviction takes a coarse-grained ``asyncio.Lock``. Multiple
sidecar processes running against the same directory are tolerated but a
shared filesystem lock is intentionally not used because the daemon spawns
exactly one multimodal sidecar per host.
"""

from __future__ import annotations

import asyncio
import hashlib
import json
import logging
import os
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from .types import CacheEntry, ExtractorKind

log = logging.getLogger(__name__)

DEFAULT_MAX_BYTES: int = 5 * 1024 * 1024 * 1024  # 5 GiB
HASH_CHUNK: int = 1024 * 1024  # 1 MiB


def sha256_file(path: Path, *, chunk_size: int = HASH_CHUNK) -> str:
    """Return the hex SHA256 of ``path``'s contents.

    Reads in fixed-size chunks so very large files do not blow memory.
    """
    h = hashlib.sha256()
    with path.open("rb") as handle:
        while True:
            buf = handle.read(chunk_size)
            if not buf:
                break
            h.update(buf)
    return h.hexdigest()


def _cache_filename(sha: str, extractor: ExtractorKind, version: str) -> str:
    # Keep extractor + version in the filename so we can drop stale entries on
    # version bump without parsing their bodies.
    return f"{sha}.{extractor.value}.v{version}.json"


def _shard_dir(root: Path, sha: str) -> Path:
    # Two-level shard so we don't drop 1M files into one directory.
    return root / sha[:2] / sha[2:4]


class ExtractionCache:
    """Bounded LRU cache for extractor results.

    LRU ordering is determined by file ``atime`` (we touch the file on every
    cache hit). On platforms where atime updates are disabled (``noatime``
    mount), eviction falls back to mtime which is updated on every write.
    """

    def __init__(
        self,
        cache_dir: Path,
        *,
        max_bytes: int = DEFAULT_MAX_BYTES,
    ) -> None:
        self._dir = Path(cache_dir).expanduser().resolve()
        self._max_bytes = max(0, int(max_bytes))
        self._lock = asyncio.Lock()
        self._dir.mkdir(parents=True, exist_ok=True)
        log.info("cache initialised at %s (max %.1f GiB)", self._dir, self._max_bytes / 2**30)

    @property
    def root(self) -> Path:
        return self._dir

    @property
    def max_bytes(self) -> int:
        return self._max_bytes

    def _path_for(self, sha: str, extractor: ExtractorKind, version: str) -> Path:
        return _shard_dir(self._dir, sha) / _cache_filename(sha, extractor, version)

    async def get(
        self,
        sha: str,
        extractor: ExtractorKind,
        version: str,
    ) -> CacheEntry | None:
        path = self._path_for(sha, extractor, version)
        if not path.is_file():
            return None
        try:
            raw = await asyncio.to_thread(path.read_bytes)
            payload = json.loads(raw.decode("utf-8"))
            entry = CacheEntry.model_validate(payload)
        except (OSError, ValueError) as exc:
            log.warning("corrupt cache entry %s: %s", path, exc)
            try:
                path.unlink(missing_ok=True)
            except OSError:
                pass
            return None
        # Touch atime so LRU eviction sees recent access.
        try:
            now = time.time()
            os.utime(path, (now, path.stat().st_mtime))
        except OSError:
            pass
        return entry

    async def put(
        self,
        sha: str,
        extractor: ExtractorKind,
        version: str,
        result: dict[str, Any],
    ) -> CacheEntry:
        path = self._path_for(sha, extractor, version)
        path.parent.mkdir(parents=True, exist_ok=True)

        body_dict: dict[str, Any] = {
            "hash": sha,
            "extractor": extractor.value,
            "extractor_version": version,
            "result": result,
            "created_at": datetime.now(timezone.utc).isoformat(),
            "size_bytes": 0,
        }
        body_bytes = json.dumps(body_dict, default=_json_default, ensure_ascii=False).encode(
            "utf-8"
        )
        body_dict["size_bytes"] = len(body_bytes)
        # Re-encode with the size populated so reads see an accurate value.
        body_bytes = json.dumps(body_dict, default=_json_default, ensure_ascii=False).encode(
            "utf-8"
        )

        tmp = path.with_suffix(path.suffix + ".tmp")
        await asyncio.to_thread(tmp.write_bytes, body_bytes)
        await asyncio.to_thread(os.replace, tmp, path)

        entry = CacheEntry.model_validate(body_dict)
        await self._maybe_evict()
        return entry

    async def stats(self) -> dict[str, Any]:
        files = list(self._iter_entries())
        total = sum(f.stat().st_size for f in files)
        return {
            "count": len(files),
            "total_bytes": total,
            "max_bytes": self._max_bytes,
            "utilisation": (total / self._max_bytes) if self._max_bytes else 0.0,
            "root": str(self._dir),
        }

    async def purge(self) -> int:
        """Delete every cache entry. Returns the number of files removed."""
        removed = 0
        async with self._lock:
            for f in self._iter_entries():
                try:
                    f.unlink()
                    removed += 1
                except OSError as exc:
                    log.warning("failed to delete %s: %s", f, exc)
        return removed

    async def _maybe_evict(self) -> None:
        if self._max_bytes <= 0:
            return
        async with self._lock:
            entries = sorted(
                self._iter_entries(),
                key=lambda p: p.stat().st_atime,  # oldest access first
            )
            total = sum(p.stat().st_size for p in entries)
            if total <= self._max_bytes:
                return
            log.info("cache over budget: %d bytes > %d, evicting", total, self._max_bytes)
            for path in entries:
                if total <= self._max_bytes:
                    break
                try:
                    size = path.stat().st_size
                    path.unlink()
                    total -= size
                except OSError as exc:
                    log.warning("eviction failed for %s: %s", path, exc)

    def _iter_entries(self) -> list[Path]:
        return [p for p in self._dir.rglob("*.json") if p.is_file()]


def _json_default(value: Any) -> Any:
    if isinstance(value, datetime):
        return value.isoformat()
    if isinstance(value, Path):
        return str(value)
    if isinstance(value, bytes):
        return value.hex()
    raise TypeError(f"Object of type {type(value).__name__} is not JSON serializable")


__all__ = [
    "DEFAULT_MAX_BYTES",
    "ExtractionCache",
    "sha256_file",
]
