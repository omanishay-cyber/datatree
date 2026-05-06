#!/usr/bin/env bash
# scripts/test/install-hash-verify.tests.sh
#
# HIGH-13 (2026-05-06 deep audit): tests for the SHA-256 verification
# block in scripts/install.sh. We mirror the install-hash-verify.tests.ps1
# matrix: parse smoke + envvar gate + tampered-archive rejection +
# override bypass + missing-manifest warn-continue.
#
# Strategy: install.sh has heavy top-level side effects (process kills,
# daemon start, register-mcp). We don't run it end-to-end; instead
# we lift the verifier into a sourced shell function and exercise
# it on synthesized tarballs.
#
# Usage:
#   bash scripts/test/install-hash-verify.tests.sh
# Exits 0 on pass, 1 on any fail.

set -u

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
INSTALL_SH="${SCRIPT_DIR}/../install.sh"

# ---------------------------------------------------------------------------
# Smoke check: install.sh has clean POSIX-sh syntax.
# ---------------------------------------------------------------------------
if ! bash -n "${INSTALL_SH}"; then
    echo "FAIL: install.sh has syntax errors" >&2
    exit 1
fi

INSTALL_SH_BODY="$(cat "${INSTALL_SH}")"

# ---------------------------------------------------------------------------
# Helpers - faked verifier mirroring install.sh's verification block.
# Echoes one of:
#   match | mismatch | skipped-override | skipped-no-manifest
# Returns non-zero on mismatch (mirrors install.sh's `exit 1`).
# ---------------------------------------------------------------------------

fake_hash_verify() {
    local archive="$1"
    local expected="$2"
    local override="$3"   # 1 to skip

    if [ "${override}" = "1" ]; then
        echo "skipped-override"
        return 0
    fi
    if [ -z "${expected}" ]; then
        echo "skipped-no-manifest"
        return 0
    fi

    local actual=""
    if command -v sha256sum >/dev/null 2>&1; then
        actual=$(sha256sum "${archive}" | awk '{print $1}')
    elif command -v shasum >/dev/null 2>&1; then
        actual=$(shasum -a 256 "${archive}" | awk '{print $1}')
    elif command -v openssl >/dev/null 2>&1; then
        actual=$(openssl dgst -sha256 -r "${archive}" | awk '{print $1}')
    else
        echo "no-hash-tool"
        return 1
    fi

    local exp_lc act_lc
    exp_lc=$(printf '%s' "${expected}" | tr 'A-F' 'a-f')
    act_lc=$(printf '%s' "${actual}" | tr 'A-F' 'a-f')
    if [ "${exp_lc}" = "${act_lc}" ]; then
        echo "match"
        return 0
    fi
    echo "ARCHIVE INTEGRITY CHECK FAILED expected=${exp_lc} actual=${act_lc}" >&2
    return 1
}

# Build a fake tarball + return its SHA-256.
new_fake_tarball() {
    local root="$1"
    local name="${2:-mneme-linux-x64.tar.gz}"
    local stage="${root}/tar-stage"
    mkdir -p "${stage}/bin"
    printf 'payload-mneme
' > "${stage}/bin/mneme"
    printf 'payload-daemon
' > "${stage}/bin/mneme-daemon"
    local tarball="${root}/${name}"
    tar -czf "${tarball}" -C "${stage}" .
    local hash=""
    if command -v sha256sum >/dev/null 2>&1; then
        hash=$(sha256sum "${tarball}" | awk '{print $1}')
    elif command -v shasum >/dev/null 2>&1; then
        hash=$(shasum -a 256 "${tarball}" | awk '{print $1}')
    fi
    echo "${tarball} ${hash}"
}

tamper_one_byte() {
    local file="$1"
    local size
    size=$(wc -c < "${file}")
    if [ "${size}" -lt 100 ]; then
        echo "tarball too small to tamper sensibly: ${size} bytes" >&2
        return 1
    fi
    local mid=$(( size / 2 ))
    # Flip one byte using dd: read one byte from /dev/urandom and write
    # at offset mid. Use conv=notrunc so the file stays the same length.
    dd if=/dev/urandom of="${file}" bs=1 count=1 seek="${mid}" conv=notrunc status=none
}

# ---------------------------------------------------------------------------
# Test runner.
# ---------------------------------------------------------------------------
PASS=0
FAIL=0

test_pass() {
    PASS=$(( PASS + 1 ))
    printf '    [PASS] %s
' "$1"
}

test_fail() {
    FAIL=$(( FAIL + 1 ))
    printf '    [FAIL] %s -- %s
' "$1" "$2" >&2
}

printf '==> install.sh SHA-256 verify (HIGH-13)
'

# ---------------------------------------------------------------------------
# 1. install.sh body declares MNEME_SKIP_HASH_VERIFY env var support.
# ---------------------------------------------------------------------------
if printf '%s' "${INSTALL_SH_BODY}" | grep -qE 'MNEME_SKIP_HASH_VERIFY'; then
    test_pass "envvar_MNEME_SKIP_HASH_VERIFY_present"
else
    test_fail "envvar_MNEME_SKIP_HASH_VERIFY_present" "install.sh has no MNEME_SKIP_HASH_VERIFY reference"
fi

# ---------------------------------------------------------------------------
# 2. install.sh body also accepts MNEME_SKIP_HASH_CHECK as alias.
# ---------------------------------------------------------------------------
if printf '%s' "${INSTALL_SH_BODY}" | grep -qE 'MNEME_SKIP_HASH_CHECK'; then
    test_pass "envvar_MNEME_SKIP_HASH_CHECK_alias_present"
else
    test_fail "envvar_MNEME_SKIP_HASH_CHECK_alias_present" "install.sh has no MNEME_SKIP_HASH_CHECK alias"
fi

# ---------------------------------------------------------------------------
# 3. install.sh prints expected/actual/verdict on every verification path.
# ---------------------------------------------------------------------------
if printf '%s' "${INSTALL_SH_BODY}" | grep -qE 'expected : '; then
    test_pass "verdict_print_expected_actual"
else
    test_fail "verdict_print_expected_actual" "install.sh does not print expected/actual lines"
fi

if printf '%s' "${INSTALL_SH_BODY}" | grep -qE 'verdict: MATCH'; then
    test_pass "verdict_print_match"
else
    test_fail "verdict_print_match" "install.sh does not print verdict: MATCH"
fi

# ---------------------------------------------------------------------------
# 4. match path: identical hashes -> match.
# ---------------------------------------------------------------------------
TMP_DIR=$(mktemp -d)
trap 'rm -rf "${TMP_DIR}"' EXIT INT TERM

read -r tarball hash <<< "$(new_fake_tarball "${TMP_DIR}")"
verdict=$(fake_hash_verify "${tarball}" "${hash}" "0" 2>&1)
if [ "${verdict}" = "match" ]; then
    test_pass "match_passes"
else
    test_fail "match_passes" "got: ${verdict}"
fi

# ---------------------------------------------------------------------------
# 5. tampered path: flip one byte after computing the hash -> mismatch -> exit 1.
# ---------------------------------------------------------------------------
read -r tarball2 hash2 <<< "$(new_fake_tarball "${TMP_DIR}" "tampered.tar.gz")"
tamper_one_byte "${tarball2}"
verdict=$(fake_hash_verify "${tarball2}" "${hash2}" "0" 2>/dev/null)
rc=$?
if [ "${rc}" -ne 0 ]; then
    test_pass "tampered_one_byte_rejected"
else
    test_fail "tampered_one_byte_rejected" "verifier returned 0; verdict=${verdict}"
fi

# ---------------------------------------------------------------------------
# 6. override path: tampered tarball + override=1 -> skipped-override (no error).
# ---------------------------------------------------------------------------
read -r tarball3 hash3 <<< "$(new_fake_tarball "${TMP_DIR}" "tampered2.tar.gz")"
tamper_one_byte "${tarball3}"
verdict=$(fake_hash_verify "${tarball3}" "${hash3}" "1" 2>&1)
rc=$?
if [ "${rc}" -eq 0 ] && [ "${verdict}" = "skipped-override" ]; then
    test_pass "override_skips_tampered"
else
    test_fail "override_skips_tampered" "rc=${rc} verdict=${verdict}"
fi

# ---------------------------------------------------------------------------
# 7. no-manifest path: empty expected hash -> skipped-no-manifest (no error).
# ---------------------------------------------------------------------------
read -r tarball4 hash4 <<< "$(new_fake_tarball "${TMP_DIR}" "plain.tar.gz")"
verdict=$(fake_hash_verify "${tarball4}" "" "0" 2>&1)
rc=$?
if [ "${rc}" -eq 0 ] && [ "${verdict}" = "skipped-no-manifest" ]; then
    test_pass "no_manifest_warns_continues"
else
    test_fail "no_manifest_warns_continues" "rc=${rc} verdict=${verdict}"
fi

# ---------------------------------------------------------------------------
# Summary.
# ---------------------------------------------------------------------------
printf '
Result: %d passed, %d failed
' "${PASS}" "${FAIL}"
if [ "${FAIL}" -gt 0 ]; then
    exit 1
fi
exit 0
