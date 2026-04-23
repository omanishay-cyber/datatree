"""One-time installer for Tesseract data + faster-whisper base model.

This script DOES NOT touch the network. It accepts a local source directory
via ``--from`` and copies the required artefacts into the datatree models tree
(default: ``~/.datatree/models``).

Expected source layout (under ``--from``)::

    <src>/tesseract/tessdata/eng.traineddata     [+ optional langs]
    <src>/whisper/base/                          # faster-whisper model dir
        ├── model.bin
        ├── config.json
        ├── tokenizer.json
        └── vocabulary.txt

Usage::

    python -m scripts.install_models --from /mnt/datatree-models
    python -m scripts.install_models --from D:\\datatree-mirror --whisper-only
"""

from __future__ import annotations

import argparse
import logging
import shutil
import sys
from pathlib import Path

log = logging.getLogger("datatree.install_models")

DEFAULT_DEST = Path.home() / ".datatree" / "models"

WHISPER_REQUIRED = {"model.bin", "config.json", "tokenizer.json", "vocabulary.txt"}
TESSERACT_REQUIRED = {"eng.traineddata"}


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="install_models",
        description="Copy Tesseract + Whisper artefacts from a local mirror into "
        "the datatree models directory. NO network calls are ever made.",
    )
    parser.add_argument(
        "--from",
        dest="source",
        type=Path,
        required=True,
        help="Local mirror root (filesystem path) containing 'whisper' and 'tesseract' subdirs.",
    )
    parser.add_argument(
        "--dest",
        type=Path,
        default=DEFAULT_DEST,
        help=f"Destination root (default: {DEFAULT_DEST}).",
    )
    parser.add_argument(
        "--whisper-only",
        action="store_true",
        help="Install only the Whisper model.",
    )
    parser.add_argument(
        "--tesseract-only",
        action="store_true",
        help="Install only Tesseract data.",
    )
    parser.add_argument(
        "--whisper-variant",
        default="base",
        help="Sub-directory under <src>/whisper/ to install (default: base).",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Overwrite existing files in the destination tree.",
    )
    parser.add_argument(
        "--log-level",
        default="INFO",
        choices=["DEBUG", "INFO", "WARNING", "ERROR"],
    )
    return parser


def _copy_tree(src: Path, dst: Path, *, force: bool) -> int:
    """Copy ``src`` directory contents into ``dst``. Returns file count."""
    if not src.is_dir():
        raise FileNotFoundError(f"source directory not found: {src}")
    dst.mkdir(parents=True, exist_ok=True)
    count = 0
    for entry in src.rglob("*"):
        if entry.is_dir():
            continue
        rel = entry.relative_to(src)
        target = dst / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        if target.exists() and not force:
            log.debug("skip existing: %s", target)
            continue
        shutil.copy2(entry, target)
        count += 1
        log.info("copied %s -> %s", entry, target)
    return count


def install_whisper(source: Path, dest: Path, variant: str, *, force: bool) -> Path:
    src = source / "whisper" / variant
    dst = dest / "whisper" / variant
    log.info("installing whisper '%s' from %s -> %s", variant, src, dst)
    _copy_tree(src, dst, force=force)
    missing = WHISPER_REQUIRED - {p.name for p in dst.iterdir() if p.is_file()}
    if missing:
        raise RuntimeError(
            f"whisper install incomplete: missing {sorted(missing)} in {dst}"
        )
    return dst


def install_tesseract(source: Path, dest: Path, *, force: bool) -> Path:
    src = source / "tesseract" / "tessdata"
    dst = dest / "tesseract" / "tessdata"
    log.info("installing tesseract data from %s -> %s", src, dst)
    _copy_tree(src, dst, force=force)
    missing = TESSERACT_REQUIRED - {p.name for p in dst.iterdir() if p.is_file()}
    if missing:
        raise RuntimeError(
            f"tesseract install incomplete: missing {sorted(missing)} in {dst}"
        )
    return dst


def main(argv: list[str] | None = None) -> int:
    parser = _build_parser()
    args = parser.parse_args(argv)
    logging.basicConfig(
        level=getattr(logging, args.log_level),
        format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
    )

    if args.whisper_only and args.tesseract_only:
        parser.error("--whisper-only and --tesseract-only are mutually exclusive")

    source: Path = args.source.expanduser().resolve()
    dest: Path = args.dest.expanduser().resolve()
    if not source.exists():
        log.error("source does not exist: %s", source)
        return 2

    do_whisper = not args.tesseract_only
    do_tesseract = not args.whisper_only

    try:
        if do_whisper:
            wpath = install_whisper(source, dest, args.whisper_variant, force=args.force)
            log.info("whisper installed at %s", wpath)
        if do_tesseract:
            tpath = install_tesseract(source, dest, force=args.force)
            log.info("tesseract data installed at %s", tpath)
    except FileNotFoundError as exc:
        log.error("%s", exc)
        return 3
    except RuntimeError as exc:
        log.error("install verification failed: %s", exc)
        return 4

    log.info("install complete; pass --whisper-model %s to the sidecar", dest / "whisper" / args.whisper_variant)
    return 0


if __name__ == "__main__":
    sys.exit(main())
