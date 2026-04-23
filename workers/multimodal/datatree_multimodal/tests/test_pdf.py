"""Smoke tests for :class:`PdfExtractor`.

These tests are skipped when PyMuPDF is not installed so the suite remains
runnable on a fresh checkout without all optional deps.
"""

from __future__ import annotations

from pathlib import Path

import pytest

pytest.importorskip("fitz", reason="pymupdf not installed")

import fitz  # noqa: E402

from datatree_multimodal.cache import sha256_file  # noqa: E402
from datatree_multimodal.extractors.pdf import PdfExtractor  # noqa: E402
from datatree_multimodal.types import ExtractorKind  # noqa: E402


@pytest.fixture()
def sample_pdf(tmp_path: Path) -> Path:
    """Synthesize a tiny two-page PDF with deterministic text."""
    path = tmp_path / "sample.pdf"
    doc = fitz.open()
    page1 = doc.new_page()
    page1.insert_text((72, 72), "Hello datatree")
    page2 = doc.new_page()
    page2.insert_text((72, 72), "Second page contents")
    doc.save(str(path))
    doc.close()
    return path


@pytest.mark.asyncio()
async def test_pdf_extractor_returns_text_and_page_count(sample_pdf: Path) -> None:
    extractor = PdfExtractor()
    sha = sha256_file(sample_pdf)
    result = await extractor.extract(sample_pdf, options={}, sha256=sha)

    assert result.success, result.error
    assert result.extractor == ExtractorKind.PDF
    assert result.data["page_count"] == 2
    assert "Hello datatree" in result.data["text"]
    assert "Second page contents" in result.data["text"]
    assert result.data["pages_extracted"] == 2
    assert result.data["figures"] == 0


@pytest.mark.asyncio()
async def test_pdf_extractor_handles_missing_file(tmp_path: Path) -> None:
    extractor = PdfExtractor()
    missing = tmp_path / "does-not-exist.pdf"
    result = await extractor.extract(missing, options={}, sha256="0" * 64)
    assert not result.success
    assert "does not exist" in (result.error or "")


@pytest.mark.asyncio()
async def test_pdf_extractor_respects_max_pages(sample_pdf: Path) -> None:
    extractor = PdfExtractor()
    sha = sha256_file(sample_pdf)
    result = await extractor.extract(sample_pdf, options={"max_pages": 1}, sha256=sha)
    assert result.success, result.error
    assert result.data["pages_extracted"] == 1
    assert result.data["page_count"] == 2
