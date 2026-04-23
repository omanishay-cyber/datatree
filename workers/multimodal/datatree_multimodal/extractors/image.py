"""Image OCR + heuristic UI element detection.

Text extraction uses Tesseract via ``pytesseract``. UI element detection is
best-effort and uses OpenCV when available; if OpenCV is missing we still
return OCR text plus a warning.
"""

from __future__ import annotations

import logging
import shutil
from pathlib import Path
from typing import Any

from .. import EXTRACTOR_VERSIONS
from ..types import BoundingBox, ExtractorKind
from . import Extractor, run_blocking

log = logging.getLogger(__name__)

try:
    import pytesseract  # type: ignore[import-untyped]
    from PIL import Image

    _HAS_OCR = True
except ImportError:  # pragma: no cover - optional dependency
    pytesseract = None  # type: ignore[assignment]
    Image = None  # type: ignore[assignment]
    _HAS_OCR = False

try:
    import cv2  # type: ignore[import-untyped]
    import numpy as np

    _HAS_CV = True
except ImportError:  # pragma: no cover - optional dependency
    cv2 = None  # type: ignore[assignment]
    np = None  # type: ignore[assignment]
    _HAS_CV = False


class ImageExtractor(Extractor):
    kind = ExtractorKind.IMAGE
    version = EXTRACTOR_VERSIONS["image"]

    def __init__(self, *, tesseract_cmd: Path | None = None) -> None:
        super().__init__()
        self._tesseract_cmd = tesseract_cmd
        if _HAS_OCR and tesseract_cmd is not None:
            pytesseract.pytesseract.tesseract_cmd = str(tesseract_cmd)

    async def _extract(
        self,
        file_path: Path,
        options: dict[str, Any],
        warnings: list[str],
    ) -> dict[str, Any]:
        if not _HAS_OCR:
            raise RuntimeError(
                "pytesseract / Pillow not installed; install with "
                "`pip install pytesseract Pillow`"
            )
        if self._tesseract_cmd is None and shutil.which("tesseract") is None:
            raise RuntimeError(
                "tesseract executable not found on PATH and no --tesseract-cmd given"
            )

        languages = str(options.get("languages", "eng"))
        detect_ui = bool(options.get("detect_ui", True))
        psm = int(options.get("psm", 3))  # default Tesseract auto layout

        return await run_blocking(
            self._extract_sync,
            file_path,
            languages,
            detect_ui,
            psm,
            warnings,
        )

    @staticmethod
    def _extract_sync(
        file_path: Path,
        languages: str,
        detect_ui: bool,
        psm: int,
        warnings: list[str],
    ) -> dict[str, Any]:
        assert pytesseract is not None and Image is not None
        img = Image.open(file_path)
        try:
            img.load()
        except Exception as exc:  # noqa: BLE001
            raise RuntimeError(f"failed to read image: {exc}") from exc

        config = f"--psm {psm}"
        text = pytesseract.image_to_string(img, lang=languages, config=config)

        # Word-level data for downstream highlighting.
        word_boxes: list[dict[str, Any]] = []
        try:
            data = pytesseract.image_to_data(
                img, lang=languages, config=config, output_type=pytesseract.Output.DICT
            )
            n = len(data.get("text", []))
            for i in range(n):
                word = (data["text"][i] or "").strip()
                if not word:
                    continue
                try:
                    conf = float(data["conf"][i])
                except (TypeError, ValueError):
                    conf = -1.0
                word_boxes.append(
                    {
                        "text": word,
                        "x": int(data["left"][i]),
                        "y": int(data["top"][i]),
                        "width": int(data["width"][i]),
                        "height": int(data["height"][i]),
                        "confidence": conf,
                    }
                )
        except Exception as exc:  # noqa: BLE001
            warnings.append(f"image_to_data failed: {exc}")

        ui_elements: list[dict[str, Any]] = []
        if detect_ui:
            if _HAS_CV:
                ui_elements = ImageExtractor._detect_ui_elements(file_path, warnings)
            else:
                warnings.append(
                    "opencv-python not installed; UI element detection skipped"
                )

        return {
            "width": img.width,
            "height": img.height,
            "format": img.format,
            "mode": img.mode,
            "languages": languages,
            "text": text,
            "char_count": len(text),
            "word_count": len(word_boxes),
            "words": word_boxes,
            "ui_elements": ui_elements,
        }

    @staticmethod
    def _detect_ui_elements(file_path: Path, warnings: list[str]) -> list[dict[str, Any]]:
        """Find rectangular regions likely to be UI controls (buttons, panels).

        Pure-heuristic — uses Canny edges + contour approximation. Returns
        bounding boxes sorted by area, capped at 256 entries.
        """
        assert cv2 is not None and np is not None
        try:
            data = np.fromfile(str(file_path), dtype=np.uint8)
            img = cv2.imdecode(data, cv2.IMREAD_GRAYSCALE)
            if img is None:
                warnings.append("cv2.imdecode returned None")
                return []
            h, w = img.shape[:2]
            min_area = max(64, (h * w) // 5000)
            blurred = cv2.GaussianBlur(img, (3, 3), 0)
            edges = cv2.Canny(blurred, 50, 150)
            contours, _ = cv2.findContours(
                edges, cv2.RETR_LIST, cv2.CHAIN_APPROX_SIMPLE
            )
            results: list[BoundingBox] = []
            for c in contours:
                area = cv2.contourArea(c)
                if area < min_area:
                    continue
                approx = cv2.approxPolyDP(c, 0.02 * cv2.arcLength(c, True), True)
                if len(approx) < 4:
                    continue
                x, y, ww, hh = cv2.boundingRect(approx)
                # Reject extreme aspect ratios — those are usually lines/dividers.
                if ww < 8 or hh < 8 or ww / max(hh, 1) > 20 or hh / max(ww, 1) > 20:
                    continue
                results.append(
                    BoundingBox(
                        x=int(x),
                        y=int(y),
                        width=int(ww),
                        height=int(hh),
                        confidence=None,
                        label="rect",
                    )
                )
            results.sort(key=lambda b: b.width * b.height, reverse=True)
            return [r.model_dump() for r in results[:256]]
        except Exception as exc:  # noqa: BLE001
            warnings.append(f"UI detection failed: {exc}")
            return []


__all__ = ["ImageExtractor"]
