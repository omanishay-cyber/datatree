#!/usr/bin/env sh
# datatree :: check-runtime.sh
# Read-only health check: reports presence + version of every runtime dep
# datatree needs.  Never installs, never modifies anything.
#
# Exit codes:
#   0 -- all REQUIRED deps present
#   1 -- one or more REQUIRED deps missing
#
# Usage:
#   sh check-runtime.sh             # human-readable table
#   sh check-runtime.sh --json      # machine-readable JSON
#   sh check-runtime.sh --no-color  # plain ASCII output
#
# POSIX-compliant.

set -eu

DATATREE_HOME="${DATATREE_HOME:-${HOME}/.datatree}"
LOG_DIR="${DATATREE_HOME}/logs"
LOG_FILE="${LOG_DIR}/install.log"
MODEL_DIR="${DATATREE_HOME}/llm"

JSON=0
NO_COLOR=0

while [ $# -gt 0 ]; do
    case "$1" in
        --json)     JSON=1 ;;
        --no-color) NO_COLOR=1 ;;
        -h|--help)
            sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *) echo "unknown arg: $1" >&2; exit 2 ;;
    esac
    shift
done

mkdir -p "$LOG_DIR"
: >> "$LOG_FILE"
log() {
    _ts="$(date -u +'%Y-%m-%dT%H:%M:%SZ')"
    printf '[%s] [CHECK] %s\n' "$_ts" "$*" >> "$LOG_FILE"
}

if [ "$NO_COLOR" -eq 1 ] || [ ! -t 1 ]; then
    GREEN=""; RED=""; YELLOW=""; CYAN=""; RESET=""
    OK_MARK="OK"; BAD_MARK="X "
else
    GREEN="$(printf '\033[0;32m')"
    RED="$(printf '\033[0;31m')"
    YELLOW="$(printf '\033[0;33m')"
    CYAN="$(printf '\033[0;36m')"
    RESET="$(printf '\033[0m')"
    # checkmark / cross: ASCII-safe alternates if locale lacks UTF-8
    if [ "${LANG:-}" != "" ] && echo "$LANG" | grep -qi 'utf'; then
        OK_MARK="$(printf '\xe2\x9c\x93')"
        BAD_MARK="$(printf '\xe2\x9c\x97')"
    else
        OK_MARK="OK"
        BAD_MARK="X "
    fi
fi

has_cmd() { command -v "$1" >/dev/null 2>&1; }

detect_os() {
    case "$(uname -s 2>/dev/null || echo unknown)" in
        Darwin*) echo macos ;;
        Linux*)
            if [ -r /etc/os-release ]; then
                # shellcheck disable=SC1091
                . /etc/os-release
                case "${ID:-}${ID_LIKE:-}" in
                    *debian*|*ubuntu*) echo linux-debian ;;
                    *fedora*|*rhel*)   echo linux-fedora ;;
                    *arch*)            echo linux-arch ;;
                    *)                 echo linux-unknown ;;
                esac
            else echo linux-unknown; fi ;;
        *) echo unknown ;;
    esac
}
OS="$(detect_os)"

install_hint() {
    case "$1:$OS" in
        bun:macos)            echo "brew install oven-sh/bun/bun" ;;
        bun:linux-*)          echo "curl -fsSL https://bun.sh/install | bash" ;;
        python3:macos)        echo "brew install python@3.12" ;;
        python3:linux-debian) echo "sudo apt-get install -y python3 python3-pip" ;;
        python3:linux-fedora) echo "sudo dnf install -y python3 python3-pip" ;;
        python3:linux-arch)   echo "sudo pacman -S --noconfirm python python-pip" ;;
        tesseract:macos)        echo "brew install tesseract" ;;
        tesseract:linux-debian) echo "sudo apt-get install -y tesseract-ocr" ;;
        tesseract:linux-fedora) echo "sudo dnf install -y tesseract" ;;
        tesseract:linux-arch)   echo "sudo pacman -S --noconfirm tesseract" ;;
        ffmpeg:macos)        echo "brew install ffmpeg" ;;
        ffmpeg:linux-debian) echo "sudo apt-get install -y ffmpeg" ;;
        ffmpeg:linux-fedora) echo "sudo dnf install -y ffmpeg" ;;
        ffmpeg:linux-arch)   echo "sudo pacman -S --noconfirm ffmpeg" ;;
        *) echo "(see scripts/install-runtime.sh)" ;;
    esac
}

# (name, version-or-empty, hint-if-missing, required[1/0])
check_one() {
    _name="$1"; _bin="$2"; _required="$3"
    if has_cmd "$_bin"; then
        case "$_bin" in
            bun)       _ver="$(bun --version 2>/dev/null || echo '?')" ;;
            python3)   _ver="$(python3 --version 2>&1 | awk '{print $2}')" ;;
            python)    _ver="$(python --version 2>&1 | awk '{print $2}')" ;;
            tesseract) _ver="$(tesseract --version 2>&1 | head -n1 | awk '{print $2}')" ;;
            ffmpeg)    _ver="$(ffmpeg -version 2>/dev/null | head -n1 | awk '{print $3}')" ;;
            *)         _ver="present" ;;
        esac
        printf "%s\t%s\t%s\t%s\t%s\n" "$_name" "1" "$_ver" "" "$_required"
    else
        printf "%s\t%s\t%s\t%s\t%s\n" "$_name" "0" "" "$(install_hint "$_bin")" "$_required"
    fi
}

# Build raw table (TSV) so we can format twice (text + JSON) if needed.
RAW="$(
    {
        # Try python3 then python as a fallback identity for the same dep
        if has_cmd python3; then
            check_one "Python"    "python3"   "1"
        else
            check_one "Python"    "python"    "1"
        fi
        check_one "Bun"       "bun"       "1"
        check_one "Tesseract" "tesseract" "1"
        check_one "ffmpeg"    "ffmpeg"    "1"
    }
)"

# SQLite is bundled, so always reported OK.
SQLITE_LINE="SQLite\t1\t(bundled in datatree-store)\t\t1"

# Models
BGE_PATH="${MODEL_DIR}/bge-small/model.onnx"
PHI3_PATH="${MODEL_DIR}/phi3-mini/model.onnx"
WHISPER_PATH="${MODEL_DIR}/faster-whisper-base/model.bin"

if [ -f "$BGE_PATH" ]; then
    BGE_LINE="bge-small\t1\t${BGE_PATH} (~33MB)\t\t1"
else
    BGE_LINE="bge-small\t0\t\tdatatree models install --required --from <dir>\t1"
fi
if [ -f "$PHI3_PATH" ]; then
    PHI3_LINE="Phi-3\t1\t${PHI3_PATH} (~2.4GB)\t\t0"
else
    PHI3_LINE="Phi-3\t0\toptional; ~2.4GB\tdatatree models install --with-phi3 --from <dir>\t0"
fi
if [ -f "$WHISPER_PATH" ]; then
    WHISPER_LINE="faster-whisper\t1\t${WHISPER_PATH} (~140MB)\t\t0"
else
    WHISPER_LINE="faster-whisper\t0\toptional; ~140MB\tdatatree models install --with-whisper --from <dir>\t0"
fi

ALL_ROWS="$(printf '%s\n%s\n%s\n%s\n%s\n' "$RAW" "$SQLITE_LINE" "$BGE_LINE" "$PHI3_LINE" "$WHISPER_LINE")"

# -------------------- output ----------------------------------------------
if [ "$JSON" -eq 1 ]; then
    printf '{\n  "deps": [\n'
    _first=1
    printf '%s\n' "$ALL_ROWS" | while IFS="$(printf '\t')" read -r name present ver hint required; do
        [ -z "$name" ] && continue
        if [ $_first -eq 1 ]; then _first=0; else printf ',\n'; fi
        printf '    {"name": "%s", "present": %s, "version": "%s", "hint": "%s", "required": %s}' \
            "$name" \
            "$([ "$present" = "1" ] && echo true || echo false)" \
            "$ver" \
            "$hint" \
            "$([ "$required" = "1" ] && echo true || echo false)"
    done
    printf '\n  ]\n}\n'
else
    # text table
    printf '\n'
    printf '%s%s%s\n' "$CYAN" "datatree :: runtime check  (OS=$OS)" "$RESET"
    printf '%s\n' "------------------------------------------------------------"
    printf '%s\n' "$ALL_ROWS" | while IFS="$(printf '\t')" read -r name present ver hint required; do
        [ -z "$name" ] && continue
        if [ "$present" = "1" ]; then
            printf '%s%s%s %-15s %s\n' "$GREEN" "$OK_MARK" "$RESET" "$name" "${ver:-present}"
        else
            if [ "$required" = "1" ]; then
                printf '%s%s%s %-15s %sNOT FOUND%s  (install: %s)\n' \
                    "$RED" "$BAD_MARK" "$RESET" "$name" "$RED" "$RESET" "$hint"
            else
                printf '%s%s%s %-15s %sNOT FOUND%s  (optional; %s)\n' \
                    "$YELLOW" "$BAD_MARK" "$RESET" "$name" "$YELLOW" "$RESET" "$hint"
            fi
        fi
    done
    printf '\n'
fi

# -------------------- exit code -------------------------------------------
MISSING_REQUIRED=0
printf '%s\n' "$ALL_ROWS" | while IFS="$(printf '\t')" read -r name present ver hint required; do
    [ -z "$name" ] && continue
    if [ "$present" = "0" ] && [ "$required" = "1" ]; then
        echo "miss"
    fi
done > /tmp/.datatree-check.$$
if [ -s /tmp/.datatree-check.$$ ]; then
    MISSING_REQUIRED=1
fi
rm -f /tmp/.datatree-check.$$

log "result: missing_required=$MISSING_REQUIRED"
exit "$MISSING_REQUIRED"
