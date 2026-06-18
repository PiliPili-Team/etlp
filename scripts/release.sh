#!/usr/bin/env bash
#
# release.sh
# Bump version across all manifests, verify compilation, tag, and push.
#
# Usage: bash scripts/release.sh <version> [--dry-run] [--help]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
GUI_DIR="${REPO_ROOT}/etlp-gui"
DRY_RUN=false
VERSION=""

C_GREEN='\033[1;32m'
C_YELLOW='\033[1;33m'
C_RED='\033[1;31m'
C_CYAN='\033[1;36m'
C_RESET='\033[0m'

_log()     { printf "${C_GREEN}%12s${C_RESET} %s\n" "$1" "$2"; }
_log_warn(){ printf "${C_YELLOW}%12s${C_RESET} %s\n" "$1" "$2"; }
_log_err() { printf "${C_RED}%12s${C_RESET} %s\n" "error" "$1" >&2; }

_run() {
    if "${DRY_RUN}"; then
        printf "${C_YELLOW}%12s${C_RESET} %s\n" "dry-run" "$*"
    else
        "$@"
    fi
}

_on_exit() {
    local code=$?
    if [ "${code}" -ne 0 ]; then
        printf "${C_RED}%12s${C_RESET} exited with code %s\n" "error" "${code}" >&2
    fi
}
trap _on_exit EXIT

show_help() {
    cat <<'EOF'
release.sh — Bump version, verify compilation, create tag, and push

USAGE
    bash scripts/release.sh <version> [OPTIONS]

ARGUMENTS
    <version>    New version, e.g. v0.0.2 or 0.0.2

OPTIONS
    --dry-run    Print actions without making changes
    --help, -h   Show this message

WHAT IT DOES
    1. Verifies the git working tree is clean
    2. Bumps version in:
         Cargo.toml (workspace)
         etlp-gui/src-tauri/Cargo.toml
         etlp-gui/src-tauri/tauri.conf.json
         etlp-gui/package.json
    3. Runs cargo check --workspace (fast compile verification)
    4. On macOS: also checks etlp-gui workspace
    5. Commits the version changes
    6. Creates git tag vX.Y.Z
    7. Asks for confirmation before pushing to remote
EOF
}

_validate_version() {
    local v="${1#v}"
    if ! printf '%s' "${v}" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
        _log_err "invalid version '${1}' — expected vX.Y.Z or X.Y.Z"
        exit 1
    fi
    printf '%s' "${v}"
}

_check_git_clean() {
    if ! git -C "${REPO_ROOT}" rev-parse --git-dir > /dev/null 2>&1; then
        _log_err "not a git repository: ${REPO_ROOT}"
        exit 1
    fi
    local status
    status="$(git -C "${REPO_ROOT}" status --porcelain 2>/dev/null)"
    if [[ -n "${status}" ]]; then
        _log_err "working tree is dirty — commit or stash changes first"
        git -C "${REPO_ROOT}" status --short >&2
        exit 1
    fi
    _log "Verified" "working tree is clean"
}

_check_tag_absent() {
    local tag="$1"
    if git -C "${REPO_ROOT}" tag --list | grep -qx "${tag}"; then
        _log_err "tag ${tag} already exists"
        exit 1
    fi
}

_bump_cargo_toml() {
    local file="$1" ver="$2"
    _log "Bumping" "${file#"${REPO_ROOT}/"} → ${ver}"
    if ! "${DRY_RUN}"; then
        sed -i '' "s/^version = \"[^\"]*\"/version = \"${ver}\"/" "${file}"
    fi
}

_bump_json_version() {
    local file="$1" ver="$2"
    _log "Bumping" "${file#"${REPO_ROOT}/"} → ${ver}"
    if ! "${DRY_RUN}"; then
        # Match top-level "version" field (2-space indent) in JSON manifests
        sed -i '' "s/^  \"version\": \"[^\"]*\"/  \"version\": \"${ver}\"/" "${file}"
    fi
}

bump_versions() {
    local ver="$1"
    _bump_cargo_toml "${REPO_ROOT}/Cargo.toml"                        "${ver}"
    _bump_cargo_toml "${GUI_DIR}/src-tauri/Cargo.toml"                "${ver}"
    _bump_json_version "${GUI_DIR}/src-tauri/tauri.conf.json"         "${ver}"
    _bump_json_version "${GUI_DIR}/package.json"                      "${ver}"
}

compile_check() {
    _log "Checking" "workspace — cargo check --workspace"
    _run cargo check --workspace --manifest-path "${REPO_ROOT}/Cargo.toml"

    if [[ "$(uname -s)" == "Darwin" ]]; then
        _log "Checking" "etlp-gui — cargo check"
        _run cargo check --manifest-path "${GUI_DIR}/src-tauri/Cargo.toml"
    else
        _log_warn "Skipped" "etlp-gui check (requires macOS)"
    fi
}

commit_and_tag() {
    local ver="$1" tag="v${ver}"

    _log "Staging" "modified tracked files"
    _run git -C "${REPO_ROOT}" add -u

    _log "Committing" "chore: bump version to ${ver}"
    _run git -C "${REPO_ROOT}" commit -m "chore: bump version to ${ver}"

    _log "Tagging" "${tag}"
    _run git -C "${REPO_ROOT}" tag "${tag}"

    printf "\n"
    printf "${C_GREEN}%12s${C_RESET} ${tag}\n" "Created tag"
}

ask_push() {
    local tag="$1"
    if "${DRY_RUN}"; then
        _log_warn "dry-run" "skipping push confirmation"
        return
    fi
    printf "${C_CYAN}%12s${C_RESET} push commits + tag %s to remote? [y/N] " \
        "Confirm" "${tag}"
    read -r answer
    case "${answer}" in
        [yY]|[yY][eE][sS])
            _log "Pushing" "commits"
            git -C "${REPO_ROOT}" push
            _log "Pushing" "tag ${tag}"
            git -C "${REPO_ROOT}" push origin "${tag}"
            printf "${C_GREEN}%12s${C_RESET} %s pushed to remote\n" "Done" "${tag}"
            ;;
        *)
            _log_warn "Skipped" "push — run manually when ready:"
            printf "    git push && git push origin %s\n" "${tag}"
            ;;
    esac
}

main() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --dry-run)  DRY_RUN=true ;;
            --help|-h)  show_help; exit 0 ;;
            -*)         _log_err "unknown option: $1"; show_help; exit 1 ;;
            *)          VERSION="$1" ;;
        esac
        shift
    done

    if [[ -z "${VERSION}" ]]; then
        _log_err "version argument is required"
        printf "\n"
        show_help
        exit 1
    fi

    local ver
    ver="$(_validate_version "${VERSION}")"
    local tag="v${ver}"

    printf "${C_CYAN}%12s${C_RESET} %s\n" "Release" "${tag}"

    if "${DRY_RUN}"; then
        printf "${C_YELLOW}%12s${C_RESET} no changes will be made\n" "dry-run"
    fi

    _check_git_clean
    _check_tag_absent "${tag}"
    bump_versions "${ver}"
    compile_check
    commit_and_tag "${ver}"
    ask_push "${tag}"

    printf "${C_GREEN}%12s${C_RESET} release %s complete\n" "Finished" "${tag}"
}

main "$@"
