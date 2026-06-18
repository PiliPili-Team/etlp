#!/usr/bin/env bash
#
# build-linux-app.sh
# Build the etlp-gui Tauri v2 app for Linux (x86_64, AppImage).
#
# Usage: bash scripts/build-linux-app.sh [--arch amd64] [--dry-run] [--help]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=build-app.sh
source "${SCRIPT_DIR}/build-app.sh"

TARGET="x86_64-unknown-linux-gnu"
ARCH="amd64"

show_help() {
    cat <<'EOF'
build-linux-app.sh — Build etlp-gui Tauri app for Linux (x86_64, AppImage)

USAGE
    bash scripts/build-linux-app.sh [OPTIONS]

OPTIONS
    --arch amd64     Target architecture (only amd64 supported)
    --dry-run        Print actions without making changes
    --help, -h       Show this message

REQUIREMENTS
    Ubuntu/Debian:
      sudo apt-get install libayatana-appindicator3-dev \
        libwebkit2gtk-4.1-dev libgtk-3-dev patchelf

OUTPUT
    etlp-gui/src-tauri/target/x86_64-unknown-linux-gnu/release/bundle/appimage/*.AppImage
EOF
}

_ensure_linux_deps() {
    if ! command -v apt-get > /dev/null 2>&1; then
        _log_skip "system dep install (not Debian/Ubuntu — install manually)"
        return 0
    fi
    local pkgs=(
        libayatana-appindicator3-dev
        libwebkit2gtk-4.1-dev
        libgtk-3-dev
        patchelf
    )
    local missing=()
    for pkg in "${pkgs[@]}"; do
        dpkg -s "${pkg}" > /dev/null 2>&1 || missing+=("${pkg}")
    done
    if [[ ${#missing[@]} -eq 0 ]]; then
        _log_skip "system deps (all already installed)"
        return 0
    fi
    _log "Installing" "system deps: ${missing[*]}"
    _run sudo apt-get update -qq
    _run sudo apt-get install -y "${missing[@]}"
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
        _log_err "only amd64 is supported for Linux app builds (got: ${ARCH})"
        exit 1
    fi

    "${DRY_RUN}" && printf "${C_YELLOW}%12s${C_RESET} no changes will be made\n\n" "dry-run"

    check_rust_toolchain
    check_node
    _ensure_linux_deps
    add_rust_target "${TARGET}"
    install_frontend_deps
    build_tauri_app "${TARGET}"

    _log "Done" \
        "bundle → etlp-gui/src-tauri/target/${TARGET}/release/bundle/appimage/"
    printf "\n${C_GREEN}%12s${C_RESET} build complete\n" "Finished"
}

main "$@"
