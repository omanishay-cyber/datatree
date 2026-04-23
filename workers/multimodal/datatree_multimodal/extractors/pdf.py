"""PDF text extraction backed by PyMuPDF (a.k.a. ``fitz``).

Returns text per page plus aggregate metadata: ``page_count``, ``figures``
(count of embedded raster images), and a summary dict suitable for indexing.

PyMuPDF is optional at import time: if it is missing, :class:`PdfExtractor`
still constructs but :meth:`_extract` raises ``RuntimeError`` with a clear
message instead of crashing the whole sidecar at startup.
"""

from __future__ import annotations

import logging
from pathlib import Path
from typing import Any

from .. import EXTRACTOR_VERSIONS
from ..types import ExtractorKind
from . import Extractor, run_blocking

log = logging.getLogger(__name__)

try:
    import fitz  # type: ignore[import-untyped]

    _HAS_PYMUPDF = True
except ImportError:  # pragma: no cover - import-time only
    fitz = None  # type: ignore[assignment]
    _HAS_PYMUPDF = False


class PdfExtractor(Extractor):
    kind = ExtractorKind.PDF
    version = EXTRACTOR_VERSIONS["pdf"]

    async def _extract(
        self,
        file_path: Path,
        options: dict[str, Any],
        warnings: list[str],
    ) -> dict[str, Any]:
        if not _HAS_PYMUPDF:
            raise RuntimeError(
                "pymupdf is not installed; install with `pip install pymupdf`"
            )
        max_pages = int(options.get("max_pages", 0)) or None
        include_blocks = bool(options.get("include_blocks", False))

        return await run_blocking(
            self._extract_sync,
            file_path,
            max_pages,
            include_blocks,
            warnings,
        )

    @staticmethod
    def _extract_sync(
        file_path: Path,
        max_pages: int | None,
        include_blocks: bool,
        warnings: list[str],
    ) -> dict[str, Any]:
        assert fitz is not None  # for type-checkers
        pages: list[dict[str, Any]] = []
        figures = 0
        total_chars = 0

        doc = fitz.open(str(file_path))
        try:
            page_count = doc.page_count
            limit = page_count if max_pages is None else min(page_count, max_pages)
            for idx in range(limit):
                try:
                    page = doc.load_page(idx)
                except Exception as exc:  # noqa: BLE001 - PyMuPDF raises generic errors
                    warnings.append(f"page {idx}: load failed: {exc}")
                    continue
                try:
                    text = page.get_text("text") or ""
                except Exception as exc:  # noqa: BLE001
                    warnings.append(f"page {idx}: get_text failed: {exc}")
                    text = ""
                total_chars += len(text)

                page_info: dict[str, Any] = {
                    "page": idx,
                    "text": text,
                    "char_count": len(text),
                }
                if include_blocks:
                    try:
                        page_info["blocks"] = page.get_text("blocks")
                    except Exception as exc:  # noqa: BLE001
                        warnings.append(f"page {idx}: blocks failed: {exc}")

                try:
                    images = page.get_images(full=False)
                    figures += len(images)
                    page_info["figure_count"] = len(images)
                except Exception as exc:  # noqa: BLE001
                    warnings.append(f"page {idx}: image enum failed: {exc}")
                    page_info["figure_count"] = 0

                pages.append(page_info)

            metadata = dict(doc.metadata or {})
        finally:
            doc.close()

        full_text = "\n\n".join(p["text"] for p in pages if p.get("text"))
        return {
            "page_count": page_count,
            "pages_extracted": len(pages),
            "figures": figures,
            "total_chars": total_chars,
            "text": full_text,
            "pages": pages,
            "metadata": metadata,
        }


__all__ = ["PdfExtractor"]
