#!/bin/sh
# Uninstall datatree binaries + service registrations.
# KEEPS user data ($DATATREE_HOME/projects, /cache, /models) by default.
# Pass --purge to also delete user data.
set -eu

PURGE=0
for arg in "$@"; do
    case "$arg" in
        --purge) PURGE=1 ;;
        -h|--help)
            echo "Usage: uninstall.sh [--purge]"
            echo "  --purge  also delete projects/, cache/, models/ data"
            exit 0
            ;;
        *)
            echo "Unknown argument: $arg" >&2
            exit 1
            ;;
    esac
done

DATATREE_HOME="${DATATREE_HOME:-$HOME/.datatree}"
BIN_DIR="$DATATREE_HOME/bin"

# stop daemon first
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
if [ -x "$SCRIPT_DIR/stop-daemon.sh" ]; then
    sh "$SCRIPT_DIR/stop-daemon.sh" || true
fi

OS="$(uname -s)"
case "$OS" in
    Darwin)
        PLIST="$HOME/Library/LaunchAgents/com.datatree.supervisor.plist"
        if [ -f "$PLIST" ]; then
            launchctl unload "$PLIST" 2>/dev/null || true
            rm -f "$PLIST"
            echo "Removed launchd plist"
        fi
        ;;
    Linux)
        UNIT="$HOME/.config/systemd/user/datatree.service"
        if [ -f "$UNIT" ]; then
            if command -v systemctl >/dev/null 2>&1; then
                systemctl --user disable datatree.service 2>/dev/null || true
                systemctl --user stop    datatree.service 2>/dev/null || true
            fi
            rm -f "$UNIT"
            command -v systemctl >/dev/null 2>&1 && systemctl --user daemon-reload || true
            echo "Removed systemd unit"
        fi
        ;;
esac

if [ -d "$BIN_DIR" ]; then
    rm -rf "$BIN_DIR"
    echo "Removed $BIN_DIR"
fi

# logs are state-ish — remove them, they re-create on next install
rm -rf "$DATATREE_HOME/logs" 2>/dev/null || true
rm -f  "$DATATREE_HOME/supervisor.pid" 2>/dev/null || true

if [ "$PURGE" -eq 1 ]; then
    echo "WARNING: --purge will delete $DATATREE_HOME/projects, /cache, /models"
    rm -rf "$DATATREE_HOME/projects" "$DATATREE_HOME/cache" "$DATATREE_HOME/models"
    rmdir  "$DATATREE_HOME" 2>/dev/null || true
    echo "User data purged"
else
    echo "User data preserved at $DATATREE_HOME (projects/, cache/, models/)"
    echo "Run 'uninstall.sh --purge' to delete it."
fi
exit 0
