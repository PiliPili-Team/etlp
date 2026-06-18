#!/usr/bin/env bash
#
# build-binary.sh
# Shared functions for etlp binary build scripts.
#
# Source this file; do not execute it directly.
# shellcheck disable=SC2034

[[ "${BASH_SOURCE[0]}" == "${0}" ]] && {
    printf "build-binary.sh is a shared library — source it, do not run directly.\n" >&2
    exit 1
}

BIN_NAME="etlp"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
OUT_DIR="${REPO_ROOT}/dist/binaries"
DRY_RUN=false

C_GREEN='\033[1;32m'
C_YELLOW='\033[1;33m'
C_RED='\033[1;31m'
C_RESET='\033[0m'

_log()     { printf "${C_GREEN}%12s${C_RESET} %s\n" "$1" "$2"; }
_log_skip(){ printf "${C_YELLOW}%12s${C_RESET} %s\n" "Skipping" "$1"; }
_log_err() { printf "${C_RED}%12s${C_RESET} %s\n" "error" "$1" >&2; }

_run() {
    if "${DRY_RUN}"; then
        printf "${C_YELLOW}%12s${C_RESET} %s\n" "dry-run" "$*"
    else
        "$@"
    fi
}

_run_in() {
    local dir="$1"; shift
    if "${DRY_RUN}"; then
        printf "${C_YELLOW}%12s${C_RESET} (cd %s) %s\n" "dry-run" "${dir}" "$*"
    else
        (cd "${dir}" && "$@")
    fi
}

_on_exit() {
    local code=$?
    if [ "${code}" -ne 0 ]; then
        printf "\n${C_RED}%12s${C_RESET} exited with code %s\n" "error" "${code}" >&2
    fi
}
trap _on_exit EXIT

check_rust_toolchain() {
    if ! command -v cargo > /dev/null 2>&1; then
        _log_err "cargo not found — install via https://rustup.rs"
        exit 1
    fi
    _log "Found" "$(cargo --version)"
}

add_rust_target() {
    local target="$1"
    if rustup target list --installed 2>/dev/null | grep -q "^${target}$"; then
        _log_skip "target ${target} (already installed)"
    else
        _log "Adding" "target ${target}"
        _run rustup target add "${target}"
    fi
}

build_binary() {
    local target="$1"
    _log "Building" "${BIN_NAME} → ${target}"
    # CARGO_EXTRA_FLAGS may be set by CI (e.g. --locked) but is empty locally.
    _run_in "${REPO_ROOT}" cargo build --release --target "${target}" \
        ${CARGO_EXTRA_FLAGS:-}
}

package_tar() {
    local target="$1" asset="$2"
    _run mkdir -p "${OUT_DIR}"
    _log "Packaging" "${asset}"
    _run tar -czf "${OUT_DIR}/${asset}" \
        -C "${REPO_ROOT}/target/${target}/release" "${BIN_NAME}"
    _log "Output" "${OUT_DIR}/${asset}"
}

package_zip() {
    local target="$1" asset="$2"
    _run mkdir -p "${OUT_DIR}"
    _log "Packaging" "${asset}"
    _run_in "${REPO_ROOT}/target/${target}/release" \
        zip -j "${OUT_DIR}/${asset}" "${BIN_NAME}.exe"
    _log "Output" "${OUT_DIR}/${asset}"
}
