"""Async job server.

Reads msgpack-framed jobs from stdin, dispatches them to the router, and
writes msgpack-framed results back to stdout. Jobs are processed in a worker
pool so a slow Whisper transcription does not block faster PDF/image jobs.
"""

from __future__ import annotations

import asyncio
import logging
import os
import sys
from pathlib import Path
from typing import Any

from .cache import DEFAULT_MAX_BYTES, ExtractionCache
from .ipc import FrameError, read_frame, write_frame
from .router import Router
from .types import ExtractorKind, Job, JobResult, JobType

log = logging.getLogger(__name__)


def _is_windows() -> bool:
    return os.name == "nt"


async def _stdio_streams() -> tuple[asyncio.StreamReader, asyncio.StreamWriter]:
    """Wrap raw stdin/stdout in asyncio streams.

    On Windows, ``connect_read_pipe``/``connect_write_pipe`` is not supported on
    the default ``ProactorEventLoop`` for stdio, so we fall back to a thread
    executor. The returned ``StreamReader``/``StreamWriter`` API is identical.
    """
    loop = asyncio.get_running_loop()

    if _is_windows():
        return await _stdio_streams_windows(loop)

    reader = asyncio.StreamReader(limit=64 * 1024 * 1024)
    protocol = asyncio.StreamReaderProtocol(reader)
    await loop.connect_read_pipe(lambda: protocol, sys.stdin.buffer)

    transport, write_protocol = await loop.connect_write_pipe(
        asyncio.streams.FlowControlMixin, sys.stdout.buffer
    )
    writer = asyncio.StreamWriter(transport, write_protocol, None, loop)
    return reader, writer


async def _stdio_streams_windows(
    loop: asyncio.AbstractEventLoop,
) -> tuple[asyncio.StreamReader, asyncio.StreamWriter]:
    """Windows-friendly stdio bridge using a background reader thread."""
    reader = asyncio.StreamReader(limit=64 * 1024 * 1024)

    def _pump() -> None:
        try:
            while True:
                chunk = sys.stdin.buffer.read1(65536)
                if not chunk:
                    loop.call_soon_threadsafe(reader.feed_eof)
                    return
                loop.call_soon_threadsafe(reader.feed_data, chunk)
        except Exception as exc:  # pragma: no cover - defensive
            loop.call_soon_threadsafe(
                reader.set_exception,
                ConnectionError(f"stdin pump failed: {exc}"),
            )

    import threading

    threading.Thread(target=_pump, name="datatree-mm-stdin", daemon=True).start()

    # Writer wraps an in-process queue + thread that flushes to stdout.
    transport = _ThreadedStdoutTransport(loop)
    protocol = asyncio.streams.FlowControlMixin(loop=loop)
    writer = asyncio.StreamWriter(transport, protocol, None, loop)  # type: ignore[arg-type]
    return reader, writer


class _ThreadedStdoutTransport(asyncio.WriteTransport):
    """Minimal write-transport that flushes bytes to ``sys.stdout.buffer``.

    Used only on Windows where ``connect_write_pipe`` does not support stdio
    on the default proactor loop.
    """

    def __init__(self, loop: asyncio.AbstractEventLoop) -> None:
        super().__init__()
        self._loop = loop
        self._closed = False

    def write(self, data: bytes) -> None:
        if self._closed:
            return
        try:
            sys.stdout.buffer.write(data)
            sys.stdout.buffer.flush()
        except (BrokenPipeError, OSError):
            self._closed = True

    def can_write_eof(self) -> bool:  # pragma: no cover - trivial
        return False

    def is_closing(self) -> bool:  # pragma: no cover - trivial
        return self._closed

    def close(self) -> None:
        self._closed = True

    def get_write_buffer_size(self) -> int:  # pragma: no cover - trivial
        return 0


class JobServer:
    """Reads jobs from stdin, runs them, writes results to stdout."""

    def __init__(
        self,
        *,
        cache_dir: Path,
        whisper_model_path: Path | None = None,
        tesseract_cmd: Path | None = None,
        max_cache_bytes: int = DEFAULT_MAX_BYTES,
        concurrency: int = 4,
    ) -> None:
        self._cache = ExtractionCache(cache_dir, max_bytes=max_cache_bytes)
        self._router = Router(
            self._cache,
            whisper_model_path=whisper_model_path,
            tesseract_cmd=tesseract_cmd,
        )
        self._sem = asyncio.Semaphore(concurrency)
        self._writer_lock = asyncio.Lock()
        self._shutting_down = asyncio.Event()

    @property
    def router(self) -> Router:
        return self._router

    async def serve(self) -> None:
        reader, writer = await _stdio_streams()
        log.info("server ready, awaiting frames")
        tasks: set[asyncio.Task[None]] = set()
        try:
            while not self._shutting_down.is_set():
                try:
                    frame = await read_frame(reader)
                except FrameError as exc:
                    log.error("frame error: %s", exc)
                    break
                if frame is None:
                    log.info("stdin closed; exiting")
                    break
                task = asyncio.create_task(self._dispatch(frame, writer))
                tasks.add(task)
                task.add_done_callback(tasks.discard)
        finally:
            if tasks:
                await asyncio.gather(*tasks, return_exceptions=True)
            try:
                writer.close()
            except Exception:  # pragma: no cover - shutdown best-effort
                pass

    async def _dispatch(self, raw_frame: Any, writer: asyncio.StreamWriter) -> None:
        async with self._sem:
            result = await self._handle(raw_frame)
            async with self._writer_lock:
                try:
                    await write_frame(writer, result.model_dump(mode="json"))
                except Exception as exc:  # noqa: BLE001
                    log.error("failed to write result: %s", exc)

    async def _handle(self, raw_frame: Any) -> JobResult:
        try:
            if not isinstance(raw_frame, dict):
                raise ValueError(f"frame must be a map, got {type(raw_frame).__name__}")
            job = Job.model_validate(raw_frame)
        except Exception as exc:  # noqa: BLE001
            return JobResult(
                job_id=str((raw_frame or {}).get("job_id", "?"))
                if isinstance(raw_frame, dict)
                else "?",
                type=JobType.EXTRACT,
                ok=False,
                error=f"invalid job frame: {exc}",
            )

        try:
            if job.type == JobType.PING:
                return JobResult(
                    job_id=job.job_id,
                    type=job.type,
                    ok=True,
                    result={"pong": True},
                )
            if job.type == JobType.SHUTDOWN:
                self._shutting_down.set()
                return JobResult(
                    job_id=job.job_id,
                    type=job.type,
                    ok=True,
                    result={"shutting_down": True},
                )
            if job.type == JobType.CACHE_STATS:
                stats = await self._cache.stats()
                return JobResult(
                    job_id=job.job_id, type=job.type, ok=True, result=stats
                )
            if job.type == JobType.CACHE_PURGE:
                removed = await self._cache.purge()
                return JobResult(
                    job_id=job.job_id,
                    type=job.type,
                    ok=True,
                    result={"removed": removed},
                )
            if job.type == JobType.EXTRACT:
                return await self._handle_extract(job)
            return JobResult(
                job_id=job.job_id,
                type=job.type,
                ok=False,
                error=f"unsupported job type: {job.type.value}",
            )
        except Exception as exc:  # noqa: BLE001
            log.exception("job %s failed", job.job_id)
            return JobResult(
                job_id=job.job_id,
                type=job.type,
                ok=False,
                error=f"{type(exc).__name__}: {exc}",
            )

    async def _handle_extract(self, job: Job) -> JobResult:
        payload = dict(job.payload or {})
        file_path_raw = payload.get("file_path")
        if not file_path_raw:
            return JobResult(
                job_id=job.job_id,
                type=job.type,
                ok=False,
                error="extract payload missing 'file_path'",
            )
        kind_raw = payload.get("extractor")
        force_kind: ExtractorKind | None = None
        if kind_raw is not None:
            try:
                force_kind = ExtractorKind(kind_raw)
            except ValueError:
                return JobResult(
                    job_id=job.job_id,
                    type=job.type,
                    ok=False,
                    error=f"unknown extractor kind: {kind_raw!r}",
                )
        result = await self._router.extract(
            Path(str(file_path_raw)),
            options=payload.get("options") or {},
            force_kind=force_kind,
            bypass_cache=bool(payload.get("bypass_cache", False)),
        )
        return JobResult(
            job_id=job.job_id,
            type=job.type,
            ok=True,
            result=result.model_dump(mode="json"),
        )


__all__ = ["JobServer"]
