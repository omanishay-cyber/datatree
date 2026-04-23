"""Smoke tests for :class:`WhisperExtractor`.

The full transcription test is skipped automatically when the base model is
not present on disk. We still cover the missing-model error path because that
is a critical contract — the extractor MUST refuse to download.
"""

from __future__ import annotations

import os
from pathlib import Path

import pytest

from datatree_multimodal.extractors.whisper import WhisperExtractor
from datatree_multimodal.types import ExtractorKind


def _model_path() -> Path | None:
    raw = os.environ.get("DATATREE_WHISPER_MODEL")
    if not raw:
        return None
    p = Path(raw)
    return p if p.exists() else None


@pytest.mark.asyncio()
async def test_whisper_refuses_when_model_missing(tmp_path: Path) -> None:
    fake_audio = tmp_path / "audio.wav"
    fake_audio.write_bytes(b"RIFF0000WAVEfmt ")  # not a real WAV — never reached.

    extractor = WhisperExtractor(model_path=tmp_path / "nonexistent-model")
    result = await extractor.extract(fake_audio, options={}, sha256="0" * 64)
    assert not result.success
    assert "model not found" in (result.error or "").lower() or "not configured" in (
        result.error or ""
    ).lower()


@pytest.mark.asyncio()
async def test_whisper_refuses_when_path_unset(tmp_path: Path) -> None:
    fake_audio = tmp_path / "audio.wav"
    fake_audio.write_bytes(b"RIFF0000WAVEfmt ")
    extractor = WhisperExtractor(model_path=None)
    result = await extractor.extract(fake_audio, options={}, sha256="0" * 64)
    assert not result.success
    assert "not configured" in (result.error or "").lower()


@pytest.mark.model_required()
@pytest.mark.asyncio()
async def test_whisper_transcribes_real_audio(tmp_path: Path) -> None:
    """End-to-end test that runs only when a real model + sample audio exist.

    Set ``DATATREE_WHISPER_MODEL`` to a directory containing the faster-whisper
    model files and ``DATATREE_WHISPER_SAMPLE`` to a small WAV/MP3 to enable.
    """
    model = _model_path()
    sample_raw = os.environ.get("DATATREE_WHISPER_SAMPLE")
    if model is None or not sample_raw or not Path(sample_raw).exists():
        pytest.skip("DATATREE_WHISPER_MODEL or DATATREE_WHISPER_SAMPLE not set")

    extractor = WhisperExtractor(model)
    sample = Path(sample_raw)
    result = await extractor.extract(
        sample,
        options={"domain_prompt": "datatree integration test"},
        sha256="0" * 64,
    )
    assert result.success, result.error
    assert result.extractor == ExtractorKind.WHISPER
    assert isinstance(result.data["segments"], list)
    assert "text" in result.data
