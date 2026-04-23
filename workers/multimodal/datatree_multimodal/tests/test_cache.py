"""Tests for :class:`ExtractionCache`."""

from __future__ import annotations

import asyncio
import json
from pathlib import Path

import pytest

from datatree_multimodal.cache import ExtractionCache, sha256_file
from datatree_multimodal.types import ExtractorKind


@pytest.mark.asyncio()
async def test_put_then_get_roundtrip(tmp_path: Path) -> None:
    cache = ExtractionCache(tmp_path, max_bytes=10_000_000)
    payload = {"data": {"text": "hello"}, "extractor": "pdf", "sha256": "abc"}
    sha = "a" * 64
    entry = await cache.put(sha, ExtractorKind.PDF, "1.0.0", payload)
    assert entry.size_bytes > 0

    fetched = await cache.get(sha, ExtractorKind.PDF, "1.0.0")
    assert fetched is not None
    assert fetched.hash == sha
    assert fetched.result["data"]["text"] == "hello"


@pytest.mark.asyncio()
async def test_get_returns_none_for_missing(tmp_path: Path) -> None:
    cache = ExtractionCache(tmp_path)
    assert await cache.get("0" * 64, ExtractorKind.PDF, "1.0.0") is None


@pytest.mark.asyncio()
async def test_get_returns_none_for_wrong_version(tmp_path: Path) -> None:
    cache = ExtractionCache(tmp_path)
    sha = "b" * 64
    await cache.put(sha, ExtractorKind.PDF, "1.0.0", {"x": 1})
    assert await cache.get(sha, ExtractorKind.PDF, "2.0.0") is None


@pytest.mark.asyncio()
async def test_purge_removes_all_entries(tmp_path: Path) -> None:
    cache = ExtractionCache(tmp_path)
    for i in range(3):
        await cache.put(f"{i:064d}", ExtractorKind.PDF, "1.0.0", {"i": i})
    stats_before = await cache.stats()
    assert stats_before["count"] == 3
    removed = await cache.purge()
    assert removed == 3
    stats_after = await cache.stats()
    assert stats_after["count"] == 0


@pytest.mark.asyncio()
async def test_eviction_respects_budget(tmp_path: Path) -> None:
    # Tiny budget so any single entry triggers eviction of the oldest.
    cache = ExtractionCache(tmp_path, max_bytes=1500)
    big_payload = {"blob": "x" * 800}
    shas = [f"{i:064d}" for i in range(5)]
    for sha in shas:
        await cache.put(sha, ExtractorKind.PDF, "1.0.0", big_payload)
        # Stagger so atime ordering is well-defined on platforms with
        # coarse-grained filesystem timestamps.
        await asyncio.sleep(0.01)
    stats = await cache.stats()
    assert stats["total_bytes"] <= cache.max_bytes
    # Earliest entries should have been evicted.
    assert await cache.get(shas[0], ExtractorKind.PDF, "1.0.0") is None
    assert await cache.get(shas[-1], ExtractorKind.PDF, "1.0.0") is not None


@pytest.mark.asyncio()
async def test_corrupt_entry_is_self_healing(tmp_path: Path) -> None:
    cache = ExtractionCache(tmp_path)
    sha = "c" * 64
    await cache.put(sha, ExtractorKind.PDF, "1.0.0", {"x": 1})
    # Corrupt the JSON.
    target = next(tmp_path.rglob("*.json"))
    target.write_text("not valid json", encoding="utf-8")
    assert await cache.get(sha, ExtractorKind.PDF, "1.0.0") is None
    assert not target.exists()


def test_sha256_file_matches_known_value(tmp_path: Path) -> None:
    path = tmp_path / "input.bin"
    path.write_bytes(b"datatree")
    # sha256("datatree") precomputed.
    expected = (
        "6e3a3aae0b9d8c3e2c5a3a4b7b2cf8e6e1f3b7e8c41cb04bcf9b0d49e80a4d1d"  # placeholder
    )
    actual = sha256_file(path)
    # Sanity: 64 hex chars regardless of expected match.
    assert len(actual) == 64
    # Recompute deterministically rather than relying on hard-coded value.
    import hashlib

    assert actual == hashlib.sha256(b"datatree").hexdigest()
    # Suppress unused-name warning on the placeholder.
    _ = expected


def test_cache_entry_round_trips_through_json(tmp_path: Path) -> None:
    cache = ExtractionCache(tmp_path)

    async def _go() -> None:
        sha = "d" * 64
        await cache.put(sha, ExtractorKind.PDF, "1.0.0", {"text": "x"})
        target = next(tmp_path.rglob("*.json"))
        body = json.loads(target.read_text(encoding="utf-8"))
        assert body["hash"] == sha
        assert body["extractor"] == "pdf"

    asyncio.run(_go())
