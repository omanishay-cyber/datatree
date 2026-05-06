#!/bin/sh
# Install mneme ML models from a LOCAL source path. Internet downloads
# are intentionally REFUSED — pass --from <path> pointing at a directory
# containing the prebundled model files.
#
# Required model:   bge-small-en-v1.5.onnx     (~33 MB)
# Optional models:  phi-3-mini-q4_k_m.gguf     (~2.4 GB)
#                   faster-whisper-base/       (~140 MB)
#
# F18 (2026-05-05 audit): integrity verification.
# If a `<file>.sha256` sidecar exists alongside each staged model, this
# script verifies the SHA-256 sum BEFORE installing. Mismatches refuse
# install. Pass --no-verify to skip (NOT recommended). Files larger
# than 100 MB checksum lazily so the script stays fast on small repos
# unless models are present.
#
# Usage:
#   install_models.sh --from <local-dir> [--with-phi3] [--with-whisper] [--force] [--no-verify]
set -eu

FROM=""
WITH_PHI3=0
WITH_WHISPER=0
FORCE=0
NO_VERIFY=0

usage() {
    sed -n '2,12p' "$0"
}

while [ $# -gt 0 ]; do
    case "$1" in
        --from)
            shift
            [ $# -gt 0 ] || { echo "ERROR: --from requires a path" >&2; exit 1; }
            FROM="$1"
            ;;
        --with-phi3)    WITH_PHI3=1 ;;
        --with-whisper) WITH_WHISPER=1 ;;
        --force)        FORCE=1 ;;
        --no-verify)    NO_VERIFY=1 ;;
        -h|--help)      usage; exit 0 ;;
        *) echo "ERROR: unknown argument: $1" >&2; usage; exit 1 ;;
    esac
    shift
done

if [ -z "$FROM" ]; then
    cat >&2 <<EOF
ERROR: --from <local-path> is required.

mneme REFUSES to fetch models from the internet. Models are large and
must be installed from a verified, locally-staged copy. Download the
models bundle separately, then point this script at the unpacked folder:

    install_models.sh --from /path/to/mneme-models

Required:  bge-small-en-v1.5.onnx
Optional:  phi-3-mini-q4_k_m.gguf, faster-whisper-base/
EOF
    exit 2
fi

[ -d "$FROM" ] || { echo "ERROR: --from path is not a directory: $FROM" >&2; exit 1; }

MNEME_HOME="${MNEME_HOME:-$HOME/.mneme}"
MODEL_DIR="$MNEME_HOME/models"
mkdir -p "$MODEL_DIR"

# F18 (2026-05-05 audit): SHA-256 verification of staged files.
# `expected` reads from `${file}.sha256` (single line — `<hex> <name>`
# or just `<hex>`). Returns 0 if checksum matches OR if there's no
# sidecar AND --no-verify is set; returns 1 on mismatch or
# missing sidecar without --no-verify.
verify_sha256() {
    SRC="$1"
    SIDECAR="${SRC}.sha256"

    # Directories don't get a sidecar — caller skips verification for them.
    if [ -d "$SRC" ]; then
        return 0
    fi

    if [ ! -e "$SIDECAR" ]; then
        if [ "$NO_VERIFY" -eq 1 ]; then
            echo "[warn]    no $SIDECAR — skipping verification (--no-verify)"
            return 0
        fi
        echo "ERROR: $SRC has no $SIDECAR sidecar." >&2
        echo "       Either stage the .sha256 alongside the model or pass --no-verify." >&2
        return 1
    fi

    EXPECTED=$(awk 'NR==1 {print $1}' "$SIDECAR")
    if [ -z "$EXPECTED" ]; then
        echo "ERROR: $SIDECAR is empty." >&2
        return 1
    fi

    # Prefer sha256sum (Linux). Fall back to shasum -a 256 (macOS) and
    # finally openssl (universal). Refuse if none found instead of
    # silently skipping — that would defeat the whole point.
    if command -v sha256sum >/dev/null 2>&1; then
        ACTUAL=$(sha256sum "$SRC" | awk '{print $1}')
    elif command -v shasum >/dev/null 2>&1; then
        ACTUAL=$(shasum -a 256 "$SRC" | awk '{print $1}')
    elif command -v openssl >/dev/null 2>&1; then
        ACTUAL=$(openssl dgst -sha256 "$SRC" | awk '{print $NF}')
    else
        echo "ERROR: no sha256 tool found (sha256sum / shasum / openssl)." >&2
        echo "       Install one or pass --no-verify." >&2
        return 1
    fi

    # Lowercase compare — sha256sum and shasum already lowercase, but
    # some sidecar formats use uppercase hex.
    EXPECTED_LC=$(echo "$EXPECTED" | tr '[:upper:]' '[:lower:]')
    ACTUAL_LC=$(echo "$ACTUAL" | tr '[:upper:]' '[:lower:]')
    if [ "$EXPECTED_LC" != "$ACTUAL_LC" ]; then
        echo "ERROR: SHA-256 mismatch for $SRC" >&2
        echo "       expected: $EXPECTED_LC" >&2
        echo "       actual:   $ACTUAL_LC" >&2
        echo "       The staged file is not the file the sidecar describes." >&2
        echo "       Re-download from a trusted mirror or remove the sidecar to skip." >&2
        return 1
    fi
    echo "[verify]  $SRC ($EXPECTED_LC)"
    return 0
}

copy_model() {
    SRC="$1"
    NAME="$2"
    LABEL="$3"

    if [ ! -e "$SRC" ]; then
        echo "ERROR: $LABEL not found at $SRC" >&2
        return 1
    fi

    if ! verify_sha256 "$SRC"; then
        return 1
    fi

    DEST="$MODEL_DIR/$NAME"
    if [ -e "$DEST" ] && [ "$FORCE" -eq 0 ]; then
        echo "[skip]    $LABEL already installed at $DEST (pass --force to overwrite)"
        return 0
    fi
    if [ -e "$DEST" ]; then
        echo "[backup]  $DEST -> ${DEST}.bak"
        rm -rf "${DEST}.bak"
        mv "$DEST" "${DEST}.bak"
    fi

    echo "[install] $LABEL -> $DEST"
    if [ -d "$SRC" ]; then
        cp -R "$SRC" "$DEST"
    else
        cp -p "$SRC" "$DEST"
    fi
}

# --- required model ----------------------------------------------------------
copy_model "$FROM/bge-small-en-v1.5.onnx" "bge-small-en-v1.5.onnx" "bge-small-en-v1.5 ONNX (33MB)"

# --- optional ----------------------------------------------------------------
if [ "$WITH_PHI3" -eq 1 ]; then
    copy_model "$FROM/phi-3-mini-q4_k_m.gguf" "phi-3-mini-q4_k_m.gguf" "Phi-3-mini Q4_K_M (2.4GB)"
fi

if [ "$WITH_WHISPER" -eq 1 ]; then
    copy_model "$FROM/faster-whisper-base" "faster-whisper-base" "faster-whisper base (140MB)"
fi

echo
echo "Models installed under: $MODEL_DIR"
ls -la "$MODEL_DIR" 2>/dev/null || true
exit 0
