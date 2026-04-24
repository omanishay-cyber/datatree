#!/usr/bin/env sh
# mneme - one-line installer for macOS / Linux (v0.3.1+)
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/omanishay-cyber/codex/main/scripts/install.sh | sh
#
# What it does, in order (mirrors scripts/install.ps1):
#   0. Stops any running mneme processes (upgrade safety, prevents
#      partial-extract over locked binaries).
#   1. Detects required runtimes (bun / node / git). Prints a clear
#      manual-install hint for each missing tool. Does NOT auto-invoke
#      sudo - a piped-curl installer should never run sudo on Unix.
#   2. Resolves the latest GitHub release asset for the host OS+arch.
#   3. Downloads + extracts to ~/.mneme/ (bin/, mcp/, plugin/).
#   4. (No Windows Defender on Unix - prints a SELinux note for
#      Fedora/Rocky users instead.)
#   5. Adds ~/.mneme/bin to the user's PATH via ~/.profile (Linux)
#      or ~/.zprofile (macOS).
#   6. Starts the mneme daemon in the background and polls daemon
#      status until healthy or 15s timeout.
#   7. Registers the mneme MCP server with Claude Code.
#   8. Prints next steps and verification commands.
#
# Safe to re-run. Every step is idempotent; a step that fails prints a
# clear warning and does not abort the remaining steps (except when
# download / extract themselves fail, which is unrecoverable).
#
# POSIX sh-compatible. ASCII-only output. No emoji.

set -eu

REPO="omanishay-cyber/mneme"
HOME_DIR="${HOME:?HOME not set}"
MNEME_HOME="${HOME_DIR}/.mneme"
BIN_DIR="${MNEME_HOME}/bin"
MNEME_BIN="${BIN_DIR}/mneme"

# ----------------------------------------------------------------------------
# Output helpers (ASCII-only; intentionally plain so it pipes cleanly into
# CI logs and is screen-reader friendly).
# ----------------------------------------------------------------------------

step() { printf '==> %s\n' "$1"; }
info() { printf '    %s\n' "$1"; }
warn() { printf '    warning: %s\n' "$1" >&2; }
ok()   { printf '    ok: %s\n' "$1"; }
fail() { printf '    error: %s\n' "$1" >&2; }

# ----------------------------------------------------------------------------
# Step 0 - Stop any running mneme processes (upgrade safety)
# ----------------------------------------------------------------------------
#
# If a previous mneme daemon / worker is alive, tar will happily overwrite
# the executable inode on Linux/macOS (unlike Windows), but the *running*
# process keeps the old code in memory. The user then thinks they upgraded
# but is still talking to the old binary until the daemon is restarted.
# Easier: just stop everything first. If nothing's running, this is a
# silent no-op.

step "step 0/8 - stop any existing mneme daemon + workers"

# pkill with anchored regex so we don't accidentally kill processes whose
# command line CONTAINS "mneme" but aren't ours (e.g. an editor with a
# mneme path open). `^mneme` only matches argv[0] starting with mneme.
tries=0
while [ "${tries}" -lt 5 ]; do
    if command -v pkill >/dev/null 2>&1; then
        if pkill -f '^mneme' >/dev/null 2>&1; then
            info "sent SIGTERM to mneme process(es); waiting"
            sleep 2
        else
            break
        fi
    else
        # No pkill (rare on minimal containers). Best-effort via the CLI.
        if [ -x "${MNEME_BIN}" ]; then
            "${MNEME_BIN}" daemon stop >/dev/null 2>&1 || true
            sleep 1
        fi
        break
    fi
    tries=$((tries + 1))
done

if command -v pgrep >/dev/null 2>&1 && pgrep -f '^mneme' >/dev/null 2>&1; then
    warn "could not stop all mneme processes - close any 'mneme' shell and rerun"
    warn "tar will still extract on Unix, but running processes keep the old binary in memory"
else
    ok "no mneme processes running - safe to extract"
fi

# ----------------------------------------------------------------------------
# Step 1 - Check runtime prerequisites
# ----------------------------------------------------------------------------
#
# Three tools matter for a full mneme + Claude-Code experience:
#   bun  - mneme's MCP server runtime (`mneme mcp stdio` runs Bun TS).
#   node - only needed for the Claude Code CLI itself (`@anthropic-ai/claude-code`).
#   git  - optional, gives `mneme build` commit SHA metadata.
#
# Adaptation from install.ps1: on Windows we silently install missing
# tools because winget / direct MSI is safe at user scope. On Unix we
# REFUSE to invoke sudo from a piped-curl install. We detect, then print
# the platform-correct manual-install command. The user has full control.

# Detect a tool: PATH first, then platform-standard fallback locations.
# Echoes the resolved path (or empty string if not found).
find_tool() {
    name="$1"
    if command -v "${name}" >/dev/null 2>&1; then
        command -v "${name}"
        return 0
    fi
    # Fallback locations vary by OS.
    case "${UNAME_S}" in
        Darwin)
            for p in \
                "/opt/homebrew/bin/${name}" \
                "/usr/local/bin/${name}" \
                "${HOME_DIR}/.bun/bin/${name}"; do
                if [ -x "${p}" ]; then
                    echo "${p}"
                    return 0
                fi
            done
            ;;
        Linux)
            for p in \
                "/usr/bin/${name}" \
                "/usr/local/bin/${name}" \
                "${HOME_DIR}/.bun/bin/${name}"; do
                if [ -x "${p}" ]; then
                    echo "${p}"
                    return 0
                fi
            done
            ;;
    esac
    echo ""
}

# Detect package manager (used for clearer install hints).
# Echoes one of: brew | apt | dnf | yum | pacman | apk | unknown
detect_pkg_mgr() {
    case "${UNAME_S}" in
        Darwin)
            if command -v brew >/dev/null 2>&1; then echo brew
            else echo unknown; fi
            ;;
        Linux)
            if command -v apt-get >/dev/null 2>&1; then echo apt
            elif command -v dnf      >/dev/null 2>&1; then echo dnf
            elif command -v yum      >/dev/null 2>&1; then echo yum
            elif command -v pacman   >/dev/null 2>&1; then echo pacman
            elif command -v apk      >/dev/null 2>&1; then echo apk
            else echo unknown; fi
            ;;
        *) echo unknown ;;
    esac
}

# Print the manual install hint for a missing tool.
hint_install() {
    tool="$1"
    case "${PKG_MGR}-${tool}" in
        brew-bun)     info "to install: brew install oven-sh/bun/bun" ;;
        brew-node)    info "to install: brew install node" ;;
        brew-git)     info "to install: brew install git" ;;
        apt-bun)      info "to install: curl -fsSL https://bun.sh/install | bash" ;;
        apt-node)     info "to install: sudo apt install nodejs npm" ;;
        apt-git)      info "to install: sudo apt install git" ;;
        dnf-bun)      info "to install: curl -fsSL https://bun.sh/install | bash" ;;
        dnf-node)     info "to install: sudo dnf install nodejs npm" ;;
        dnf-git)      info "to install: sudo dnf install git" ;;
        yum-bun)      info "to install: curl -fsSL https://bun.sh/install | bash" ;;
        yum-node)     info "to install: sudo yum install nodejs npm" ;;
        yum-git)      info "to install: sudo yum install git" ;;
        pacman-bun)   info "to install: sudo pacman -S bun  (or: curl -fsSL https://bun.sh/install | bash)" ;;
        pacman-node)  info "to install: sudo pacman -S nodejs npm" ;;
        pacman-git)   info "to install: sudo pacman -S git" ;;
        apk-bun)      info "to install: curl -fsSL https://bun.sh/install | bash" ;;
        apk-node)     info "to install: sudo apk add nodejs npm" ;;
        apk-git)      info "to install: sudo apk add git" ;;
        *-bun)        info "to install: curl -fsSL https://bun.sh/install | bash" ;;
        *-node)       info "to install: see https://nodejs.org/" ;;
        *-git)        info "to install: see https://git-scm.com/" ;;
    esac
}

# OS detection runs early so Step 0 / Step 1 can use UNAME_S.
UNAME_S=$(uname -s 2>/dev/null || echo unknown)
UNAME_M=$(uname -m 2>/dev/null || echo unknown)
PKG_MGR=$(detect_pkg_mgr)

step "mneme - one-line installer"
info "target   : ${MNEME_HOME}"
info "bin      : ${BIN_DIR}"
info "os       : ${UNAME_S} ${UNAME_M}"
info "pkg mgr  : ${PKG_MGR}"
echo ""

step "step 1/8 - runtime prerequisites (bun / node / git)"

# --- 1a. bun (required for MCP server) -------------------------------------
BUN_PATH=$(find_tool bun)
if [ -n "${BUN_PATH}" ]; then
    BUN_VER=$("${BUN_PATH}" --version 2>/dev/null || echo "?")
    ok "bun ${BUN_VER} present at ${BUN_PATH}"
else
    warn "bun not found - mneme CLI will work, but MCP tools in Claude Code will not"
    hint_install bun
fi

# --- 1b. node (for Claude Code CLI) ----------------------------------------
NODE_PATH=$(find_tool node)
if [ -n "${NODE_PATH}" ]; then
    NODE_VER=$("${NODE_PATH}" --version 2>/dev/null || echo "?")
    ok "node ${NODE_VER} present at ${NODE_PATH}"
else
    warn "node not found - Claude Code CLI cannot be installed until node is present"
    hint_install node
fi

# --- 1c. git (optional) ----------------------------------------------------
GIT_PATH=$(find_tool git)
if [ -n "${GIT_PATH}" ]; then
    GIT_VER=$("${GIT_PATH}" --version 2>/dev/null || echo "?")
    ok "${GIT_VER} present at ${GIT_PATH}"
else
    warn "git not found - mneme will still work, but no commit-SHA metadata in the graph"
    hint_install git
fi

# ----------------------------------------------------------------------------
# Step 2 - Resolve OS+arch -> release asset name and fetch metadata
# ----------------------------------------------------------------------------

step "step 2/8 - fetching latest release metadata"

case "${UNAME_S}" in
    Linux)
        case "${UNAME_M}" in
            x86_64|amd64) ASSET="mneme-linux-x64.tar.gz" ;;
            aarch64|arm64)
                fail "no prebuilt binary for linux/${UNAME_M} yet"
                fail "build from source: https://github.com/${REPO}"
                exit 1
                ;;
            *)
                fail "no prebuilt binary for linux/${UNAME_M}"
                fail "build from source: https://github.com/${REPO}"
                exit 1
                ;;
        esac
        ;;
    Darwin)
        case "${UNAME_M}" in
            arm64) ASSET="mneme-macos-arm64.tar.gz" ;;
            x86_64)
                # Apple Silicon Macs run arm64 natively; Intel Macs can run
                # arm64 binaries under Rosetta 2. We fall back rather than
                # refuse, because the universal experience is nicer than
                # "sorry, Intel Mac unsupported".
                warn "no native x86_64 mac build yet; falling back to arm64 (runs under Rosetta 2)"
                warn "if you don't have Rosetta installed: softwareupdate --install-rosetta --agree-to-license"
                ASSET="mneme-macos-arm64.tar.gz"
                ;;
            *)
                fail "unsupported mac arch: ${UNAME_M}"
                exit 1
                ;;
        esac
        ;;
    MINGW*|MSYS*|CYGWIN*)
        fail "on Windows, use install.ps1 instead:"
        fail "  iwr -useb https://raw.githubusercontent.com/${REPO}/main/scripts/install.ps1 | iex"
        exit 1
        ;;
    *)
        fail "unsupported OS: ${UNAME_S}"
        exit 1
        ;;
esac

info "asset    : ${ASSET}"

# Pick a fetcher. curl preferred; wget fallback for minimal images.
if command -v curl >/dev/null 2>&1; then
    HAVE_CURL=1
elif command -v wget >/dev/null 2>&1; then
    HAVE_CURL=0
else
    fail "neither curl nor wget available - install one and retry"
    exit 1
fi

API_URL="https://api.github.com/repos/${REPO}/releases/latest"

if [ "${HAVE_CURL}" -eq 1 ]; then
    RELEASE_JSON=$(curl -fsSL --retry 3 "${API_URL}") || {
        fail "GitHub API unreachable (curl exit $?)"
        exit 1
    }
else
    RELEASE_JSON=$(wget -qO- "${API_URL}") || {
        fail "GitHub API unreachable (wget failed)"
        exit 1
    }
fi

# Resolve the asset's browser_download_url without requiring jq. Splits on
# commas, then picks the line containing the asset name.
ASSET_URL=$(printf '%s' "${RELEASE_JSON}" \
    | tr ',' '\n' \
    | grep "browser_download_url.*${ASSET}" \
    | head -n1 \
    | sed 's/.*"\(https:[^"]*\)".*/\1/')

if [ -z "${ASSET_URL}" ]; then
    fail "${ASSET} not yet attached to the latest release"
    fail "the release workflow may still be building; retry in ~15 min"
    fail "see: https://github.com/${REPO}/releases"
    exit 1
fi

# Pull the tag for the final summary line. Best-effort; missing tag is fine.
RELEASE_TAG=$(printf '%s' "${RELEASE_JSON}" \
    | tr ',' '\n' \
    | grep '"tag_name"' \
    | head -n1 \
    | sed 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
[ -z "${RELEASE_TAG}" ] && RELEASE_TAG="unknown"

ok "release ${RELEASE_TAG} - asset URL resolved"

# ----------------------------------------------------------------------------
# Step 3 - Download + extract
# ----------------------------------------------------------------------------

step "step 3/8 - downloading + extracting"

TMP=$(mktemp -d 2>/dev/null || mktemp -d -t mneme-install)
# Cleanup on any exit path - success, failure, ^C.
trap 'rm -rf "${TMP}"' EXIT INT TERM

ARCHIVE="${TMP}/${ASSET}"

info "downloading ${ASSET_URL}"
if [ "${HAVE_CURL}" -eq 1 ]; then
    curl -fsSL --retry 3 -o "${ARCHIVE}" "${ASSET_URL}" || {
        fail "download failed (curl exit $?)"
        exit 1
    }
else
    wget -qO "${ARCHIVE}" "${ASSET_URL}" || {
        fail "download failed (wget)"
        exit 1
    }
fi

mkdir -p "${MNEME_HOME}"

info "extracting to ${MNEME_HOME}"
if ! tar -xzf "${ARCHIVE}" -C "${MNEME_HOME}"; then
    fail "extract failed - archive may be corrupt"
    exit 1
fi

# tar preserves modes inside the archive, but be defensive in case the
# release was packed without +x on bin/.
if [ -d "${BIN_DIR}" ]; then
    chmod +x "${BIN_DIR}"/* 2>/dev/null || true
fi

ok "extracted to ${MNEME_HOME}"

# ----------------------------------------------------------------------------
# Step 4 - SELinux note (Unix equivalent of the Windows Defender step)
# ----------------------------------------------------------------------------
#
# Adaptation: there is no Defender on Unix. The closest analogue that
# actually bites users is SELinux on Fedora / Rocky / RHEL in Enforcing
# mode, which can label mneme's per-project sqlite files in a way that
# blocks writes from the user's home directory if it sits on an unusual
# fs (e.g. an SELinux-confined NFS mount). We don't try to fix it from
# this installer - sealert / chcon would need elevated privileges. We
# print a one-line warning so the user knows where to look if writes
# fail later.

step "step 4/8 - sandbox / mandatory access control check"

if command -v getenforce >/dev/null 2>&1; then
    SE_STATE=$(getenforce 2>/dev/null || echo "")
    if [ "${SE_STATE}" = "Enforcing" ]; then
        warn "SELinux is Enforcing. If mneme database writes fail later,"
        warn "see: https://github.com/${REPO}#selinux-faq"
    else
        ok "SELinux not enforcing (${SE_STATE:-not present})"
    fi
else
    ok "no SELinux on this host (skip)"
fi

# ----------------------------------------------------------------------------
# Step 5 - Add bin dir to PATH
# ----------------------------------------------------------------------------
#
# Adaptation: Windows updates the User-scope PATH registry key. On Unix
# we append a one-line `export PATH=...` to the right shell rc file. We
# pick rc files conservatively:
#   - Linux: ~/.profile (sourced by all login shells regardless of bash/zsh/dash)
#   - macOS: ~/.zprofile (macOS defaults to zsh since Catalina)
# If the user runs a non-default shell, they'll see the printed export
# line and can add it themselves.

step "step 5/8 - updating PATH"

case "${UNAME_S}" in
    Darwin) PROFILE_FILE="${HOME_DIR}/.zprofile" ;;
    *)      PROFILE_FILE="${HOME_DIR}/.profile" ;;
esac

# Already on PATH for THIS process? Then nothing to do for this session.
ALREADY_ON_PATH=0
case ":${PATH}:" in
    *":${BIN_DIR}:"*) ALREADY_ON_PATH=1 ;;
esac

# Already in the rc file? Then idempotent on disk.
ALREADY_IN_RC=0
if [ -f "${PROFILE_FILE}" ] && grep -q '\.mneme/bin' "${PROFILE_FILE}" 2>/dev/null; then
    ALREADY_IN_RC=1
fi

if [ "${ALREADY_IN_RC}" -eq 1 ]; then
    ok "${PROFILE_FILE} already exports ~/.mneme/bin"
else
    # Append, preserving an existing newline if present.
    # The literal `$PATH` is intentional - it must be expanded by the
    # user's shell each time the rc is sourced, NOT at install time.
    {
        printf '\n# Added by mneme installer\n'
        # shellcheck disable=SC2016
        printf 'export PATH="%s:$PATH"\n' "${BIN_DIR}"
    } >> "${PROFILE_FILE}"
    ok "appended PATH entry to ${PROFILE_FILE}"
fi

if [ "${ALREADY_ON_PATH}" -eq 0 ]; then
    info "open a new shell to pick up the PATH change, or run:"
    info "  export PATH=\"${BIN_DIR}:\$PATH\""
fi

# ----------------------------------------------------------------------------
# Step 6 - Start the mneme daemon (background, with liveness poll)
# ----------------------------------------------------------------------------
#
# Mirror of install.ps1 step 6. We launch via nohup so the daemon
# survives the shell that ran the installer. stdin <- /dev/null and
# stdout/stderr -> /tmp/mneme-daemon.log so the daemon does not inherit
# our pipe handles (which would hang the curl-piped install if the
# daemon is slow to fork its own process group).
#
# After spawn, we poll `mneme daemon status` every 0.5s for up to 15s.
# Healthy = exit 0 AND output mentions "running" / "healthy" / a pid.
# Unhealthy after 15s -> warn (not fail) so the install completes and
# the user can investigate with `mneme doctor`.

step "step 6/8 - starting mneme daemon"

if [ ! -x "${MNEME_BIN}" ]; then
    warn "mneme binary not found at ${MNEME_BIN} - did extraction succeed?"
    warn "skipping daemon start. run manually later: mneme daemon start"
else
    DAEMON_LOG="/tmp/mneme-daemon.log"
    # nohup + & + redirected fds = fully detached. The shell continues
    # immediately. The daemon log is at /tmp/mneme-daemon.log.
    nohup "${MNEME_BIN}" daemon start </dev/null >"${DAEMON_LOG}" 2>&1 &
    DAEMON_PID=$!
    info "spawned daemon (parent pid ${DAEMON_PID}); polling status..."

    # Poll status. 0.5s intervals up to 15s = 30 attempts.
    waited_ms=0
    healthy=0
    while [ "${waited_ms}" -lt 15000 ]; do
        sleep 1
        # We sleep 1s instead of 0.5s because POSIX sh's sleep doesn't
        # universally accept fractional seconds. This still gives us
        # 15 polls inside the 15s budget.
        waited_ms=$((waited_ms + 1000))
        STATUS_OUT=$("${MNEME_BIN}" daemon status 2>&1 || true)
        case "${STATUS_OUT}" in
            *running*|*healthy*|*'"pid"'*)
                healthy=1
                break
                ;;
        esac
    done

    if [ "${healthy}" -eq 1 ]; then
        ok "daemon started"
    else
        warn "daemon did not report healthy within 15s - it may still be coming up"
        warn "check later: mneme doctor; daemon log at ${DAEMON_LOG}"
    fi
fi

# ----------------------------------------------------------------------------
# Step 7 - Register MCP with Claude Code
# ----------------------------------------------------------------------------
#
# Same v0.3.1 hard rule as the Windows installer: only writes
# mcpServers.mneme into ~/.claude.json. Does NOT touch
# ~/.claude/settings.json. Does NOT inject hooks. Does NOT write a
# CLAUDE.md manifest.

step "step 7/8 - registering MCP with Claude Code"

if [ ! -x "${MNEME_BIN}" ]; then
    warn "mneme binary not present - skipping MCP registration"
else
    if "${MNEME_BIN}" register-mcp --platform claude-code 2>&1 | while IFS= read -r line; do
        info "${line}"
    done; then
        ok "Claude Code MCP registration complete"
    else
        warn "register-mcp exited non-zero - MCP may not be registered"
        warn "run manually later: mneme register-mcp --platform claude-code"
    fi
fi

# ----------------------------------------------------------------------------
# Step 8 - Done
# ----------------------------------------------------------------------------

step "step 8/8 - complete"
echo ""
echo "================================================================"
echo "  mneme installed - ${RELEASE_TAG}"
echo "================================================================"
echo ""
echo "  Next steps:"
echo "    1. Restart Claude Code so it picks up the new MCP server"
echo "    2. Open a project directory and run: mneme build ."
echo "    3. Inside Claude Code, try:  /mn-recall \"what does auth do\""
echo ""
echo "  Verify:"
echo "    mneme daemon status"
echo "    mneme --version"
echo ""
echo "  Uninstall:"
echo "    mneme unregister-mcp --platform claude-code"
echo "    sh ${MNEME_HOME}/scripts/uninstall.sh   (or rm -rf ${MNEME_HOME})"
echo ""
if [ "${ALREADY_ON_PATH}" -eq 0 ]; then
    echo "  Open a NEW shell to pick up the PATH change."
    echo ""
fi

exit 0
