#!/usr/bin/env bash
#
# build-windows-portable.sh
# Build the Windows Portable zip distribution for etlp-gui.
#
# Produces: dist/Genshin-portable-x64-<version>.zip
# Structure inside the zip:
#   Genshin.exe
#   updater.exe
#   portable.bin
#   config/   (empty marker directory)
#   data/     (empty marker directory)
#   update/   (empty marker directory)
#
# Run in Git Bash on Windows (or via the CI workflow).
# Usage: bash scripts/build-windows-portable.sh [--dry-run] [--help]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=build-app.sh
source "${SCRIPT_DIR}/build-app.sh"

TARGET="x86_64-pc-windows-msvc"
# Cargo builds the binary as etlp-gui.exe (package.name); PRODUCT_NAME is the
# display name used for the zip archive and the entry point inside it.
BIN_NAME="etlp-gui"
PRODUCT_NAME="Genshin"
DIST_DIR="${REPO_ROOT}/dist"

# ---------------------------------------------------------------------------
# Help
# ---------------------------------------------------------------------------

show_help() {
    cat <<'EOF'
build-windows-portable.sh — Build etlp-gui Windows Portable zip

USAGE
    bash scripts/build-windows-portable.sh [OPTIONS]

OPTIONS
    --dry-run        Print actions without making changes
    --help, -h       Show this message

REQUIREMENTS
    Windows with MSVC build tools, Node.js, Rust, and PowerShell (for zip).

OUTPUT
    dist/Genshin-portable-x64-<version>.zip
EOF
}

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

read_version() {
    # Read from workspace Cargo.toml — single source of truth.
    grep -m1 '^version' "${REPO_ROOT}/Cargo.toml" \
        | sed 's/.*"\(.*\)".*/\1/'
}

build_updater() {
    _log "Building" "updater.exe → ${TARGET}"
    _run cargo build -p etlp-updater --release --target "${TARGET}"
}

build_main_app() {
    check_node
    install_frontend_deps
    _log "Building" "${PRODUCT_NAME}.exe → ${TARGET}"
    _run_in "${GUI_DIR}" npx tauri build --target "${TARGET}" --no-bundle
}

package_portable() {
    local version="$1"
    local zip_name="Genshin-portable-x64-${version}.zip"
    local stage_dir="${DIST_DIR}/.portable-stage"
    local zip_path="${DIST_DIR}/${zip_name}"

    local tauri_target="${GUI_DIR}/src-tauri/target/${TARGET}/release"
    local main_exe="${tauri_target}/${BIN_NAME}.exe"
    local updater_exe="${REPO_ROOT}/target/${TARGET}/release/updater.exe"

    _log "Staging" "${zip_name}"

    if ! "${DRY_RUN}"; then
        rm -rf "${stage_dir}"
        mkdir -p "${stage_dir}/config" \
                 "${stage_dir}/data" \
                 "${stage_dir}/update"

        cp "${main_exe}"    "${stage_dir}/${PRODUCT_NAME}.exe"
        cp "${updater_exe}" "${stage_dir}/updater.exe"

        # portable.bin signals the Portable detection logic in dirs.rs.
        printf "" > "${stage_dir}/portable.bin"

        mkdir -p "${DIST_DIR}"

        # cygpath converts MSYS2/Git-Bash Unix paths (e.g. /d/a/…) to Windows
        # paths (D:\a\…) so PowerShell's Compress-Archive can resolve them.
        local win_stage_dir win_zip_path
        win_stage_dir="$(cygpath -w "${stage_dir}")"
        win_zip_path="$(cygpath -w "${zip_path}")"

        powershell.exe -NoProfile -Command \
            "Compress-Archive -Force -Path '${win_stage_dir}\\*' -DestinationPath '${win_zip_path}'"

        rm -rf "${stage_dir}"
    else
        printf "${C_YELLOW}%12s${C_RESET} would create %s\n" "dry-run" "${zip_path}"
    fi

    _log "Packaged" "${zip_path}"
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

main() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --dry-run) DRY_RUN=true ;;
            --help|-h) show_help; exit 0 ;;
            *)         _log_err "unknown option: $1"; show_help; exit 1 ;;
        esac
        shift
    done

    if "${DRY_RUN}"; then
        printf "${C_YELLOW}%12s${C_RESET} no changes will be made\n\n" "dry-run"
    fi

    check_rust_toolchain
    add_rust_target "${TARGET}"

    local version
    version="$(read_version)"
    _log "Version" "${version}"

    build_updater
    build_main_app
    package_portable "${version}"

    printf "\n${C_GREEN}%12s${C_RESET} build complete\n" "Finished"
}

main "$@"
