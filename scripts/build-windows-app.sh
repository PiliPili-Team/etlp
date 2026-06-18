#!/usr/bin/env bash
#
# build-windows-app.sh
# Build the etlp-gui Tauri v2 app for Windows (x86_64 MSVC, NSIS installer).
#
# Run in Git Bash on Windows. Requires MSVC build tools and Node.js.
# Usage: bash scripts/build-windows-app.sh [--arch amd64] [--dry-run] [--help]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=build-app.sh
source "${SCRIPT_DIR}/build-app.sh"

TARGET="x86_64-pc-windows-msvc"
ARCH="amd64"

show_help() {
    cat <<'EOF'
build-windows-app.sh — Build etlp-gui Tauri app for Windows (NSIS installer)

USAGE
    bash scripts/build-windows-app.sh [OPTIONS]

OPTIONS
    --arch amd64     Target architecture (only amd64 supported)
    --dry-run        Print actions without making changes
    --help, -h       Show this message

REQUIREMENTS
    Windows with MSVC build tools, Node.js, and Rust.

OUTPUT
    etlp-gui/src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/*.exe
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
        _log_err "only amd64 is supported for Windows app builds (got: ${ARCH})"
        exit 1
    fi
    if "${DRY_RUN}"; then
        printf "${C_YELLOW}%12s${C_RESET} no changes will be made\n\n" "dry-run"
    fi
    check_rust_toolchain
    check_node
    add_rust_target "${TARGET}"
    install_frontend_deps
    build_tauri_app "${TARGET}"
    _log "Done" \
        "bundle → etlp-gui/src-tauri/target/${TARGET}/release/bundle/nsis/"
    printf "\n${C_GREEN}%12s${C_RESET} build complete\n" "Finished"
}

main "$@"
