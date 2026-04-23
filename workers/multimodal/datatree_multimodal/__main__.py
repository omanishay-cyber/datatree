"""Entry point for the datatree multimodal sidecar.

Invoked by the Rust ``multimodal-bridge`` crate as::

    python -m datatree_multimodal [--cache-dir PATH] [--whisper-model PATH]
                                  [--tesseract-cmd PATH] [--log-level LEVEL]

The process reads msgpack-framed jobs from stdin and writes msgpack-framed
results to stdout. All log messages go to stderr so they do not corrupt the
binary IPC stream on stdout.
"""

from __future__ import annotations

import argparse
import asyncio
import logging
import os
import signal
import sys
from pathlib import Path

from .server import JobServer


def _build_arg_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="datatree-multimodal",
        description="Local multimodal extraction sidecar for the datatree daemon.",
    )
    parser.add_argument(
        "--cache-dir",
        type=Path,
        default=Path.home() / ".datatree" / "cache" / "multimodal",
        help="Directory used to cache extraction results (default: ~/.datatree/cache/multimodal).",
    )
    parser.add_argument(
        "--whisper-model",
        type=Path,
        default=None,
        help="Path to the faster-whisper base model directory on disk.",
    )
    parser.add_argument(
        "--tesseract-cmd",
        type=Path,
        default=None,
        help="Path to the tesseract executable (overrides PATH lookup).",
    )
    parser.add_argument(
        "--log-level",
        default=os.environ.get("DATATREE_MM_LOG", "INFO"),
        choices=["DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"],
        help="Logging level (default: INFO).",
    )
    parser.add_argument(
        "--max-cache-bytes",
        type=int,
        default=5 * 1024 * 1024 * 1024,
        help="Maximum on-disk cache size in bytes (default: 5 GiB).",
    )
    return parser


def _configure_logging(level: str) -> None:
    logging.basicConfig(
        level=getattr(logging, level),
        stream=sys.stderr,
        format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
    )


async def _run(server: JobServer) -> int:
    loop = asyncio.get_running_loop()
    stop_event = asyncio.Event()

    def _request_stop() -> None:
        stop_event.set()

    # SIGTERM/SIGINT handling. On Windows, add_signal_handler raises
    # NotImplementedError; fall back to default KeyboardInterrupt handling.
    for sig_name in ("SIGTERM", "SIGINT"):
        sig = getattr(signal, sig_name, None)
        if sig is None:
            continue
        try:
            loop.add_signal_handler(sig, _request_stop)
        except NotImplementedError:
            # Windows event loop does not support add_signal_handler.
            pass

    serve_task = asyncio.create_task(server.serve(), name="datatree-mm-serve")
    stop_task = asyncio.create_task(stop_event.wait(), name="datatree-mm-stop")
    done, _pending = await asyncio.wait(
        {serve_task, stop_task},
        return_when=asyncio.FIRST_COMPLETED,
    )

    if stop_task in done and not serve_task.done():
        serve_task.cancel()
        try:
            await serve_task
        except asyncio.CancelledError:
            pass
        return 0

    # serve_task completed (likely EOF on stdin).
    try:
        await serve_task
    except Exception:  # pragma: no cover - defensive
        logging.getLogger(__name__).exception("server crashed")
        return 1
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = _build_arg_parser()
    args = parser.parse_args(argv)
    _configure_logging(args.log_level)

    log = logging.getLogger("datatree_multimodal")
    log.info(
        "starting datatree-multimodal sidecar (cache_dir=%s, whisper=%s)",
        args.cache_dir,
        args.whisper_model,
    )

    server = JobServer(
        cache_dir=args.cache_dir,
        whisper_model_path=args.whisper_model,
        tesseract_cmd=args.tesseract_cmd,
        max_cache_bytes=args.max_cache_bytes,
    )

    try:
        return asyncio.run(_run(server))
    except KeyboardInterrupt:
        return 0


if __name__ == "__main__":
    raise SystemExit(main())
