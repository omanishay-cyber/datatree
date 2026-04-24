#!/usr/bin/env sh
# mneme :: uninstall-runtime.sh
# Removes the runtime dependencies mneme installed.  Reads the install
# manifest at ~/.mneme/install-manifest.json to know which deps were
# installed by mneme (vs. preexisting on the user's machine).
#
# Flags:
#   --keep-shared   : do NOT remove anything listed under "preexisting"
#                     (default behaviour -- shared tools are preserved)
#   --remove-shared : explicitly opt-in to also remove preexisting deps
#                     (DANGEROUS; almost never what you want)
#   --yes / -y      : skip confirmation prompts
#   --dry-run       : print what would be removed without doing it
#
# Exit codes:
#   0  success
#   1  manifest missing or invalid
#   2  removal failed for one or more deps

set -eu

MNEME_HOME="${MNEME_HOME:-${HOME}/.mneme}"
LOG_DIR="${MNEME_HOME}/logs"
LOG_FILE="${LOG_DIR}/install.log"
MANIFEST_FILE="${MNEME_HOME}/install-manifest.json"

KEEP_SHARED=1   # default true
ASSUME_YES=0
DRY_RUN=0

while [ $# -gt 0 ]; do
    case "$1" in
        --keep-shared)   KEEP_SHARED=1 ;;
        --remove-shared) KEEP_SHARED=0 ;;
        --yes|-y)        ASSUME_YES=1 ;;
        --dry-run)       DRY_RUN=1 ;;
        -h|--help)
            sed -n '2,18p' "$0" | sed 's/^# \{0,1\}//'
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
    printf '[%s] [UNINSTALL] %s\n' "$_ts" "$*" >> "$LOG_FILE"
    printf '\033[0;36m[uninst]\033[0m %s\n' "$*"
}
err() {
    log "ERROR: $*"
    printf '\033[0;31m[uninst][err]\033[0m %s\n' "$*" >&2
}
confirm() {
    [ $ASSUME_YES -eq 1 ] && return 0
    printf '%s [y/N]: ' "$1"
    read -r _a </dev/tty 2>/dev/null || _a="n"
    case "$_a" in y|Y|yes|YES) return 0 ;; *) return 1 ;; esac
}

if [ ! -f "$MANIFEST_FILE" ]; then
    err "Install manifest not found: $MANIFEST_FILE"
    err "Either mneme was never installed, or you wiped \$MNEME_HOME."
    exit 1
fi

# Tiny JSON parser: pulls a string-array field out of a flat JSON object.
# Works on dash/ash; no jq dependency.
parse_array() {
    _field="$1"
    awk -v field="$_field" '
        BEGIN { in_field=0; out="" }
        {
            line=$0
            # find the field
            if (match(line, "\"" field "\"[[:space:]]*:[[:space:]]*\\[")) {
                # could be single-line or span lines until a closing ]
                rest = substr(line, RSTART)
                # accumulate until we see the closing ]
                buf = rest
                while (index(buf, "]") == 0) {
                    if ((getline nl) <= 0) break
                    buf = buf "\n" nl
                }
                # extract everything between [ and ]
                lb = index(buf, "[")
                rb = index(buf, "]")
                inner = substr(buf, lb+1, rb-lb-1)
                # split on commas, strip quotes/whitespace
                n = split(inner, parts, ",")
                for (i=1; i<=n; i++) {
                    gsub(/[ \t\n\r"]/, "", parts[i])
                    if (parts[i] != "") print parts[i]
                }
                exit 0
            }
        }
    ' "$MANIFEST_FILE"
}

INSTALLED_BY_DT="$(parse_array installed_by_mneme | tr '\n' ' ')"
PREEXISTING="$(parse_array preexisting | tr '\n' ' ')"

log "manifest: installed_by_mneme=[$INSTALLED_BY_DT]  preexisting=[$PREEXISTING]"

if [ -z "$(echo "$INSTALLED_BY_DT" | tr -d ' ')" ] && [ $KEEP_SHARED -eq 1 ]; then
    log "Nothing to uninstall (no deps were installed by mneme)."
    exit 0
fi

# Determine target list
if [ $KEEP_SHARED -eq 1 ]; then
    TARGETS="$INSTALLED_BY_DT"
else
    TARGETS="$INSTALLED_BY_DT $PREEXISTING"
fi
TARGETS="$(echo "$TARGETS" | tr ' ' '\n' | awk 'NF && !seen[$0]++' | tr '\n' ' ')"

if [ -z "$(echo "$TARGETS" | tr -d ' ')" ]; then
    log "Nothing to uninstall."
    exit 0
fi

log "Will remove:$TARGETS"
[ $DRY_RUN -eq 1 ] && { log "Dry run -- exiting without changes."; exit 0; }

if ! confirm "Proceed with removal?"; then
    log "User declined."
    exit 0
fi

# OS detection (same logic as install-runtime.sh)
detect_os() {
    case "$(uname -s 2>/dev/null || echo unknown)" in
        Darwin*) echo macos ;;
        Linux*)
            if [ -r /etc/os-release ]; then
                # shellcheck disable=SC1091
                . /etc/os-release
                case "${ID:-}${ID_LIKE:-}" in
                    *debian*|*ubuntu*) echo linux-debian ;;
                    *fedora*|*rhel*)   echo linux-fedora ;;
                    *arch*)            echo linux-arch ;;
                    *suse*)            echo linux-suse ;;
                    *)                 echo linux-unknown ;;
                esac
            else echo linux-unknown; fi ;;
        *) echo unknown ;;
    esac
}
OS="$(detect_os)"
log "OS=$OS"

remove_pkg() {
    _dep="$1"
    case "$OS:$_dep" in
        macos:bun)        brew uninstall oven-sh/bun/bun  >>"$LOG_FILE" 2>&1 || return 1 ;;
        macos:python3)    brew uninstall python@3.12      >>"$LOG_FILE" 2>&1 || return 1 ;;
        macos:tesseract)  brew uninstall tesseract        >>"$LOG_FILE" 2>&1 || return 1 ;;
        macos:ffmpeg)     brew uninstall ffmpeg           >>"$LOG_FILE" 2>&1 || return 1 ;;

        linux-debian:bun)
            # installed via bun.sh installer; remove from $HOME/.bun
            rm -rf "$HOME/.bun"
            ;;
        linux-debian:python3)   sudo apt-get remove -y python3 python3-pip python3-venv >>"$LOG_FILE" 2>&1 || return 1 ;;
        linux-debian:tesseract) sudo apt-get remove -y tesseract-ocr                    >>"$LOG_FILE" 2>&1 || return 1 ;;
        linux-debian:ffmpeg)    sudo apt-get remove -y ffmpeg                           >>"$LOG_FILE" 2>&1 || return 1 ;;

        linux-fedora:bun)       rm -rf "$HOME/.bun" ;;
        linux-fedora:python3)   sudo dnf remove -y python3 python3-pip >>"$LOG_FILE" 2>&1 || return 1 ;;
        linux-fedora:tesseract) sudo dnf remove -y tesseract           >>"$LOG_FILE" 2>&1 || return 1 ;;
        linux-fedora:ffmpeg)    sudo dnf remove -y ffmpeg              >>"$LOG_FILE" 2>&1 || return 1 ;;

        linux-arch:bun)         sudo pacman -Rns --noconfirm bun-bin    >>"$LOG_FILE" 2>&1 || rm -rf "$HOME/.bun" ;;
        linux-arch:python3)     sudo pacman -Rns --noconfirm python python-pip >>"$LOG_FILE" 2>&1 || return 1 ;;
        linux-arch:tesseract)   sudo pacman -Rns --noconfirm tesseract tesseract-data-eng >>"$LOG_FILE" 2>&1 || return 1 ;;
        linux-arch:ffmpeg)      sudo pacman -Rns --noconfirm ffmpeg     >>"$LOG_FILE" 2>&1 || return 1 ;;

        *)
            err "No removal driver for $_dep on $OS"
            return 3
            ;;
    esac
    return 0
}

FAILED=""
REMOVED=""
for d in $TARGETS; do
    log "Removing $d ..."
    if remove_pkg "$d"; then
        log "  removed $d"
        REMOVED="$REMOVED $d"
    else
        err "  failed to remove $d (see $LOG_FILE)"
        FAILED="$FAILED $d"
    fi
done

# Update manifest -- strip the removed entries
if [ -n "$(echo "$REMOVED" | tr -d ' ')" ]; then
    log "Updating manifest"
    # very small awk filter: rewrites installed_by_mneme minus REMOVED entries
    _now="$(date -u +'%Y-%m-%dT%H:%M:%SZ')"
    _new_installed=""
    for d in $INSTALLED_BY_DT; do
        _keep=1
        for r in $REMOVED; do
            if [ "$d" = "$r" ]; then _keep=0; fi
        done
        [ $_keep -eq 1 ] && _new_installed="$_new_installed $d"
    done
    _new_preexisting="$PREEXISTING"
    if [ $KEEP_SHARED -eq 0 ]; then
        for d in $PREEXISTING; do
            for r in $REMOVED; do
                if [ "$d" = "$r" ]; then
                    _new_preexisting="$(echo "$_new_preexisting" | sed "s/\b$d\b//g")"
                fi
            done
        done
    fi
    _to_json() {
        _r="["; _f=1
        for _x in $1; do
            [ -z "$_x" ] && continue
            if [ $_f -eq 1 ]; then _r="${_r}\"${_x}\""; _f=0
            else _r="${_r},\"${_x}\""; fi
        done
        printf '%s]' "$_r"
    }
    cat > "$MANIFEST_FILE" <<EOF
{
  "mneme_version": "0.1.0",
  "installed_at": "${_now}",
  "installed_by_mneme": $(_to_json "$_new_installed"),
  "preexisting": $(_to_json "$_new_preexisting"),
  "models": {}
}
EOF
fi

if [ -n "$(echo "$FAILED" | tr -d ' ')" ]; then
    err "Some removals failed:$FAILED"
    exit 2
fi

log "Uninstall complete."
exit 0
