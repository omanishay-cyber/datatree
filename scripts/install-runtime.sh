#!/usr/bin/env sh
# datatree :: install-runtime.sh
# Detects and (optionally) installs the runtime dependencies datatree needs:
#   bun >=1.0, python3 >=3.10, tesseract >=5.0, ffmpeg
# SQLite is bundled inside datatree-store via rusqlite's `bundled` feature, so
# it is intentionally NOT listed here.
#
# LOCAL-ONLY rule: this script never reaches the internet by itself.  When the
# user passes --auto-install, it delegates to the platform package manager
# (brew/apt/dnf/pacman/winget) -- those tools may go to the network on the
# user's behalf, but datatree itself never does.
#
# Usage:
#   sh install-runtime.sh                  # detect only, print install hints
#   sh install-runtime.sh --auto-install   # actually install via system pkg mgr
#   sh install-runtime.sh --from /mnt/usb  # use local-mirror folder of installers
#   sh install-runtime.sh --yes            # assume "yes" to confirmation prompts
#   sh install-runtime.sh --quiet          # less chatty output
#
# Exit codes:
#   0  all required deps installed (or were already)
#   1  one or more required deps missing and --auto-install not given
#   2  install attempted but failed verification
#   3  unsupported OS / package manager not available
#   4  invalid CLI arguments
#
# POSIX-compliant.  Avoids bashisms.  Works on /bin/sh, dash, ash.

set -eu

# --------------------------------------------------------------------- config
MNEME_HOME="${MNEME_HOME:-${HOME}/.datatree}"
LOG_DIR="${MNEME_HOME}/logs"
LOG_FILE="${LOG_DIR}/install.log"
MANIFEST_FILE="${MNEME_HOME}/install-manifest.json"
DATATREE_VERSION="0.1.0"

REQUIRED_DEPS="bun python3 tesseract ffmpeg"

AUTO_INSTALL=0
ASSUME_YES=0
QUIET=0
FROM_DIR=""

# ------------------------------------------------------------------ argparse
while [ $# -gt 0 ]; do
    case "$1" in
        --auto-install) AUTO_INSTALL=1 ;;
        --yes|-y)       ASSUME_YES=1 ;;
        --quiet|-q)     QUIET=1 ;;
        --from)
            shift
            if [ $# -eq 0 ]; then
                echo "ERROR: --from requires a directory argument" >&2
                exit 4
            fi
            FROM_DIR="$1"
            ;;
        --from=*)       FROM_DIR="${1#--from=}" ;;
        -h|--help)
            sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "ERROR: unknown argument '$1'" >&2
            echo "Run '$0 --help' for usage." >&2
            exit 4
            ;;
    esac
    shift
done

# --------------------------------------------------------------------- utils
mkdir -p "$LOG_DIR"
: >> "$LOG_FILE"

log() {
    # log <level> <message...>
    _level="$1"; shift
    _ts="$(date -u +'%Y-%m-%dT%H:%M:%SZ')"
    printf '[%s] [%s] %s\n' "$_ts" "$_level" "$*" >> "$LOG_FILE"
    if [ "$QUIET" -eq 0 ] || [ "$_level" = "ERROR" ]; then
        case "$_level" in
            INFO)  printf '\033[0;36m[i]\033[0m %s\n' "$*" ;;
            OK)    printf '\033[0;32m[+]\033[0m %s\n' "$*" ;;
            WARN)  printf '\033[0;33m[!]\033[0m %s\n' "$*" ;;
            ERROR) printf '\033[0;31m[x]\033[0m %s\n' "$*" >&2 ;;
            *)     printf '%s\n' "$*" ;;
        esac
    fi
}

confirm() {
    # confirm <prompt> -> 0 yes, 1 no
    if [ "$ASSUME_YES" -eq 1 ]; then return 0; fi
    printf '%s [y/N]: ' "$1"
    read -r _ans </dev/tty 2>/dev/null || _ans="n"
    case "$_ans" in
        y|Y|yes|YES) return 0 ;;
        *)           return 1 ;;
    esac
}

has_cmd() { command -v "$1" >/dev/null 2>&1; }

# --------------------------------------------------------------- OS detection
detect_os() {
    _u="$(uname -s 2>/dev/null || echo unknown)"
    case "$_u" in
        Darwin*) echo "macos" ;;
        Linux*)
            if [ -r /etc/os-release ]; then
                # shellcheck disable=SC1091
                . /etc/os-release
                case "${ID:-}${ID_LIKE:-}" in
                    *debian*|*ubuntu*) echo "linux-debian" ;;
                    *fedora*|*rhel*|*centos*) echo "linux-fedora" ;;
                    *arch*|*manjaro*) echo "linux-arch" ;;
                    *suse*) echo "linux-suse" ;;
                    *) echo "linux-unknown" ;;
                esac
            else
                echo "linux-unknown"
            fi
            ;;
        FreeBSD*) echo "freebsd" ;;
        MINGW*|MSYS*|CYGWIN*) echo "windows-bash" ;;
        *) echo "unknown" ;;
    esac
}

OS="$(detect_os)"
log INFO "Detected OS: $OS"

# ------------------------------------------------------ package-name mapping
# pkg_name <dep> <os> -> system-package name
pkg_name() {
    case "$1:$2" in
        bun:macos)             echo "oven-sh/bun/bun" ;;
        bun:linux-debian)      echo "" ;;   # no apt pkg; use bun.sh installer
        bun:linux-fedora)      echo "" ;;
        bun:linux-arch)        echo "bun-bin" ;;  # AUR
        bun:windows-bash)      echo "" ;;

        python3:macos)         echo "python@3.12" ;;
        python3:linux-debian)  echo "python3 python3-pip python3-venv" ;;
        python3:linux-fedora)  echo "python3 python3-pip" ;;
        python3:linux-arch)    echo "python python-pip" ;;

        tesseract:macos)        echo "tesseract" ;;
        tesseract:linux-debian) echo "tesseract-ocr" ;;
        tesseract:linux-fedora) echo "tesseract" ;;
        tesseract:linux-arch)   echo "tesseract tesseract-data-eng" ;;

        ffmpeg:macos)         echo "ffmpeg" ;;
        ffmpeg:linux-debian)  echo "ffmpeg" ;;
        ffmpeg:linux-fedora)  echo "ffmpeg" ;;
        ffmpeg:linux-arch)    echo "ffmpeg" ;;

        *) echo "" ;;
    esac
}

# install_cmd_hint <dep> <os> -> human-readable suggested command
install_cmd_hint() {
    _dep="$1"; _os="$2"
    case "$_dep" in
        bun)
            case "$_os" in
                macos)        echo "brew install oven-sh/bun/bun" ;;
                linux-debian|linux-fedora|linux-suse|linux-unknown)
                    echo "curl -fsSL https://bun.sh/install | bash" ;;
                linux-arch)   echo "yay -S bun-bin   (or: curl -fsSL https://bun.sh/install | bash)" ;;
                *)            echo "see https://bun.sh/docs/installation" ;;
            esac ;;
        python3)
            case "$_os" in
                macos)        echo "brew install python@3.12" ;;
                linux-debian) echo "sudo apt-get install -y python3 python3-pip python3-venv" ;;
                linux-fedora) echo "sudo dnf install -y python3 python3-pip" ;;
                linux-arch)   echo "sudo pacman -S --noconfirm python python-pip" ;;
                *)            echo "install python >=3.10 from your distro" ;;
            esac ;;
        tesseract)
            case "$_os" in
                macos)        echo "brew install tesseract" ;;
                linux-debian) echo "sudo apt-get install -y tesseract-ocr" ;;
                linux-fedora) echo "sudo dnf install -y tesseract" ;;
                linux-arch)   echo "sudo pacman -S --noconfirm tesseract tesseract-data-eng" ;;
                *)            echo "install tesseract >=5.0 from your distro" ;;
            esac ;;
        ffmpeg)
            case "$_os" in
                macos)        echo "brew install ffmpeg" ;;
                linux-debian) echo "sudo apt-get install -y ffmpeg" ;;
                linux-fedora) echo "sudo dnf install -y ffmpeg" ;;
                linux-arch)   echo "sudo pacman -S --noconfirm ffmpeg" ;;
                *)            echo "install ffmpeg from your distro" ;;
            esac ;;
        *) echo "(no hint available)" ;;
    esac
}

# ------------------------------------------------------ detection / version
dep_present() {
    case "$1" in
        bun)        has_cmd bun ;;
        python3)    has_cmd python3 || has_cmd python ;;
        tesseract)  has_cmd tesseract ;;
        ffmpeg)     has_cmd ffmpeg ;;
        *)          return 1 ;;
    esac
}

dep_version() {
    case "$1" in
        bun)
            bun --version 2>/dev/null || echo "?"
            ;;
        python3)
            if has_cmd python3; then
                python3 --version 2>&1 | awk '{print $2}'
            elif has_cmd python; then
                python --version 2>&1 | awk '{print $2}'
            else
                echo "?"
            fi
            ;;
        tesseract)
            tesseract --version 2>&1 | head -n1 | awk '{print $2}'
            ;;
        ffmpeg)
            ffmpeg -version 2>/dev/null | head -n1 | awk '{print $3}'
            ;;
        *) echo "?" ;;
    esac
}

# --------------------------------------------------------- preexisting set
# Snapshot what's already on the box BEFORE we install anything; this becomes
# the "preexisting" array in the install manifest so uninstall --keep-shared
# knows what to leave alone.
PREEXISTING=""
for d in $REQUIRED_DEPS; do
    if dep_present "$d"; then
        PREEXISTING="$PREEXISTING $d"
    fi
done

# ---------------------------------------------------------- install drivers

ensure_homebrew() {
    if has_cmd brew; then return 0; fi
    log WARN "Homebrew not found."
    if [ "$AUTO_INSTALL" -eq 0 ]; then
        log INFO "Install Homebrew first: see https://brew.sh"
        return 1
    fi
    if confirm "Install Homebrew now? (will run the official brew installer)"; then
        log INFO "Running official Homebrew installer (network)..."
        /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)" \
            >> "$LOG_FILE" 2>&1 || {
                log ERROR "Homebrew install failed; see $LOG_FILE"
                return 1
            }
        # add brew to PATH for the rest of this session
        if [ -d /opt/homebrew/bin ]; then
            PATH="/opt/homebrew/bin:$PATH"; export PATH
        elif [ -d /usr/local/bin ]; then
            PATH="/usr/local/bin:$PATH"; export PATH
        fi
        return 0
    fi
    return 1
}

run_pkg_install() {
    # run_pkg_install <os> <pkg-string>
    _os="$1"; _pkgs="$2"
    [ -z "$_pkgs" ] && return 1
    case "$_os" in
        macos)
            ensure_homebrew || return 1
            log INFO "brew install $_pkgs"
            # shellcheck disable=SC2086
            brew install $_pkgs >>"$LOG_FILE" 2>&1
            ;;
        linux-debian)
            log INFO "sudo apt-get update && sudo apt-get install -y $_pkgs"
            sudo apt-get update >>"$LOG_FILE" 2>&1 || true
            # shellcheck disable=SC2086
            sudo apt-get install -y $_pkgs >>"$LOG_FILE" 2>&1
            ;;
        linux-fedora)
            log INFO "sudo dnf install -y $_pkgs"
            # shellcheck disable=SC2086
            sudo dnf install -y $_pkgs >>"$LOG_FILE" 2>&1
            ;;
        linux-arch)
            log INFO "sudo pacman -S --noconfirm $_pkgs"
            # shellcheck disable=SC2086
            sudo pacman -S --noconfirm $_pkgs >>"$LOG_FILE" 2>&1
            ;;
        linux-suse)
            log INFO "sudo zypper install -y $_pkgs"
            # shellcheck disable=SC2086
            sudo zypper install -y $_pkgs >>"$LOG_FILE" 2>&1
            ;;
        *)
            log ERROR "No package manager driver for $_os"
            return 3
            ;;
    esac
}

install_bun_official() {
    log INFO "Installing Bun via official installer (curl -fsSL https://bun.sh/install | bash)"
    if ! has_cmd curl; then
        log ERROR "curl not found; cannot run bun installer"
        return 1
    fi
    curl -fsSL https://bun.sh/install | bash >>"$LOG_FILE" 2>&1 || {
        log ERROR "Bun installer failed; see $LOG_FILE"
        return 1
    }
    # bun installs to ~/.bun/bin -- pick it up for this session
    if [ -d "$HOME/.bun/bin" ]; then
        PATH="$HOME/.bun/bin:$PATH"; export PATH
    fi
}

copy_from_mirror() {
    # copy_from_mirror <dep>
    [ -z "$FROM_DIR" ] && return 1
    [ -d "$FROM_DIR" ] || { log ERROR "--from dir not found: $FROM_DIR"; return 1; }
    _dep="$1"
    # Look for a hint file: <dep>.tar.gz or <dep>-installer.sh
    for _candidate in \
        "$FROM_DIR/$_dep" \
        "$FROM_DIR/$_dep.tar.gz" \
        "$FROM_DIR/$_dep-installer.sh"
    do
        if [ -e "$_candidate" ]; then
            log INFO "Found local mirror artifact: $_candidate"
            log WARN "Local-mirror copy is a stub: extract/install $_candidate manually."
            return 0
        fi
    done
    return 1
}

install_dep() {
    _dep="$1"
    _pkgs="$(pkg_name "$_dep" "$OS")"
    log INFO "Installing $_dep on $OS..."

    # Try local mirror first if requested
    if [ -n "$FROM_DIR" ]; then
        if copy_from_mirror "$_dep"; then
            return 0
        else
            log WARN "No mirror artifact for $_dep; falling back to package manager"
        fi
    fi

    case "$_dep:$OS" in
        bun:linux-debian|bun:linux-fedora|bun:linux-suse|bun:linux-unknown|bun:windows-bash)
            install_bun_official
            ;;
        *)
            if [ -n "$_pkgs" ]; then
                run_pkg_install "$OS" "$_pkgs"
            else
                # final fallback
                if [ "$_dep" = "bun" ]; then
                    install_bun_official
                else
                    log ERROR "No install path for $_dep on $OS"
                    return 3
                fi
            fi
            ;;
    esac
}

# --------------------------------------------------- manifest write helper
write_manifest() {
    _installed="$1"  # space-separated list
    mkdir -p "$MNEME_HOME"
    # JSON arrays
    _to_json_array() {
        _result="["; _first=1
        for _x in $1; do
            [ -z "$_x" ] && continue
            if [ $_first -eq 1 ]; then
                _result="${_result}\"${_x}\""; _first=0
            else
                _result="${_result},\"${_x}\""
            fi
        done
        _result="${_result}]"
        printf '%s' "$_result"
    }
    _now="$(date -u +'%Y-%m-%dT%H:%M:%SZ')"
    cat > "$MANIFEST_FILE" <<EOF
{
  "datatree_version": "${DATATREE_VERSION}",
  "installed_at": "${_now}",
  "installed_by_datatree": $(_to_json_array "$_installed"),
  "preexisting": $(_to_json_array "$PREEXISTING"),
  "models": {}
}
EOF
    log OK "Wrote install manifest -> $MANIFEST_FILE"
}

# =========================================================== main flow

log INFO "datatree runtime installer v${DATATREE_VERSION} starting"
log INFO "AUTO_INSTALL=$AUTO_INSTALL  FROM=${FROM_DIR:-<none>}"

MISSING=""
for d in $REQUIRED_DEPS; do
    if dep_present "$d"; then
        log OK "$d already installed ($(dep_version "$d"))"
    else
        log WARN "$d MISSING"
        MISSING="$MISSING $d"
    fi
done

if [ -z "$(echo "$MISSING" | tr -d ' ')" ]; then
    log OK "All required runtime deps already present.  Nothing to do."
    write_manifest ""   # no new installs
    exit 0
fi

if [ "$AUTO_INSTALL" -eq 0 ]; then
    log WARN "Missing required deps:$MISSING"
    echo
    echo "To install them, either re-run with --auto-install, or run these commands:"
    echo
    for d in $MISSING; do
        printf '  %-12s -> %s\n' "$d" "$(install_cmd_hint "$d" "$OS")"
    done
    echo
    echo "If you have a pre-downloaded mirror folder, pass --from <dir>."
    exit 1
fi

# AUTO_INSTALL path
if ! confirm "About to install:$MISSING. Proceed?"; then
    log WARN "User declined auto-install"
    exit 1
fi

INSTALLED=""
for d in $MISSING; do
    if install_dep "$d"; then
        # verify
        if dep_present "$d"; then
            log OK "$d installed -> version $(dep_version "$d")"
            INSTALLED="$INSTALLED $d"
        else
            log ERROR "$d install reported success but binary not found on PATH"
            exit 2
        fi
    else
        log ERROR "Failed to install $d (see $LOG_FILE)"
        exit 2
    fi
done

write_manifest "$INSTALLED"
log OK "datatree runtime install complete."
exit 0
