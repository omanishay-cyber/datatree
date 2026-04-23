"""Jupyter (.ipynb) notebook extractor.

We parse the notebook JSON directly (no ``nbformat`` dependency) so the
extractor works on stripped-down installs. Each cell yields a
:class:`~datatree_multimodal.types.NotebookCell` with cell type, language,
source, execution count, and a one-line summary of any outputs.
"""

from __future__ import annotations

import json
import logging
from pathlib import Path
from typing import Any

from .. import EXTRACTOR_VERSIONS
from ..types import ExtractorKind, NotebookCell
from . import Extractor, run_blocking

log = logging.getLogger(__name__)


class NotebookExtractor(Extractor):
    kind = ExtractorKind.NOTEBOOK
    version = EXTRACTOR_VERSIONS["notebook"]

    async def _extract(
        self,
        file_path: Path,
        options: dict[str, Any],
        warnings: list[str],
    ) -> dict[str, Any]:
        include_outputs = bool(options.get("include_outputs", False))
        return await run_blocking(
            self._parse_sync, file_path, include_outputs, warnings
        )

    @staticmethod
    def _parse_sync(
        file_path: Path,
        include_outputs: bool,
        warnings: list[str],
    ) -> dict[str, Any]:
        try:
            raw = file_path.read_text(encoding="utf-8")
            doc = json.loads(raw)
        except (OSError, ValueError) as exc:
            raise RuntimeError(f"failed to parse notebook: {exc}") from exc

        if not isinstance(doc, dict):
            raise RuntimeError("notebook root is not a JSON object")

        nb_metadata = doc.get("metadata") or {}
        kernel_lang = (
            (nb_metadata.get("kernelspec") or {}).get("language")
            or (nb_metadata.get("language_info") or {}).get("name")
        )

        cells_raw = doc.get("cells")
        if not isinstance(cells_raw, list):
            raise RuntimeError("'cells' field is missing or not a list")

        parsed: list[dict[str, Any]] = []
        outputs_full: list[list[dict[str, Any]]] = []
        code_chars = 0
        markdown_chars = 0

        for idx, cell in enumerate(cells_raw):
            if not isinstance(cell, dict):
                warnings.append(f"cell {idx}: not an object, skipped")
                continue
            ct = cell.get("cell_type")
            if ct not in {"code", "markdown", "raw"}:
                warnings.append(f"cell {idx}: unknown cell_type {ct!r}, skipped")
                continue
            source = cell.get("source", "")
            if isinstance(source, list):
                source = "".join(str(s) for s in source)
            elif not isinstance(source, str):
                source = str(source)

            outputs_summary: str | None = None
            outputs = cell.get("outputs", []) if ct == "code" else []
            if isinstance(outputs, list) and outputs:
                outputs_summary = NotebookExtractor._summarise_outputs(outputs)
            else:
                outputs = []

            try:
                model_cell = NotebookCell(
                    cell_type=ct,  # type: ignore[arg-type]
                    language=kernel_lang if ct == "code" else None,
                    source=source,
                    execution_count=cell.get("execution_count")
                    if ct == "code"
                    else None,
                    outputs_summary=outputs_summary,
                )
            except Exception as exc:  # noqa: BLE001
                warnings.append(f"cell {idx}: validation failed: {exc}")
                continue

            parsed.append(model_cell.model_dump())
            if include_outputs:
                outputs_full.append(outputs if isinstance(outputs, list) else [])

            if ct == "code":
                code_chars += len(source)
            elif ct == "markdown":
                markdown_chars += len(source)

        result: dict[str, Any] = {
            "nbformat": doc.get("nbformat"),
            "nbformat_minor": doc.get("nbformat_minor"),
            "kernel_language": kernel_lang,
            "metadata": nb_metadata,
            "cell_count": len(parsed),
            "code_chars": code_chars,
            "markdown_chars": markdown_chars,
            "cells": parsed,
        }
        if include_outputs:
            result["outputs_raw"] = outputs_full
        return result

    @staticmethod
    def _summarise_outputs(outputs: list[dict[str, Any]]) -> str:
        parts: list[str] = []
        for out in outputs:
            if not isinstance(out, dict):
                continue
            ot = out.get("output_type", "?")
            if ot == "stream":
                name = out.get("name", "stdout")
                text = out.get("text", "")
                if isinstance(text, list):
                    text = "".join(str(s) for s in text)
                snippet = (text or "").strip().splitlines()[:1]
                parts.append(f"stream({name}):{(snippet[0] if snippet else '')[:80]}")
            elif ot in {"execute_result", "display_data"}:
                mimes = list((out.get("data") or {}).keys())
                parts.append(f"{ot}:{','.join(mimes) or 'empty'}")
            elif ot == "error":
                parts.append(f"error:{out.get('ename', '?')}")
            else:
                parts.append(ot)
        return "; ".join(parts)


__all__ = ["NotebookExtractor"]
