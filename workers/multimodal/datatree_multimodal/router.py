"""Picks the right extractor for a file path and runs it.

The router owns the long-lived extractor instances (notably the Whisper model,
which is expensive to load) and delegates SHA256 computation + cache lookup to
:class:`~datatree_multimodal.cache.ExtractionCache`.
"""

from __future__ import annotations

import asyncio
import logging
from pathlib import Path
from typing import Any

from .cache import ExtractionCache, sha256_file
from .extractors import Extractor
from .extractors.docx import DocxExtractor
from .extractors.image import ImageExtractor
from .extractors.notebook import NotebookExtractor
from .extractors.pdf import PdfExtractor
from .extractors.whisper import WhisperExtractor
from .extractors.xlsx import XlsxExtractor
from .types import ExtractionResult, ExtractorKind

log = logging.getLogger(__name__)

# Map of file extensions (lowercase, with leading dot) → extractor kind.
EXTENSION_MAP: dict[str, ExtractorKind] = {
    ".pdf": ExtractorKind.PDF,
    ".png": ExtractorKind.IMAGE,
    ".jpg": ExtractorKind.IMAGE,
    ".jpeg": ExtractorKind.IMAGE,
    ".bmp": ExtractorKind.IMAGE,
    ".tif": ExtractorKind.IMAGE,
    ".tiff": ExtractorKind.IMAGE,
    ".gif": ExtractorKind.IMAGE,
    ".webp": ExtractorKind.IMAGE,
    ".wav": ExtractorKind.WHISPER,
    ".mp3": ExtractorKind.WHISPER,
    ".m4a": ExtractorKind.WHISPER,
    ".flac": ExtractorKind.WHISPER,
    ".ogg": ExtractorKind.WHISPER,
    ".opus": ExtractorKind.WHISPER,
    ".aac": ExtractorKind.WHISPER,
    ".mp4": ExtractorKind.WHISPER,
    ".m4v": ExtractorKind.WHISPER,
    ".mov": ExtractorKind.WHISPER,
    ".mkv": ExtractorKind.WHISPER,
    ".webm": ExtractorKind.WHISPER,
    ".ipynb": ExtractorKind.NOTEBOOK,
    ".docx": ExtractorKind.DOCX,
    ".xlsx": ExtractorKind.XLSX,
    ".xlsm": ExtractorKind.XLSX,
}


class Router:
    """Resolve files to extractors, manage caching."""

    def __init__(
        self,
        cache: ExtractionCache,
        *,
        whisper_model_path: Path | None = None,
        tesseract_cmd: Path | None = None,
    ) -> None:
        self._cache = cache
        self._extractors: dict[ExtractorKind, Extractor] = {
            ExtractorKind.PDF: PdfExtractor(),
            ExtractorKind.IMAGE: ImageExtractor(tesseract_cmd=tesseract_cmd),
            ExtractorKind.WHISPER: WhisperExtractor(whisper_model_path),
            ExtractorKind.NOTEBOOK: NotebookExtractor(),
            ExtractorKind.DOCX: DocxExtractor(),
            ExtractorKind.XLSX: XlsxExtractor(),
        }

    @property
    def cache(self) -> ExtractionCache:
        return self._cache

    @staticmethod
    def kind_for_path(path: Path) -> ExtractorKind:
        return EXTENSION_MAP.get(path.suffix.lower(), ExtractorKind.UNKNOWN)

    async def extract(
        self,
        file_path: Path,
        options: dict[str, Any] | None = None,
        *,
        force_kind: ExtractorKind | None = None,
        bypass_cache: bool = False,
    ) -> ExtractionResult:
        opts: dict[str, Any] = options or {}
        path = Path(file_path).expanduser()

        if not path.exists():
            return ExtractionResult(
                extractor=ExtractorKind.UNKNOWN,
                extractor_version="0.0.0",
                file_path=path,
                sha256="",
                success=False,
                error=f"file does not exist: {path}",
            )

        kind = force_kind or self.kind_for_path(path)
        if kind == ExtractorKind.UNKNOWN:
            return ExtractionResult(
                extractor=ExtractorKind.UNKNOWN,
                extractor_version="0.0.0",
                file_path=path,
                sha256="",
                success=False,
                error=f"no extractor registered for extension {path.suffix!r}",
            )

        extractor = self._extractors.get(kind)
        if extractor is None:
            return ExtractionResult(
                extractor=kind,
                extractor_version="0.0.0",
                file_path=path,
                sha256="",
                success=False,
                error=f"extractor for {kind.value!r} is not configured",
            )

        sha = await asyncio.to_thread(sha256_file, path)

        if not bypass_cache:
            cached = await self._cache.get(sha, kind, extractor.version)
            if cached is not None:
                cached_result_dict = dict(cached.result)
                cached_result_dict["cached"] = True
                # Re-validate to guarantee a clean ExtractionResult with no
                # unexpected fields slipping through.
                try:
                    return ExtractionResult.model_validate(cached_result_dict)
                except Exception as exc:  # noqa: BLE001
                    log.warning("ignoring corrupt cache entry for %s: %s", sha, exc)

        result = await extractor.extract(path, opts, sha256=sha)
        if result.success:
            try:
                await self._cache.put(sha, kind, extractor.version, result.model_dump(mode="json"))
            except Exception as exc:  # noqa: BLE001 - cache failures must never crash extraction
                log.warning("failed to write cache entry for %s: %s", sha, exc)
        return result


__all__ = ["EXTENSION_MAP", "Router"]
