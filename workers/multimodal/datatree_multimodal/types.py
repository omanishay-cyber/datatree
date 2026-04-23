"""Pydantic models describing jobs and extraction results.

These models are validated at the IPC boundary so the Rust shim crate can rely
on a stable schema. All models use ``model_config = ConfigDict(extra="forbid")``
so unexpected fields surface as validation errors instead of being silently
dropped.
"""

from __future__ import annotations

from datetime import datetime, timezone
from enum import Enum
from pathlib import Path
from typing import Any, Literal

from pydantic import BaseModel, ConfigDict, Field, field_validator


class JobType(str, Enum):
    """Supported job kinds. Matches the Rust ``MultimodalJobKind`` enum."""

    EXTRACT = "extract"
    PING = "ping"
    SHUTDOWN = "shutdown"
    CACHE_STATS = "cache_stats"
    CACHE_PURGE = "cache_purge"


class ExtractorKind(str, Enum):
    PDF = "pdf"
    IMAGE = "image"
    WHISPER = "whisper"
    NOTEBOOK = "notebook"
    DOCX = "docx"
    XLSX = "xlsx"
    UNKNOWN = "unknown"


class ExtractPayload(BaseModel):
    """Payload for ``JobType.EXTRACT``."""

    model_config = ConfigDict(extra="forbid")

    file_path: Path
    extractor: ExtractorKind | None = Field(
        default=None,
        description="Force a specific extractor; if omitted the router picks one by extension.",
    )
    options: dict[str, Any] = Field(default_factory=dict)
    bypass_cache: bool = False

    @field_validator("file_path")
    @classmethod
    def _path_not_empty(cls, value: Path) -> Path:
        if str(value).strip() == "":
            raise ValueError("file_path must not be empty")
        return value


class Job(BaseModel):
    """Top-level job envelope."""

    model_config = ConfigDict(extra="forbid")

    job_id: str
    type: JobType
    payload: dict[str, Any] = Field(default_factory=dict)


class ExtractionFigure(BaseModel):
    model_config = ConfigDict(extra="forbid")

    page: int
    width: int
    height: int
    bbox: tuple[float, float, float, float] | None = None


class BoundingBox(BaseModel):
    model_config = ConfigDict(extra="forbid")

    x: int
    y: int
    width: int
    height: int
    confidence: float | None = None
    label: str | None = None


class TranscriptSegment(BaseModel):
    model_config = ConfigDict(extra="forbid")

    start: float
    end: float
    text: str
    avg_logprob: float | None = None
    no_speech_prob: float | None = None


class NotebookCell(BaseModel):
    model_config = ConfigDict(extra="forbid")

    cell_type: Literal["code", "markdown", "raw"]
    language: str | None = None
    source: str
    execution_count: int | None = None
    outputs_summary: str | None = None


class ExtractionResult(BaseModel):
    """Uniform return type for every extractor.

    ``data`` carries the extractor-specific payload; the keys are documented
    per-extractor in the corresponding module docstring.
    """

    model_config = ConfigDict(extra="forbid")

    extractor: ExtractorKind
    extractor_version: str
    file_path: Path
    sha256: str
    success: bool
    data: dict[str, Any] = Field(default_factory=dict)
    error: str | None = None
    warnings: list[str] = Field(default_factory=list)
    duration_ms: float = 0.0
    cached: bool = False
    created_at: datetime = Field(default_factory=lambda: datetime.now(timezone.utc))


class JobResult(BaseModel):
    """Envelope written back to the Rust shim for every job."""

    model_config = ConfigDict(extra="forbid")

    job_id: str
    type: JobType
    ok: bool
    result: dict[str, Any] | None = None
    error: str | None = None


class CacheEntry(BaseModel):
    """On-disk representation of a cache entry."""

    model_config = ConfigDict(extra="forbid")

    hash: str
    extractor: ExtractorKind
    extractor_version: str
    result: dict[str, Any]
    created_at: datetime
    size_bytes: int = 0


__all__ = [
    "JobType",
    "ExtractorKind",
    "ExtractPayload",
    "Job",
    "ExtractionFigure",
    "BoundingBox",
    "TranscriptSegment",
    "NotebookCell",
    "ExtractionResult",
    "JobResult",
    "CacheEntry",
]
