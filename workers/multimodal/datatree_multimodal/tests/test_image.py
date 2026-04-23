"""Smoke tests for :class:`ImageExtractor`.

We do not require Tesseract to be installed for the tests to pass — when
Tesseract is unavailable the extractor returns an error result and we assert
that contract instead.
"""

from __future__ import annotations

import shutil
from pathlib import Path

import pytest

pytest.importorskip("PIL", reason="Pillow not installed")

from PIL import Image, ImageDraw  # noqa: E402

from datatree_multimodal.cache import sha256_file  # noqa: E402
from datatree_multimodal.extractors.image import ImageExtractor  # noqa: E402
from datatree_multimodal.types import ExtractorKind  # noqa: E402


@pytest.fixture()
def sample_png(tmp_path: Path) -> Path:
    path = tmp_path / "sample.png"
    img = Image.new("RGB", (320, 120), color="white")
    draw = ImageDraw.Draw(img)
    draw.rectangle((10, 10, 200, 80), outline="black", width=2)
    draw.text((20, 30), "DATATREE 123", fill="black")
    img.save(path, format="PNG")
    return path


@pytest.mark.asyncio()
async def test_image_extractor_metadata_is_present(sample_png: Path) -> None:
    extractor = ImageExtractor()
    sha = sha256_file(sample_png)
    result = await extractor.extract(sample_png, options={"detect_ui": False}, sha256=sha)

    if result.success:
        assert result.data["width"] == 320
        assert result.data["height"] == 120
        assert result.data["format"] == "PNG"
        assert result.extractor == ExtractorKind.IMAGE
        # OCR may produce noise but the field must exist.
        assert "text" in result.data
    else:
        # Tesseract not installed — that is allowed.
        assert "tesseract" in (result.error or "").lower()


@pytest.mark.asyncio()
async def test_image_extractor_missing_file(tmp_path: Path) -> None:
    extractor = ImageExtractor()
    missing = tmp_path / "missing.png"
    result = await extractor.extract(missing, options={}, sha256="0" * 64)
    assert not result.success
    assert "does not exist" in (result.error or "")


@pytest.mark.asyncio()
async def test_ui_detection_returns_list(sample_png: Path) -> None:
    if shutil.which("tesseract") is None:
        pytest.skip("tesseract not installed; UI detection still runs but skipped here")
    extractor = ImageExtractor()
    sha = sha256_file(sample_png)
    result = await extractor.extract(sample_png, options={"detect_ui": True}, sha256=sha)
    assert result.success, result.error
    assert isinstance(result.data["ui_elements"], list)
