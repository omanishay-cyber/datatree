#!/usr/bin/env sh
# mneme :: install-bundle.sh
# Master end-to-end installer.  Single command does everything in order:
#   1. Detect OS + arch
#   2. Run check-runtime.sh; collect missing
#   3. If missing required: install-runtime.sh --auto-install (with confirm)
#   4. install-supervisor.sh
#   5. install_models.sh --required (only bge-small)
#   6. start-daemon.sh
#   7. Print next-steps banner
#
# Flags:
#   --yes              : assume yes to all prompts
#   --no-start         : skip step 6 (don't start the daemon)
#   --from <dir>       : pass through to install-runtime.sh / install_models.sh
#   --skip-models      : skip step 5
#   --skip-runtime     : skip steps 2-3 (assume runtime already OK)
#
# Exit codes:
#   0  success
#   1+ first failing step's exit code (early-exit)

set -eu

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
MNEME_HOME="${MNEME_HOME:-${HOME}/.mneme}"
LOG_DIR="${MNEME_HOME}/logs"
LOG_FILE="${LOG_DIR}/install.log"

ASSUME_YES=0
NO_START=0
SKIP_MODELS=0
SKIP_RUNTIME=0
FROM_DIR=""

while [ $# -gt 0 ]; do
    case "$1" in
        --yes|-y)        ASSUME_YES=1 ;;
        --no-start)      NO_START=1 ;;
        --skip-models)   SKIP_MODELS=1 ;;
        --skip-runtime)  SKIP_RUNTIME=1 ;;
        --from)
            shift; FROM_DIR="$1"
            ;;
        --from=*) FROM_DIR="${1#--from=}" ;;
        -h|--help)
            sed -n '2,22p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *) echo "unknown arg: $1" >&2; exit 4 ;;
    esac
    shift
done

mkdir -p "$LOG_DIR"
: >> "$LOG_FILE"

log() {
    _ts="$(date -u +'%Y-%m-%dT%H:%M:%SZ')"
    printf '[%s] [BUNDLE] %s\n' "$_ts" "$*" >> "$LOG_FILE"
    printf '\033[1;35m[bundle]\033[0m %s\n' "$*"
}

step() {
    log "===== STEP: $* ====="
    printf '\n\033[1;35m========== %s ==========\033[0m\n' "$*"
}

die() {
    log "FATAL: $*"
    printf '\n\033[1;31m[bundle][FATAL]\033[0m %s\n' "$*" >&2
    printf 'See log: %s\n' "$LOG_FILE" >&2
    exit "${2:-1}"
}

# ---------- step 1: detect OS + arch
step "1. Detect OS + arch"
OS_RAW="$(uname -s 2>/dev/null || echo unknown)"
ARCH="$(uname -m 2>/dev/null || echo unknown)"
log "uname: $OS_RAW / $ARCH"
case "$OS_RAW" in
    Darwin*|Linux*) ;;
    *) die "Unsupported OS: $OS_RAW (use install-bundle.ps1 on Windows)" 3 ;;
esac
log "OS=$OS_RAW arch=$ARCH"

# ---------- argument forwarding
PASS_THROUGH=""
[ $ASSUME_YES -eq 1 ] && PASS_THROUGH="$PASS_THROUGH --yes"
[ -n "$FROM_DIR" ] && PASS_THROUGH="$PASS_THROUGH --from $FROM_DIR"

# ---------- step 2 + 3: runtime check + auto-install
if [ $SKIP_RUNTIME -eq 1 ]; then
    log "Skipping runtime steps (--skip-runtime)"
else
    step "2. Check runtime dependencies"
    set +e
    sh "$SCRIPT_DIR/check-runtime.sh"
    CHECK_RC=$?
    set -e
    log "check-runtime.sh exit=$CHECK_RC"

    if [ $CHECK_RC -ne 0 ]; then
        step "3. Auto-install missing runtime dependencies"
        if [ $ASSUME_YES -eq 0 ]; then
            printf 'Some required runtime deps are missing. Install them now? [y/N]: '
            read -r _ans </dev/tty 2>/dev/null || _ans="n"
            case "$_ans" in
                y|Y|yes|YES) ;;
                *) die "User declined runtime install" 1 ;;
            esac
        fi
        # shellcheck disable=SC2086
        sh "$SCRIPT_DIR/install-runtime.sh" --auto-install $PASS_THROUGH \
            || die "install-runtime.sh failed" 2
    else
        log "All runtime deps present; skipping install-runtime.sh"
    fi
fi

# ---------- step 4: supervisor
step "4. Install supervisor (mneme-supervisor)"
if [ -f "$SCRIPT_DIR/install-supervisor.sh" ]; then
    sh "$SCRIPT_DIR/install-supervisor.sh" \
        || die "install-supervisor.sh failed" 4
else
    log "WARN: install-supervisor.sh not found; skipping"
fi

# ---------- step 5: required models
if [ $SKIP_MODELS -eq 1 ]; then
    log "Skipping models (--skip-models)"
else
    step "5. Install required models (bge-small)"
    if [ -f "$SCRIPT_DIR/install_models.sh" ]; then
        # shellcheck disable=SC2086
        sh "$SCRIPT_DIR/install_models.sh" --required $PASS_THROUGH \
            || die "install_models.sh --required failed" 5
    else
        log "WARN: install_models.sh not found; skipping"
    fi
fi

# ---------- step 6: start daemon
if [ $NO_START -eq 1 ]; then
    log "Skipping daemon start (--no-start)"
else
    step "6. Start mneme daemon"
    if [ -f "$SCRIPT_DIR/start-daemon.sh" ]; then
        sh "$SCRIPT_DIR/start-daemon.sh" \
            || die "start-daemon.sh failed" 6
    else
        log "WARN: start-daemon.sh not found; skipping"
    fi
fi

# ---------- step 7: banner
step "7. Done"
cat <<'BANNER'

   mneme is installed.

   Next:  open Claude Code in your project and run

          /plugin install mneme

   Useful commands:
     sh scripts/check-runtime.sh           # health-check
     sh scripts/start-daemon.sh            # start
     sh scripts/stop-daemon.sh             # stop
     sh scripts/uninstall-runtime.sh       # remove deps mneme installed

BANNER

log "bundle install complete"
exit 0
