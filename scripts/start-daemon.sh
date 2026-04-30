#!/bin/sh
# Start the mneme supervisor daemon (POSIX).
# Uses systemd --user on Linux, launchd on macOS, or a backgrounded
# fallback when no init system is available.
set -eu

MNEME_HOME="${MNEME_HOME:-$HOME/.mneme}"
BIN="$MNEME_HOME/bin/mneme-supervisor"
LOG_DIR="$MNEME_HOME/logs"
PID_FILE="$MNEME_HOME/supervisor.pid"

[ -x "$BIN" ] || { echo "mneme-supervisor not installed at $BIN" >&2; exit 1; }
mkdir -p "$LOG_DIR"

OS="$(uname -s)"
case "$OS" in
    Darwin)
        PLIST="$HOME/Library/LaunchAgents/com.mneme.supervisor.plist"
        if [ -f "$PLIST" ]; then
            launchctl load -w "$PLIST" 2>/dev/null || true
            launchctl start com.mneme.supervisor
            echo "mneme daemon started via launchd"
            exit 0
        fi
        ;;
    Linux)
        if command -v systemctl >/dev/null 2>&1 && \
           systemctl --user list-unit-files mneme.service >/dev/null 2>&1; then
            systemctl --user start mneme.service
            echo "mneme daemon started via systemd --user"
            exit 0
        fi
        ;;
esac

# fallback: background process with PID file
if [ -f "$PID_FILE" ]; then
    OLD_PID="$(cat "$PID_FILE" 2>/dev/null || echo)"
    if [ -n "$OLD_PID" ] && kill -0 "$OLD_PID" 2>/dev/null; then
        echo "mneme daemon already running (pid $OLD_PID)"
        exit 0
    fi
    rm -f "$PID_FILE"
fi

nohup "$BIN" --daemon >> "$LOG_DIR/supervisor.out.log" 2>> "$LOG_DIR/supervisor.err.log" &
echo $! > "$PID_FILE"
echo "mneme daemon started (pid $(cat "$PID_FILE"))"
exit 0
