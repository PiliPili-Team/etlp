#!/usr/bin/env bash
#
# build-windows-binary.sh
# Build the etlp binary for Windows (x86_64 MSVC).
#
# Run in Git Bash on Windows. Requires MSVC build tools and Rust.
# Usage: bash scripts/build-windows-binary.sh [--arch amd64] [--dry-run] [--help]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=build-binary.sh
source "${SCRIPT_DIR}/build-binary.sh"

TARGET="x86_64-pc-windows-msvc"
ASSET="etlp-windows-amd64.zip"
ARCH="amd64"

show_help() {
    cat <<'EOF'
build-windows-binary.sh — Build etlp binary for Windows (x86_64 MSVC)

USAGE
    bash scripts/build-windows-binary.sh [OPTIONS]

OPTIONS
    --arch amd64     Target architecture (only amd64 supported)
    --dry-run        Print actions without making changes
    --help, -h       Show this message

REQUIREMENTS
    Windows with MSVC build tools and Rust (x86_64-pc-windows-msvc target).

OUTPUT
    dist/binaries/etlp-windows-amd64.zip
EOF
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

    if [[ "${ARCH}" != "amd64" ]]; then
        _log_err "only amd64 is supported for Windows builds (got: ${ARCH})"
        exit 1
    fi

    "${DRY_RUN}" && printf "${C_YELLOW}%12s${C_RESET} no changes will be made\n\n" "dry-run"

    check_rust_toolchain
    add_rust_target "${TARGET}"
    build_binary    "${TARGET}"
    package_zip     "${TARGET}" "${ASSET}"

    printf "\n${C_GREEN}%12s${C_RESET} build complete\n" "Finished"
}

main "$@"
