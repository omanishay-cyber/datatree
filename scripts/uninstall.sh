#!/usr/bin/env sh
# mneme - uninstaller for macOS / Linux (v0.3.1+)
#
# Usage:
#   sh ~/.mneme/scripts/uninstall.sh                  # default: remove everything
#   sh ~/.mneme/scripts/uninstall.sh --keep-data      # keep ~/.mneme/projects/
#   sh ~/.mneme/scripts/uninstall.sh -h | --help
#
# What it does, in order (mirrors scripts/uninstall.ps1):
#   1. Verifies ~/.mneme/ exists; exits silently if not.
#   2. Stops the daemon (idempotent).
#   3. Waits up to 5s for processes to exit.
#   4. Prints the `mneme rollback --list` hint - the user should look
#      at this before nuking ~/.mneme/, in case they want to reverse a
#      single install instead.
#   5. Runs `mneme unregister-mcp --platform claude-code` plus any
#      other platforms detected via on-disk MCP config files.
#   6. Strips the `~/.mneme/bin` PATH line from ~/.profile and ~/.zprofile.
#   7. By default: rm -rf ~/.mneme/.
#      With --keep-data: keeps ~/.mneme/projects/ so re-install preserves
#      indexed knowledge.
#   8. Prints a final summary of what was removed.
#
# POSIX sh-compatible. ASCII-only output. No emoji. Idempotent.

set -eu

KEEP_DATA=0

for arg in "$@"; do
    case "$arg" in
        --keep-data)
            KEEP_DATA=1
            ;;
        -h|--help)
            cat <<'EOF'
mneme uninstaller

Usage:
  uninstall.sh                # remove ~/.mneme/ entirely (default)
  uninstall.sh --keep-data    # remove binaries but keep ~/.mneme/projects/

Steps:
  1. stop daemon
  2. unregister MCP from detected platforms (claude-code, cursor, copilot...)
  3. strip PATH entry from ~/.profile and ~/.zprofile
  4. rm -rf ~/.mneme/   (or just bin/ + cache/ if --keep-data)
EOF
            exit 0
            ;;
        *)
            echo "uninstall.sh: unknown argument '${arg}'" >&2
            echo "see: uninstall.sh --help" >&2
            exit 1
            ;;
    esac
done

HOME_DIR="${HOME:?HOME not set}"
MNEME_HOME="${MNEME_HOME:-${HOME_DIR}/.mneme}"
BIN_DIR="${MNEME_HOME}/bin"
MNEME_BIN="${BIN_DIR}/mneme"

step() { printf '==> %s\n' "$1"; }
info() { printf '    %s\n' "$1"; }
warn() { printf '    warning: %s\n' "$1" >&2; }
ok()   { printf '    ok: %s\n' "$1"; }

# ----------------------------------------------------------------------------
# 1. Verify mneme is actually installed
# ----------------------------------------------------------------------------
#
# Silent exit 0 if not - re-running uninstall after a successful uninstall
# should not error.

if [ ! -d "${MNEME_HOME}" ]; then
    # No output. Nothing to do, nothing to report. Matches the
    # uninstall.ps1 contract.
    exit 0
fi

step "mneme uninstaller"
info "target   : ${MNEME_HOME}"
info "mode     : $([ "${KEEP_DATA}" -eq 1 ] && echo 'keep-data (preserve projects/)' || echo 'full (remove everything)')"
echo ""

# ----------------------------------------------------------------------------
# 2. Stop the daemon
# ----------------------------------------------------------------------------

step "stopping mneme daemon"

if [ -x "${MNEME_BIN}" ]; then
    # `daemon stop` returns 0 even if the daemon isn't running (the IPC
    # error is caught and treated as "already stopped" - see
    # cli/src/commands/daemon.rs). So this is always safe.
    "${MNEME_BIN}" daemon stop >/dev/null 2>&1 || true
    ok "daemon stop requested"
else
    info "mneme binary missing - skipping graceful stop"
fi

# Belt-and-suspenders: SIGTERM any survivors, since daemon stop may have
# raced with mid-flight workers.
if command -v pkill >/dev/null 2>&1; then
    pkill -f '^mneme' >/dev/null 2>&1 || true
fi

# ----------------------------------------------------------------------------
# 3. Wait for processes to exit (up to 5s)
# ----------------------------------------------------------------------------

step "waiting for processes to exit"

waited=0
while [ "${waited}" -lt 5 ]; do
    if command -v pgrep >/dev/null 2>&1; then
        if ! pgrep -f '^mneme' >/dev/null 2>&1; then
            break
        fi
    else
        break
    fi
    sleep 1
    waited=$((waited + 1))
done

if command -v pgrep >/dev/null 2>&1 && pgrep -f '^mneme' >/dev/null 2>&1; then
    warn "some mneme processes are still alive after 5s"
    warn "you may need to kill -9 them manually:"
    warn "  pgrep -af '^mneme'"
else
    ok "all mneme processes stopped"
fi

# ----------------------------------------------------------------------------
# 4. Print rollback hint - this is BEFORE we destroy receipts
# ----------------------------------------------------------------------------
#
# The `mneme rollback` command can reverse a single install using its
# receipt, which is more surgical than nuking ~/.mneme/. We print this
# hint before continuing so the user has a chance to ^C and run rollback
# instead. Receipts live at ~/.mneme/install-receipts/ - they go away
# when we remove ~/.mneme/.

step "rollback receipts (informational)"

if [ -d "${MNEME_HOME}/install-receipts" ] && [ -x "${MNEME_BIN}" ]; then
    info "before removing ~/.mneme/, you may want to inspect existing"
    info "install receipts (an install can be reversed surgically):"
    info ""
    info "  mneme rollback --list"
    info ""
fi

# ----------------------------------------------------------------------------
# 5. Unregister MCP from every detected host
# ----------------------------------------------------------------------------
#
# claude-code is always attempted (matches uninstall.ps1). For other
# platforms we check whether the user has an on-disk MCP config file
# (~/.claude.json, ~/.cursor/mcp.json, etc) - if it exists, the user
# probably has mneme registered there and we should unregister.
# unregister-mcp itself is idempotent and key-scoped (it only removes
# mcpServers.mneme), so a redundant call is harmless.

step "unregistering MCP from detected platforms"

if [ ! -x "${MNEME_BIN}" ]; then
    warn "mneme binary missing - skipping MCP unregistration"
    warn "you may want to manually edit ~/.claude.json / ~/.cursor/mcp.json"
else
    # Always try claude-code (the default).
    if "${MNEME_BIN}" unregister-mcp --platform claude-code >/dev/null 2>&1; then
        ok "unregistered from claude-code"
    else
        info "claude-code unregister: nothing to do (or platform not configured)"
    fi

    # Detect-and-unregister for the rest. Each platform is opt-in based
    # on the presence of its config file - we don't want to surprise
    # users who never used Cursor with a "removed Cursor MCP entry"
    # log line.
    if [ -f "${HOME_DIR}/.cursor/mcp.json" ]; then
        if "${MNEME_BIN}" unregister-mcp --platform cursor >/dev/null 2>&1; then
            ok "unregistered from cursor"
        fi
    fi
    if [ -f "${HOME_DIR}/.config/github-copilot/mcp.json" ]; then
        if "${MNEME_BIN}" unregister-mcp --platform copilot >/dev/null 2>&1; then
            ok "unregistered from github-copilot"
        fi
    fi
    if [ -f "${HOME_DIR}/.factory/mcp.json" ]; then
        if "${MNEME_BIN}" unregister-mcp --platform factory-droid >/dev/null 2>&1; then
            ok "unregistered from factory-droid"
        fi
    fi
    if [ -f "${HOME_DIR}/.hermes/mcp.json" ]; then
        if "${MNEME_BIN}" unregister-mcp --platform hermes >/dev/null 2>&1; then
            ok "unregistered from hermes"
        fi
    fi
fi

# ----------------------------------------------------------------------------
# 6. Strip ~/.mneme/bin export from ~/.profile and ~/.zprofile
# ----------------------------------------------------------------------------
#
# Match install.sh's exact "Added by mneme installer\nexport PATH=..." block.
# Use sed to remove both lines if present. We also handle the legacy
# install.sh layout that wrote into ~/.bashrc / ~/.zshrc, in case the user
# upgraded from an older mneme.

strip_path_entry() {
    rc="$1"
    [ -f "${rc}" ] || return 0
    if ! grep -q '\.mneme/bin' "${rc}" 2>/dev/null; then
        return 0
    fi
    # Make a backup once per run, in case the user's rc has more in it
    # than the mneme block.
    cp "${rc}" "${rc}.mneme-uninstall.bak"
    # Two-line block removal: the comment "Added by mneme installer" plus
    # the very next export line that mentions .mneme/bin. We also
    # tolerate whitespace and rm any standalone line.
    sed -i.tmp \
        -e '/^# Added by mneme installer$/d' \
        -e '/^export PATH="[^"]*\.mneme\/bin[^"]*"$/d' \
        "${rc}"
    rm -f "${rc}.tmp"
    info "stripped PATH entry from ${rc} (backup: ${rc}.mneme-uninstall.bak)"
}

step "removing PATH entries from shell rc files"

strip_path_entry "${HOME_DIR}/.profile"
strip_path_entry "${HOME_DIR}/.zprofile"
# Legacy locations from earlier install.sh versions:
strip_path_entry "${HOME_DIR}/.bashrc"
strip_path_entry "${HOME_DIR}/.zshrc"

# ----------------------------------------------------------------------------
# 7. Remove ~/.mneme/ (or just non-data subdirs if --keep-data)
# ----------------------------------------------------------------------------

step "removing files"

REMOVED_BIN=0
REMOVED_DATA=0

if [ "${KEEP_DATA}" -eq 1 ]; then
    # Keep ~/.mneme/projects/ (and any user-edited config). Just remove
    # binaries, caches, runtime files.
    for sub in bin mcp plugin scripts logs cache models install-receipts; do
        path="${MNEME_HOME}/${sub}"
        if [ -e "${path}" ]; then
            rm -rf "${path}"
            REMOVED_BIN=1
        fi
    done
    rm -f "${MNEME_HOME}/supervisor.pid" 2>/dev/null || true
    if [ "${REMOVED_BIN}" -eq 1 ]; then
        ok "removed binaries / caches / logs (kept projects/)"
    fi
    info "user data preserved at ${MNEME_HOME}/projects/"
else
    # Full removal.
    if [ -d "${MNEME_HOME}" ]; then
        rm -rf "${MNEME_HOME}"
        REMOVED_BIN=1
        REMOVED_DATA=1
        ok "removed ${MNEME_HOME}"
    fi
fi

# ----------------------------------------------------------------------------
# 8. Final summary
# ----------------------------------------------------------------------------

echo ""
step "uninstall complete"
echo ""
echo "  removed:"
[ "${REMOVED_BIN}"  -eq 1 ] && echo "    - mneme binaries (bin/, mcp/, plugin/, scripts/)"
[ "${REMOVED_DATA}" -eq 1 ] && echo "    - mneme data (projects/, cache/, models/)"
echo "    - PATH entry from ~/.profile / ~/.zprofile (if present)"
echo "    - MCP entries from detected platform configs"
echo ""

if [ "${KEEP_DATA}" -eq 1 ]; then
    echo "  preserved:"
    echo "    - ${MNEME_HOME}/projects/   (your indexed projects)"
    echo ""
    echo "  to remove the rest:"
    echo "    rm -rf ${MNEME_HOME}"
    echo ""
fi

echo "  if a shell still has ~/.mneme/bin on PATH, open a new one."
echo ""

exit 0
