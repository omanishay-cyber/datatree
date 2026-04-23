#!/bin/sh
# Install datatree ML models from a LOCAL source path. Internet downloads
# are intentionally REFUSED — pass --from <path> pointing at a directory
# containing the prebundled model files.
#
# Required model:   bge-small-en-v1.5.onnx     (~33 MB)
# Optional models:  phi-3-mini-q4_k_m.gguf     (~2.4 GB)
#                   faster-whisper-base/       (~140 MB)
#
# Usage:
#   install_models.sh --from <local-dir> [--with-phi3] [--with-whisper] [--force]
set -eu

FROM=""
WITH_PHI3=0
WITH_WHISPER=0
FORCE=0

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
        -h|--help)      usage; exit 0 ;;
        *) echo "ERROR: unknown argument: $1" >&2; usage; exit 1 ;;
    esac
    shift
done

if [ -z "$FROM" ]; then
    cat >&2 <<EOF
ERROR: --from <local-path> is required.

datatree REFUSES to fetch models from the internet. Models are large and
must be installed from a verified, locally-staged copy. Download the
models bundle separately, then point this script at the unpacked folder:

    install_models.sh --from /path/to/datatree-models

Required:  bge-small-en-v1.5.onnx
Optional:  phi-3-mini-q4_k_m.gguf, faster-whisper-base/
EOF
    exit 2
fi

[ -d "$FROM" ] || { echo "ERROR: --from path is not a directory: $FROM" >&2; exit 1; }

MNEME_HOME="${MNEME_HOME:-$HOME/.datatree}"
MODEL_DIR="$MNEME_HOME/models"
mkdir -p "$MODEL_DIR"

copy_model() {
    SRC="$1"
    NAME="$2"
    LABEL="$3"

    if [ ! -e "$SRC" ]; then
        echo "ERROR: $LABEL not found at $SRC" >&2
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
