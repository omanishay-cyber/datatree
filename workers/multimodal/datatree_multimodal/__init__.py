"""datatree_multimodal: local-only multimodal extraction sidecar.

This package implements the Python side of the datatree daemon's multimodal
extraction worker. It exposes a small msgpack-framed JSON-RPC-like protocol on
stdin/stdout so a Rust shim crate (`multimodal-bridge`) can spawn it as a
managed sidecar process.

100% LOCAL: no network calls are ever made. Every model is loaded from a path
on disk; if a model is missing the corresponding extractor returns an error
result rather than attempting to download it.
"""

from __future__ import annotations

__all__ = [
    "__version__",
    "EXTRACTOR_VERSIONS",
]

__version__: str = "0.1.0"

# Per-extractor schema versions. Bumping any of these invalidates cache entries
# produced by the previous version, because the cache key includes both the
# file SHA256 and the extractor version.
EXTRACTOR_VERSIONS: dict[str, str] = {
    "pdf": "1.0.0",
    "image": "1.0.0",
    "whisper": "1.0.0",
    "notebook": "1.0.0",
    "docx": "1.0.0",
    "xlsx": "1.0.0",
}
