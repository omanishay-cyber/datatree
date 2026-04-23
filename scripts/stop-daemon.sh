#!/bin/sh
# Stop the datatree supervisor daemon.
set -eu

DATATREE_HOME="${DATATREE_HOME:-$HOME/.datatree}"
PID_FILE="$DATATREE_HOME/supervisor.pid"

OS="$(uname -s)"
case "$OS" in
    Darwin)
        PLIST="$HOME/Library/LaunchAgents/com.datatree.supervisor.plist"
        if [ -f "$PLIST" ]; then
            launchctl stop com.datatree.supervisor 2>/dev/null || true
            echo "datatree daemon stopped (launchd)"
            exit 0
        fi
        ;;
    Linux)
        if command -v systemctl >/dev/null 2>&1 && \
           systemctl --user list-unit-files datatree.service >/dev/null 2>&1; then
            systemctl --user stop datatree.service
            echo "datatree daemon stopped (systemd --user)"
            exit 0
        fi
        ;;
esac

if [ -f "$PID_FILE" ]; then
    PID="$(cat "$PID_FILE" 2>/dev/null || echo)"
    if [ -n "$PID" ] && kill -0 "$PID" 2>/dev/null; then
        kill "$PID" 2>/dev/null || true
        # graceful, then force
        i=0
        while [ "$i" -lt 10 ] && kill -0 "$PID" 2>/dev/null; do
            sleep 1
            i=$((i + 1))
        done
        if kill -0 "$PID" 2>/dev/null; then
            kill -9 "$PID" 2>/dev/null || true
        fi
        echo "datatree daemon stopped (pid $PID)"
    fi
    rm -f "$PID_FILE"
else
    echo "datatree daemon not running"
fi
exit 0
