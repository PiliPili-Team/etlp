#!/usr/bin/env bash
#
# build-app.sh
# Shared functions for etlp-gui (Tauri v2) app build scripts.
#
# Source this file; do not execute it directly.
# shellcheck disable=SC2034

[[ "${BASH_SOURCE[0]}" == "${0}" ]] && {
    printf "build-app.sh is a shared library — source it, do not run directly.\n" >&2
    exit 1
}

APP_NAME="etlp-gui"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
GUI_DIR="${REPO_ROOT}/etlp-gui"
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
        printf "${C_RED}%12s${C_RESET} exited with code %s\n" "error" "${code}" >&2
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

check_node() {
    if ! command -v node > /dev/null 2>&1; then
        _log_err "node not found — install via https://nodejs.org"
        exit 1
    fi
    _log "Found" "node $(node --version)"
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

install_frontend_deps() {
    _log "Installing" "frontend dependencies"
    if "${DRY_RUN}"; then
        _run_in "${GUI_DIR}" npm install
    else
        _run_in "${GUI_DIR}" npm install --silent 2>&1 \
            | grep -v "^$" \
            | grep -Ev "^(npm warn|added [0-9]+ package)" \
            || true
    fi
}

build_tauri_app() {
    local target="$1"; shift
    _log "Building" "${APP_NAME} → ${target}"
    # Any extra arguments are forwarded to `tauri build` (e.g. a per-arch
    # `--config` icon override).
    _run_in "${GUI_DIR}" npx tauri build --target "${target}" "$@"
}
