"""ASR via faster-whisper (CPU-only, base model).

The model is loaded lazily from the path supplied at construction time. We
NEVER attempt to download the model — if it is missing we surface a clear
``RuntimeError`` and let the caller install it via ``scripts/install_models.py``.
"""

from __future__ import annotations

import logging
from pathlib import Path
from typing import Any

from .. import EXTRACTOR_VERSIONS
from ..types import ExtractorKind, TranscriptSegment
from . import Extractor, run_blocking

log = logging.getLogger(__name__)

try:
    from faster_whisper import WhisperModel  # type: ignore[import-untyped]

    _HAS_WHISPER = True
except ImportError:  # pragma: no cover - optional dependency
    WhisperModel = None  # type: ignore[assignment]
    _HAS_WHISPER = False


class WhisperExtractor(Extractor):
    kind = ExtractorKind.WHISPER
    version = EXTRACTOR_VERSIONS["whisper"]

    def __init__(
        self,
        model_path: Path | None,
        *,
        compute_type: str = "int8",
        device: str = "cpu",
        cpu_threads: int = 0,
        num_workers: int = 1,
    ) -> None:
        super().__init__()
        self._model_path = model_path
        self._compute_type = compute_type
        self._device = device
        self._cpu_threads = cpu_threads
        self._num_workers = num_workers
        self._model: Any | None = None

    def _ensure_model(self) -> Any:
        if not _HAS_WHISPER:
            raise RuntimeError(
                "faster-whisper not installed; install with `pip install faster-whisper`"
            )
        if self._model_path is None:
            raise RuntimeError(
                "whisper model path not configured; pass --whisper-model or call install_models.py"
            )
        if not self._model_path.exists():
            raise RuntimeError(
                f"whisper model not found on disk at {self._model_path}; refusing to download"
            )
        if self._model is None:
            assert WhisperModel is not None
            log.info("loading whisper model from %s", self._model_path)
            self._model = WhisperModel(
                str(self._model_path),
                device=self._device,
                compute_type=self._compute_type,
                cpu_threads=self._cpu_threads,
                num_workers=self._num_workers,
                local_files_only=True,
            )
        return self._model

    async def _extract(
        self,
        file_path: Path,
        options: dict[str, Any],
        warnings: list[str],
    ) -> dict[str, Any]:
        domain_prompt = options.get("domain_prompt") or options.get("initial_prompt")
        language = options.get("language")
        beam_size = int(options.get("beam_size", 5))
        vad_filter = bool(options.get("vad_filter", True))
        word_timestamps = bool(options.get("word_timestamps", False))
        temperature = float(options.get("temperature", 0.0))

        model = self._ensure_model()

        return await run_blocking(
            self._transcribe_sync,
            model,
            file_path,
            domain_prompt,
            language,
            beam_size,
            vad_filter,
            word_timestamps,
            temperature,
            warnings,
        )

    @staticmethod
    def _transcribe_sync(
        model: Any,
        file_path: Path,
        domain_prompt: str | None,
        language: str | None,
        beam_size: int,
        vad_filter: bool,
        word_timestamps: bool,
        temperature: float,
        warnings: list[str],
    ) -> dict[str, Any]:
        try:
            segments_iter, info = model.transcribe(
                str(file_path),
                initial_prompt=domain_prompt,
                language=language,
                beam_size=beam_size,
                vad_filter=vad_filter,
                word_timestamps=word_timestamps,
                temperature=temperature,
            )
        except Exception as exc:  # noqa: BLE001 - faster-whisper raises generic errors
            raise RuntimeError(f"whisper transcribe failed: {exc}") from exc

        segments: list[dict[str, Any]] = []
        full_text_parts: list[str] = []
        for seg in segments_iter:
            try:
                model_seg = TranscriptSegment(
                    start=float(seg.start),
                    end=float(seg.end),
                    text=str(seg.text).strip(),
                    avg_logprob=getattr(seg, "avg_logprob", None),
                    no_speech_prob=getattr(seg, "no_speech_prob", None),
                )
            except Exception as exc:  # noqa: BLE001
                warnings.append(f"segment skipped: {exc}")
                continue
            segments.append(model_seg.model_dump())
            full_text_parts.append(model_seg.text)

        return {
            "language": getattr(info, "language", language),
            "language_probability": getattr(info, "language_probability", None),
            "duration": getattr(info, "duration", None),
            "duration_after_vad": getattr(info, "duration_after_vad", None),
            "domain_prompt": domain_prompt,
            "text": " ".join(full_text_parts).strip(),
            "segment_count": len(segments),
            "segments": segments,
        }


__all__ = ["WhisperExtractor"]
