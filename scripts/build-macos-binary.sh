#!/usr/bin/env bash
#
# build-macos-binary.sh
# Build the etlp binary for macOS.
#
# Usage: bash scripts/build-macos-binary.sh [--arch amd64|arm64|all] [--dry-run] [--help]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=build-binary.sh
source "${SCRIPT_DIR}/build-binary.sh"

ARCH="native"

show_help() {
    cat <<'EOF'
build-macos-binary.sh — Build etlp binary for macOS

USAGE
    bash scripts/build-macos-binary.sh [OPTIONS]

OPTIONS
    --arch amd64|arm64|all   Target architecture (default: native host arch)
    --dry-run                Print actions without making changes
    --help, -h               Show this message

OUTPUT
    dist/binaries/etlp-macos-{amd64,arm64}.tar.gz
EOF
}

_native_arch() {
    case "$(uname -m)" in
        arm64|aarch64) printf "arm64" ;;
        *)             printf "amd64" ;;
    esac
}

_build_one() {
    local arch="$1" target asset
    case "${arch}" in
        amd64) target="x86_64-apple-darwin";  asset="etlp-macos-amd64.tar.gz" ;;
        arm64) target="aarch64-apple-darwin";  asset="etlp-macos-arm64.tar.gz" ;;
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

    if [[ "${ARCH}" == "all" ]]; then
        _build_one "amd64"
        _build_one "arm64"
    else
        _build_one "${ARCH}"
    fi

    printf "\n${C_GREEN}%12s${C_RESET} build complete\n" "Finished"
}

main "$@"
