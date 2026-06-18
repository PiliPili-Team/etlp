#!/usr/bin/env bash
#
# build.sh
# Main build entry point for etlp.
# Detects the current platform and delegates to the appropriate build script.
#
# Usage: bash scripts/build.sh [--mode binary|app] [--arch amd64|arm64|all]
#                              [--platform macos|linux|windows]
#                              [--dry-run] [--help]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

C_GREEN='\033[1;32m'
C_YELLOW='\033[1;33m'
C_RED='\033[1;31m'
C_RESET='\033[0m'

_log()     { printf "${C_GREEN}%12s${C_RESET} %s\n" "$1" "$2"; }
_log_err() { printf "${C_RED}%12s${C_RESET} %s\n" "error" "$1" >&2; }

_on_exit() {
    local code=$?
    if [ "${code}" -ne 0 ]; then
        printf "\n${C_RED}%12s${C_RESET} exited with code %s\n" "error" "${code}" >&2
    fi
}
trap _on_exit EXIT

MODE="binary"
PLATFORM=""
ARCH_ARGS=()
DRY_ARGS=()

show_help() {
    cat <<'EOF'
build.sh — etlp build entry point

USAGE
    bash scripts/build.sh [OPTIONS]

OPTIONS
    --mode binary|app            What to build (default: binary)
    --platform macos|linux|windows
                                 Target platform (default: auto-detect host OS)
    --arch amd64|arm64|all       Architecture (default: native host arch)
    --dry-run                    Print actions without making changes
    --help, -h                   Show this message

EXAMPLES
    bash scripts/build.sh
    bash scripts/build.sh --mode app --arch all
    bash scripts/build.sh --mode binary --arch all --platform macos
    bash scripts/build.sh --mode app --platform linux --dry-run
EOF
}

_detect_platform() {
    case "$(uname -s)" in
        Darwin)            printf "macos"   ;;
        Linux)             printf "linux"   ;;
        MINGW*|MSYS*|CYGWIN*) printf "windows" ;;
        *)
            _log_err "unsupported OS: $(uname -s)"
            exit 1
            ;;
    esac
}

main() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --mode)         shift; MODE="$1" ;;
            --mode=*)       MODE="${1#*=}" ;;
            --platform)     shift; PLATFORM="$1" ;;
            --platform=*)   PLATFORM="${1#*=}" ;;
            --arch)         shift; ARCH_ARGS=(--arch "$1") ;;
            --arch=*)       ARCH_ARGS=(--arch "${1#*=}") ;;
            --dry-run)      DRY_ARGS=(--dry-run) ;;
            --help|-h)      show_help; exit 0 ;;
            *)              _log_err "unknown option: $1"; show_help; exit 1 ;;
        esac
        shift
    done

    [[ -z "${PLATFORM}" ]] && PLATFORM="$(_detect_platform)"

    case "${MODE}" in
        binary|app) ;;
        *) _log_err "unknown mode: ${MODE} (expected binary or app)"; exit 1 ;;
    esac

    local script="${SCRIPT_DIR}/build-${PLATFORM}-${MODE}.sh"
    if [[ ! -f "${script}" ]]; then
        _log_err "no build script found: scripts/build-${PLATFORM}-${MODE}.sh"
        exit 1
    fi

    _log "Platform" "${PLATFORM}"
    _log "Mode"     "${MODE}"

    bash "${script}" \
        ${ARCH_ARGS[@]+"${ARCH_ARGS[@]}"} \
        ${DRY_ARGS[@]+"${DRY_ARGS[@]}"}
}

main "$@"
