"""msgpack frame helpers used for stdin/stdout IPC.

Frame format
------------
Each message is a 4-byte big-endian length prefix followed by the msgpack
payload. EOF on the input stream is signalled by ``read_frame`` returning
``None`` so callers can shut down cleanly.

The Rust shim crate (``multimodal-bridge``) writes these frames; the same
encoder/decoder is also used by tests and by the Python ``JobServer``.
"""

from __future__ import annotations

import asyncio
import struct
from typing import Any

import msgpack

LENGTH_PREFIX_BYTES: int = 4
MAX_FRAME_BYTES: int = 64 * 1024 * 1024  # 64 MiB hard ceiling per message.


class FrameError(Exception):
    """Raised when a frame is malformed or exceeds ``MAX_FRAME_BYTES``."""


def encode_frame(payload: Any) -> bytes:
    """Serialise ``payload`` as a length-prefixed msgpack frame.

    Bytes are emitted as msgpack ``bin`` types; ``str`` are emitted as
    msgpack ``str`` (UTF-8). ``use_bin_type=True`` is therefore mandatory.
    """
    body = msgpack.packb(payload, use_bin_type=True)
    if body is None:  # pragma: no cover - msgpack always returns bytes
        raise FrameError("msgpack.packb returned None")
    if len(body) > MAX_FRAME_BYTES:
        raise FrameError(f"frame too large: {len(body)} > {MAX_FRAME_BYTES}")
    return struct.pack(">I", len(body)) + body


def decode_frame(frame: bytes) -> Any:
    """Inverse of :func:`encode_frame` for an already-extracted body."""
    return msgpack.unpackb(frame, raw=False)


async def read_frame(reader: asyncio.StreamReader) -> Any | None:
    """Read a single msgpack frame from ``reader``.

    Returns ``None`` on a clean EOF (zero bytes before the next prefix). Raises
    :class:`FrameError` on truncated frames or oversized payloads.
    """
    try:
        prefix = await reader.readexactly(LENGTH_PREFIX_BYTES)
    except asyncio.IncompleteReadError as exc:
        if not exc.partial:
            return None
        raise FrameError("truncated length prefix") from exc

    (length,) = struct.unpack(">I", prefix)
    if length == 0:
        return None
    if length > MAX_FRAME_BYTES:
        raise FrameError(f"declared frame too large: {length} > {MAX_FRAME_BYTES}")

    try:
        body = await reader.readexactly(length)
    except asyncio.IncompleteReadError as exc:
        raise FrameError(
            f"truncated frame body: expected {length}, got {len(exc.partial)}"
        ) from exc

    return decode_frame(body)


async def write_frame(writer: asyncio.StreamWriter, payload: Any) -> None:
    """Write a single msgpack frame to ``writer`` and flush."""
    writer.write(encode_frame(payload))
    await writer.drain()


__all__ = [
    "FrameError",
    "LENGTH_PREFIX_BYTES",
    "MAX_FRAME_BYTES",
    "encode_frame",
    "decode_frame",
    "read_frame",
    "write_frame",
]
