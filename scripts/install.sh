#!/usr/bin/env sh
# Mneme — one-line installer
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/omanishay-cyber/mneme/main/scripts/install.sh | sh
#
# What it does:
#   1. Detects your OS + CPU arch.
#   2. Downloads the matching mneme-<platform>.{tar.gz,zip} from the latest
#      GitHub release.
#   3. Extracts into ~/.mneme/ (bin/, mcp/, plugin/).
#   4. Prints the one-shot PATH export and the next command.
#
# Safe to re-run: overwrites existing ~/.mneme/bin cleanly.
# Exits non-zero on failure; never touches anything outside $HOME.

set -eu

REPO="omanishay-cyber/mneme"
HOME_DIR="${HOME:?HOME not set}"
MNEME_HOME="${HOME_DIR}/.mneme"

# ---------- platform detection ----------

uname_s=$(uname -s 2>/dev/null || echo unknown)
uname_m=$(uname -m 2>/dev/null || echo unknown)

case "${uname_s}" in
  Linux)
    case "${uname_m}" in
      x86_64|amd64)
        ASSET="mneme-linux-x64.tar.gz"
        ;;
      *)
        echo "mneme: no prebuilt binary for linux/${uname_m}." >&2
        echo "       install from source: https://github.com/${REPO}" >&2
        exit 1
        ;;
    esac
    ;;
  Darwin)
    # Both arm64 and x64 use the arm64 build (Intel Macs run under Rosetta 2).
    ASSET="mneme-macos-arm64.tar.gz"
    ;;
  MINGW*|MSYS*|CYGWIN*)
    echo "mneme: on Windows, use install.ps1 instead:" >&2
    echo "       iwr -useb https://raw.githubusercontent.com/${REPO}/main/scripts/install.ps1 | iex" >&2
    exit 1
    ;;
  *)
    echo "mneme: unsupported OS: ${uname_s}" >&2
    exit 1
    ;;
esac

echo "mneme: detected ${uname_s} ${uname_m} -> ${ASSET}"

# ---------- download ----------

API_URL="https://api.github.com/repos/${REPO}/releases/latest"
echo "mneme: fetching latest release metadata from ${API_URL}"

# Try curl first, wget as a fallback. Both are ubiquitous.
if command -v curl >/dev/null 2>&1; then
  FETCH="curl -fsSL --retry 3"
elif command -v wget >/dev/null 2>&1; then
  FETCH="wget -qO-"
else
  echo "mneme: neither curl nor wget available. Install one and retry." >&2
  exit 1
fi

# Resolve the asset's browser_download_url from the release JSON without
# requiring jq (portable to minimal systems).
RELEASE_JSON=$(${FETCH} "${API_URL}")
ASSET_URL=$(echo "${RELEASE_JSON}" | \
  tr ',' '\n' | \
  grep "browser_download_url.*${ASSET}" | \
  head -n1 | \
  sed 's/.*"\(https:[^"]*\)".*/\1/')

if [ -z "${ASSET_URL}" ]; then
  echo "mneme: ${ASSET} not found in the latest release." >&2
  echo "       the release workflow may still be building — retry in ~15 min" >&2
  echo "       see: https://github.com/${REPO}/releases" >&2
  exit 1
fi

echo "mneme: downloading ${ASSET_URL}"
TMPDIR=$(mktemp -d)
trap 'rm -rf "${TMPDIR}"' EXIT INT TERM

if command -v curl >/dev/null 2>&1; then
  curl -fsSL --retry 3 -o "${TMPDIR}/${ASSET}" "${ASSET_URL}"
else
  wget -qO "${TMPDIR}/${ASSET}" "${ASSET_URL}"
fi

# ---------- extract ----------

echo "mneme: extracting to ${MNEME_HOME}"
mkdir -p "${MNEME_HOME}"
tar -xzf "${TMPDIR}/${ASSET}" -C "${MNEME_HOME}"

# Ensure bin/ is executable.
if [ -d "${MNEME_HOME}/bin" ]; then
  chmod +x "${MNEME_HOME}/bin"/* 2>/dev/null || true
fi

# ---------- PATH hint ----------

BIN_DIR="${MNEME_HOME}/bin"

case ":${PATH}:" in
  *":${BIN_DIR}:"*)
    ;;  # already in PATH
  *)
    echo ""
    echo "mneme: add to your PATH so \`mneme\` is reachable:"
    echo "       export PATH=\"${BIN_DIR}:\$PATH\""
    echo ""
    # Best-effort: append to common rc files if they exist.
    for rc in "${HOME_DIR}/.bashrc" "${HOME_DIR}/.zshrc" "${HOME_DIR}/.profile"; do
      if [ -f "${rc}" ] && ! grep -q "\.mneme/bin" "${rc}" 2>/dev/null; then
        printf '\n# Added by mneme installer\nexport PATH="%s:$PATH"\n' "${BIN_DIR}" >> "${rc}"
        echo "mneme: added PATH entry to ${rc}"
      fi
    done
    ;;
esac

# ---------- done ----------

echo ""
echo "mneme: installed at ${MNEME_HOME}"
echo "mneme: next:"
echo "  1. mneme-daemon start          # start the supervisor"
echo "  2. mneme build .               # index this project"
echo "  3. mneme install               # register with your AI tool"
echo ""
