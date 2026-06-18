#!/usr/bin/env bash
#
# build-linux-binary.sh
# Build the etlp binary for Linux (musl static).
#
# Usage: bash scripts/build-linux-binary.sh [--arch amd64|arm64|all] [--dry-run] [--help]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=build-binary.sh
source "${SCRIPT_DIR}/build-binary.sh"

ARCH="native"

show_help() {
    cat <<'EOF'
build-linux-binary.sh — Build etlp binary for Linux (musl static)

USAGE
    bash scripts/build-linux-binary.sh [OPTIONS]

OPTIONS
    --arch amd64|arm64|all   Target architecture (default: native host arch)
    --dry-run                Print actions without making changes
    --help, -h               Show this message

REQUIREMENTS
    Ubuntu/Debian: sudo apt-get install musl-tools

OUTPUT
    dist/binaries/etlp-linux-{amd64,arm64}.tar.gz
EOF
}

_native_arch() {
    case "$(uname -m)" in
        aarch64) printf "arm64" ;;
        *)       printf "amd64" ;;
    esac
}

_ensure_musl_tools() {
    if command -v musl-gcc > /dev/null 2>&1; then
        _log_skip "musl-tools (already installed)"
        return 0
    fi
    if ! command -v apt-get > /dev/null 2>&1; then
        _log_err "musl-gcc not found — install musl-tools for your distro"
        exit 1
    fi
    _log "Installing" "musl-tools"
    _run sudo apt-get update -qq
    _run sudo apt-get install -y musl-tools
}

_build_one() {
    local arch="$1" target asset
    case "${arch}" in
        amd64) target="x86_64-unknown-linux-musl";  asset="etlp-linux-amd64.tar.gz" ;;
        arm64) target="aarch64-unknown-linux-musl";  asset="etlp-linux-arm64.tar.gz" ;;
        *)     _log_err "unknown arch: ${arch}"; exit 1 ;;
    esac
    add_rust_target "${target}"
    build_binary    "${target}"
    package_tar     "${target}" "${asset}"
}

main() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --arch)    shift; ARCH="$1" ;;
            --arch=*)  ARCH="${1#*=}" ;;
            --dry-run) DRY_RUN=true ;;
            --help|-h) show_help; exit 0 ;;
            *)         _log_err "unknown option: $1"; show_help; exit 1 ;;
        esac
        shift
    done

    [[ "${ARCH}" == "native" ]] && ARCH="$(_native_arch)"
    "${DRY_RUN}" && printf "${C_YELLOW}%12s${C_RESET} no changes will be made\n\n" "dry-run"

    check_rust_toolchain
    _ensure_musl_tools

    if [[ "${ARCH}" == "all" ]]; then
        _build_one "amd64"
        _build_one "arm64"
    else
        _build_one "${ARCH}"
    fi

    printf "\n${C_GREEN}%12s${C_RESET} build complete\n" "Finished"
}

main "$@"
