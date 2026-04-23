"""Extractor implementations for the datatree multimodal sidecar.

Every extractor exposes the same callable shape::

    async def extract(file_path: Path, options: dict) -> ExtractionResult

The wrapper :class:`Extractor` provides shared timing, error handling, and
SHA256 computation so concrete implementations only need to focus on parsing
their specific format.
"""

from __future__ import annotations

import asyncio
import logging
import time
from abc import ABC, abstractmethod
from pathlib import Path
from typing import Any

from ..types import ExtractionResult, ExtractorKind

log = logging.getLogger(__name__)


class Extractor(ABC):
    """Base class shared by every extractor.

    Subclasses implement :meth:`_extract` which performs the actual work and
    returns the extractor-specific ``data`` dict. The base class wraps the
    call with timing, error handling, and the uniform result envelope.
    """

    kind: ExtractorKind
    version: str

    def __init__(self) -> None:
        if not hasattr(self, "kind"):
            raise TypeError(f"{type(self).__name__} must set 'kind'")
        if not hasattr(self, "version"):
            raise TypeError(f"{type(self).__name__} must set 'version'")

    async def extract(
        self,
        file_path: Path,
        options: dict[str, Any],
        *,
        sha256: str,
    ) -> ExtractionResult:
        start = time.perf_counter()
        warnings: list[str] = []
        try:
            if not file_path.exists():
                raise FileNotFoundError(f"file does not exist: {file_path}")
            data = await self._extract(file_path, options, warnings)
            duration_ms = (time.perf_counter() - start) * 1000
            return ExtractionResult(
                extractor=self.kind,
                extractor_version=self.version,
                file_path=file_path,
                sha256=sha256,
                success=True,
                data=data,
                warnings=warnings,
                duration_ms=duration_ms,
            )
        except Exception as exc:
            duration_ms = (time.perf_counter() - start) * 1000
            log.exception("extractor %s failed for %s", self.kind.value, file_path)
            return ExtractionResult(
                extractor=self.kind,
                extractor_version=self.version,
                file_path=file_path,
                sha256=sha256,
                success=False,
                data={},
                error=f"{type(exc).__name__}: {exc}",
                warnings=warnings,
                duration_ms=duration_ms,
            )

    @abstractmethod
    async def _extract(
        self,
        file_path: Path,
        options: dict[str, Any],
        warnings: list[str],
    ) -> dict[str, Any]:
        """Return extractor-specific ``data`` dict. Should raise on failure."""


async def run_blocking(func: Any, *args: Any, **kwargs: Any) -> Any:
    """Run a blocking callable in the default executor."""
    loop = asyncio.get_running_loop()
    return await loop.run_in_executor(None, lambda: func(*args, **kwargs))


__all__ = ["Extractor", "run_blocking"]
