#!/usr/bin/env bash
#
# lint.sh
# Run the project's check + lint suites: native Rust CI, the web frontend, and
# the Windows-gnu cross-compile. Each suite can be run on its own, in any
# combination, or all together (the default).
#
# Usage: bash scripts/lint.sh [ci] [web] [windows-gnu] [--dry-run] [--help]
#
# Note: this is a stateless, idempotent linter — it deliberately keeps no
# resume/lock file, since re-running after a fix must re-check from scratch.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=build-app.sh
source "${SCRIPT_DIR}/build-app.sh"

TAURI_DIR="${GUI_DIR}/src-tauri"
WIN_TARGET="x86_64-pc-windows-gnu"

# Suites selected on the command line; empty means "all".
SELECTED=()
# Names of suites that failed, for the final summary.
FAILED=()

show_help() {
    cat <<'EOF'
lint.sh — Run etlp check + lint suites

USAGE
    bash scripts/lint.sh [SUITES...] [OPTIONS]

SUITES (run all when none are given)
    ci             Native Rust: fmt, clippy (-D warnings) and tests for the
                   workspace and the etlp-gui crate.
    web            Frontend: tsc type-check, eslint and prettier.
    windows-gnu    Cross-compile the etlp-gui crate to x86_64-pc-windows-gnu
                   and run clippy (-D warnings). Aliases: windows, win, gnu.

OPTIONS
    --dry-run      Print the commands without running them.
    --help, -h     Show this message.

EXAMPLES
    bash scripts/lint.sh                 # run every suite
    bash scripts/lint.sh ci              # only the native Rust suite
    bash scripts/lint.sh web windows-gnu # two suites
EOF
}

# Normalise a user-supplied suite name to its canonical form, or empty if it is
# not a known suite.
_canon_suite() {
    case "$1" in
        ci)                       printf "ci" ;;
        web)                      printf "web" ;;
        windows-gnu|windows|win|gnu|cross) printf "windows-gnu" ;;
        *)                        printf "" ;;
    esac
}

# Run one labelled step, recording a failure without aborting the whole run so
# every selected check reports its own result.
_step() {
    local label="$1"; shift
    _log "Checking" "${label}"
    if _run "$@"; then
        return 0
    fi
    _log_err "${label} failed"
    return 1
}

# Like _step but executes inside a working directory.
_step_in() {
    local label="$1" dir="$2"; shift 2
    _log "Checking" "${label}"
    if _run_in "${dir}" "$@"; then
        return 0
    fi
    _log_err "${label} failed"
    return 1
}

# Ensure the web frontend dependencies are present before linting them; only
# installs when node_modules is missing so repeat runs stay fast.
_ensure_frontend_deps() {
    if [[ -d "${GUI_DIR}/node_modules" ]]; then
        _log_skip "frontend deps (node_modules present)"
    else
        install_frontend_deps
    fi
}

# Verify the mingw-w64 cross toolchain is installed for the Windows-gnu build.
_require_mingw() {
    if command -v x86_64-w64-mingw32-gcc > /dev/null 2>&1; then
        return 0
    fi
    _log_err "x86_64-w64-mingw32-gcc not found — install mingw-w64"
    _log_err "  macOS: brew install mingw-w64"
    _log_err "  Debian/Ubuntu: apt-get install mingw-w64"
    return 1
}

# ── Suites ──────────────────────────────────────────────────────────────────

suite_ci() {
    check_rust_toolchain
    local rc=0
    # Main workspace (crates/*).
    _step "fmt (workspace)" \
        cargo fmt --all --manifest-path "${REPO_ROOT}/Cargo.toml" -- --check \
        || rc=1
    _step "clippy (workspace)" \
        cargo clippy --manifest-path "${REPO_ROOT}/Cargo.toml" \
        --workspace --all-targets -- -D warnings || rc=1
    _step "test (workspace)" \
        cargo test --manifest-path "${REPO_ROOT}/Cargo.toml" --workspace \
        || rc=1
    # The etlp-gui crate is its own isolated workspace, linted separately.
    _step_in "fmt (etlp-gui)" "${TAURI_DIR}" \
        cargo fmt --all -- --check || rc=1
    _step_in "clippy (etlp-gui)" "${TAURI_DIR}" \
        cargo clippy --all-targets -- -D warnings || rc=1
    _step_in "test (etlp-gui)" "${TAURI_DIR}" \
        cargo test || rc=1
    return "${rc}"
}

suite_web() {
    check_node
    _ensure_frontend_deps
    local rc=0
    _step_in "tsc (web)" "${GUI_DIR}" \
        npx tsc --noEmit || rc=1
    _step_in "eslint (web)" "${GUI_DIR}" \
        npm run --silent lint || rc=1
    _step_in "prettier (web)" "${GUI_DIR}" \
        npm run --silent format:check || rc=1
    return "${rc}"
}

suite_windows_gnu() {
    check_rust_toolchain
    _require_mingw || return 1
    add_rust_target "${WIN_TARGET}"
    # The mingw cross toolchain: msvc fails on aws-lc-sys (needs windows.h), so
    # the gnu target with mingw-w64 is used instead.
    _step_in "clippy (${WIN_TARGET})" "${TAURI_DIR}" \
        env CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc \
            CXX_x86_64_pc_windows_gnu=x86_64-w64-mingw32-g++ \
            AR_x86_64_pc_windows_gnu=x86_64-w64-mingw32-ar \
            CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
            cargo clippy --target "${WIN_TARGET}" --all-targets -- -D warnings
}

# Dispatch one canonical suite name to its function.
_run_suite() {
    case "$1" in
        ci)          suite_ci ;;
        web)         suite_web ;;
        windows-gnu) suite_windows_gnu ;;
    esac
}

main() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --dry-run) DRY_RUN=true ;;
            --help|-h) show_help; exit 0 ;;
            all)       SELECTED=() ;;
            -*)        _log_err "unknown option: $1"; show_help; exit 1 ;;
            *)
                local canon
                canon="$(_canon_suite "$1")"
                if [[ -z "${canon}" ]]; then
                    _log_err "unknown suite: $1"
                    show_help
                    exit 1
                fi
                SELECTED+=("${canon}")
                ;;
        esac
        shift
    done

    # No suite named → run all three in a stable order.
    if [[ ${#SELECTED[@]} -eq 0 ]]; then
        SELECTED=(ci web windows-gnu)
    fi

    if "${DRY_RUN}"; then
        printf "${C_YELLOW}%12s${C_RESET} no checks will be run\n\n" "dry-run"
    fi

    for suite in "${SELECTED[@]}"; do
        printf "\n${C_GREEN}%12s${C_RESET} suite: %s\n" "Running" "${suite}"
        if ! _run_suite "${suite}"; then
            FAILED+=("${suite}")
        fi
    done

    printf "\n"
    if [[ ${#FAILED[@]} -eq 0 ]]; then
        printf "${C_GREEN}%12s${C_RESET} all checks passed\n" "Finished"
    else
        _log_err "failed suites: ${FAILED[*]}"
        exit 1
    fi
}

main "$@"
