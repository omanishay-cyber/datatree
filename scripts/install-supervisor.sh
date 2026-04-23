#!/bin/sh
# datatree supervisor installer (POSIX)
# Installs datatree-supervisor binary to ~/.mneme/bin/ and registers it
# with the OS service manager (launchd on macOS, systemd --user on Linux).
#
# Idempotent: re-running will not duplicate entries. Existing files are
# backed up to <name>.bak before being overwritten.
#
# Usage:
#   install-supervisor.sh [--binary-path <path>] [--source-dir <path>] [--quiet]
#
# Defaults:
#   --source-dir is ./dist/supervisor (relative to repo root)
#   binary name resolved as datatree-supervisor-<os>-<arch>

set -eu

QUIET=0
BINARY_PATH=""
SOURCE_DIR=""
MNEME_HOME="${MNEME_HOME:-$HOME/.datatree}"
BIN_DIR="$MNEME_HOME/bin"
LOG_DIR="$MNEME_HOME/logs"
MARKER_VERSION="v1.0"

log() {
    [ "$QUIET" -eq 0 ] && printf '[datatree-install] %s\n' "$1"
}

die() {
    printf '[datatree-install] ERROR: %s\n' "$1" >&2
    exit 1
}

# --- arg parsing --------------------------------------------------------------
while [ $# -gt 0 ]; do
    case "$1" in
        --binary-path)
            shift
            [ $# -gt 0 ] || die "--binary-path requires a value"
            BINARY_PATH="$1"
            ;;
        --source-dir)
            shift
            [ $# -gt 0 ] || die "--source-dir requires a value"
            SOURCE_DIR="$1"
            ;;
        --quiet)
            QUIET=1
            ;;
        -h|--help)
            sed -n '2,15p' "$0"
            exit 0
            ;;
        *)
            die "Unknown argument: $1"
            ;;
    esac
    shift
done

# --- detect OS / arch ---------------------------------------------------------
OS_RAW="$(uname -s)"
case "$OS_RAW" in
    Linux*)  OS=linux ;;
    Darwin*) OS=darwin ;;
    *)       die "Unsupported OS: $OS_RAW (use install-supervisor.ps1 on Windows)" ;;
esac

ARCH_RAW="$(uname -m)"
case "$ARCH_RAW" in
    x86_64|amd64)        ARCH=x86_64 ;;
    aarch64|arm64)       ARCH=aarch64 ;;
    *)                   die "Unsupported architecture: $ARCH_RAW" ;;
esac

log "Detected platform: ${OS}/${ARCH}"

# --- resolve binary -----------------------------------------------------------
if [ -z "$BINARY_PATH" ]; then
    if [ -z "$SOURCE_DIR" ]; then
        SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
        SOURCE_DIR="$SCRIPT_DIR/../dist/supervisor"
    fi
    BINARY_PATH="$SOURCE_DIR/datatree-supervisor-${OS}-${ARCH}"
fi

[ -f "$BINARY_PATH" ] || die "Binary not found: $BINARY_PATH"
[ -r "$BINARY_PATH" ] || die "Binary not readable: $BINARY_PATH"

# --- prepare directories ------------------------------------------------------
log "Creating $MNEME_HOME"
mkdir -p "$BIN_DIR"
mkdir -p "$LOG_DIR"
mkdir -p "$MNEME_HOME/projects"
mkdir -p "$MNEME_HOME/cache"
mkdir -p "$MNEME_HOME/models"

# --- install binary -----------------------------------------------------------
DEST="$BIN_DIR/datatree-supervisor"
if [ -f "$DEST" ]; then
    log "Backing up existing binary to ${DEST}.bak"
    cp -p "$DEST" "${DEST}.bak"
fi

log "Installing binary to $DEST"
cp -p "$BINARY_PATH" "$DEST"
chmod 755 "$DEST"

# --- register with service manager --------------------------------------------
register_launchd() {
    PLIST_DIR="$HOME/Library/LaunchAgents"
    PLIST="$PLIST_DIR/com.datatree.supervisor.plist"
    mkdir -p "$PLIST_DIR"

    if [ -f "$PLIST" ]; then
        log "Backing up existing plist to ${PLIST}.bak"
        cp -p "$PLIST" "${PLIST}.bak"
        launchctl unload "$PLIST" 2>/dev/null || true
    fi

    log "Writing launchd plist: $PLIST"
    cat > "$PLIST" <<PLIST_EOF
<?xml version="1.0" encoding="UTF-8"?>
<!-- datatree-marker ${MARKER_VERSION} -->
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
  <dict>
    <key>Label</key><string>com.datatree.supervisor</string>
    <key>ProgramArguments</key>
    <array>
      <string>${DEST}</string>
      <string>--daemon</string>
    </array>
    <key>RunAtLoad</key><true/>
    <key>KeepAlive</key><true/>
    <key>StandardOutPath</key><string>${LOG_DIR}/supervisor.out.log</string>
    <key>StandardErrorPath</key><string>${LOG_DIR}/supervisor.err.log</string>
    <key>EnvironmentVariables</key>
    <dict>
      <key>MNEME_HOME</key><string>${MNEME_HOME}</string>
    </dict>
  </dict>
</plist>
PLIST_EOF

    launchctl load "$PLIST"
    log "launchd service registered: com.datatree.supervisor"
}

register_systemd() {
    UNIT_DIR="$HOME/.config/systemd/user"
    UNIT="$UNIT_DIR/datatree.service"
    mkdir -p "$UNIT_DIR"

    if [ -f "$UNIT" ]; then
        log "Backing up existing unit to ${UNIT}.bak"
        cp -p "$UNIT" "${UNIT}.bak"
    fi

    log "Writing systemd unit: $UNIT"
    cat > "$UNIT" <<UNIT_EOF
# datatree-marker ${MARKER_VERSION}
[Unit]
Description=Datatree Supervisor (per-user knowledge graph daemon)
After=default.target

[Service]
Type=simple
ExecStart=${DEST} --daemon
Restart=on-failure
RestartSec=3
Environment=MNEME_HOME=${MNEME_HOME}
StandardOutput=append:${LOG_DIR}/supervisor.out.log
StandardError=append:${LOG_DIR}/supervisor.err.log

[Install]
WantedBy=default.target
UNIT_EOF

    if command -v systemctl >/dev/null 2>&1; then
        systemctl --user daemon-reload
        systemctl --user enable datatree.service
        log "systemd --user service registered: datatree.service"
    else
        log "systemctl not found; unit written but not enabled"
    fi
}

case "$OS" in
    darwin) register_launchd ;;
    linux)  register_systemd ;;
esac

log "Install complete. Run scripts/start-daemon.sh to launch."
exit 0
