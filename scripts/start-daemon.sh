#!/bin/sh
# Start the datatree supervisor daemon (POSIX).
# Uses systemd --user on Linux, launchd on macOS, or a backgrounded
# fallback when no init system is available.
set -eu

DATATREE_HOME="${DATATREE_HOME:-$HOME/.datatree}"
BIN="$DATATREE_HOME/bin/datatree-supervisor"
LOG_DIR="$DATATREE_HOME/logs"
PID_FILE="$DATATREE_HOME/supervisor.pid"

[ -x "$BIN" ] || { echo "datatree-supervisor not installed at $BIN" >&2; exit 1; }
mkdir -p "$LOG_DIR"

OS="$(uname -s)"
case "$OS" in
    Darwin)
        PLIST="$HOME/Library/LaunchAgents/com.datatree.supervisor.plist"
        if [ -f "$PLIST" ]; then
            launchctl load -w "$PLIST" 2>/dev/null || true
            launchctl start com.datatree.supervisor
            echo "datatree daemon started via launchd"
            exit 0
        fi
        ;;
    Linux)
        if command -v systemctl >/dev/null 2>&1 && \
           systemctl --user list-unit-files datatree.service >/dev/null 2>&1; then
            systemctl --user start datatree.service
            echo "datatree daemon started via systemd --user"
            exit 0
        fi
        ;;
esac

# fallback: background process with PID file
if [ -f "$PID_FILE" ]; then
    OLD_PID="$(cat "$PID_FILE" 2>/dev/null || echo)"
    if [ -n "$OLD_PID" ] && kill -0 "$OLD_PID" 2>/dev/null; then
        echo "datatree daemon already running (pid $OLD_PID)"
        exit 0
    fi
    rm -f "$PID_FILE"
fi

nohup "$BIN" --daemon >> "$LOG_DIR/supervisor.out.log" 2>> "$LOG_DIR/supervisor.err.log" &
echo $! > "$PID_FILE"
echo "datatree daemon started (pid $(cat "$PID_FILE"))"
exit 0
