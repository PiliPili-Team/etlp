#!/usr/bin/env bash
#
# check-file-lines.sh
# Detect Rust source files that exceed the configured line-count limit.
#
# Config: reads max_file_lines from .rustlint.toml in the repo root;
#         defaults to 500 when the key is absent or the file does not exist.
#
# Usage:
#   bash scripts/check-file-lines.sh [FILE_LIST] [--dry-run] [--help]
#
#   FILE_LIST  Path to a plain-text file with one .rs path per line.
#              Blank lines and lines starting with # are ignored.
#              When omitted, every .rs file in the repository is scanned
#              (build artefacts under target/ are excluded).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=build-app.sh
source "${SCRIPT_DIR}/build-app.sh"

DEFAULT_MAX_LINES=500
CONFIG_FILE="${REPO_ROOT}/.rustlint.toml"
CONFIG_KEY="max_file_lines"
FILE_LIST=""

show_help() {
    cat <<'EOF'
check-file-lines.sh — Detect Rust files exceeding the configured line limit

USAGE
    bash scripts/check-file-lines.sh [FILE_LIST] [OPTIONS]

ARGUMENTS
    FILE_LIST   Path to a plain-text file containing one .rs path per line.
                Blank lines and lines starting with # are ignored.
                When omitted, all .rs files in the repository are scanned
                (target/ is excluded).

OPTIONS
    --dry-run   Print the configuration and files that would be checked
                without writing a report or emitting a failure exit code.
    --help, -h  Show this message.

CONFIG
    Read max_file_lines from .rustlint.toml in the repository root.
    Defaults to 500 when the key is absent or the file does not exist.

EXIT CODES
    0   No violations — CI passing.
    1   One or more files exceed the limit — CI failed.

EXAMPLES
    bash scripts/check-file-lines.sh                # scan all .rs files
    bash scripts/check-file-lines.sh changed.txt    # scan listed files only
EOF
}

_read_max_lines() {
    if [[ ! -f "${CONFIG_FILE}" ]]; then
        printf "%d" "${DEFAULT_MAX_LINES}"
        return
    fi
    local val
    val=$(grep -E "^[[:space:]]*${CONFIG_KEY}[[:space:]]*=" "${CONFIG_FILE}" \
        | sed -E 's/.*=[[:space:]]*([0-9]+).*/\1/' \
        | head -1)
    if [[ -z "${val}" || ! "${val}" =~ ^[0-9]+$ ]]; then
        printf "%d" "${DEFAULT_MAX_LINES}"
    else
        printf "%d" "${val}"
    fi
}

_collect_files() {
    if [[ -n "${FILE_LIST}" ]]; then
        if [[ ! -f "${FILE_LIST}" ]]; then
            _log_err "file list not found: ${FILE_LIST}"
            exit 1
        fi
        grep -v -E '^\s*(#|$)' "${FILE_LIST}" | grep '\.rs$' || true
    else
        find "${REPO_ROOT}" -type f -name "*.rs" \
            -not -path "*/target/*" \
            | sort
    fi
}

main() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --dry-run) DRY_RUN=true ;;
            --help|-h) show_help; exit 0 ;;
            -*)
                _log_err "unknown option: $1"
                show_help
                exit 1
                ;;
            *)
                if [[ -n "${FILE_LIST}" ]]; then
                    _log_err "unexpected argument: $1"
                    show_help
                    exit 1
                fi
                FILE_LIST="$1"
                ;;
        esac
        shift
    done

    local max_lines
    max_lines="$(_read_max_lines)"

    if [[ -f "${CONFIG_FILE}" ]]; then
        _log "Config" "${CONFIG_FILE} → ${CONFIG_KEY} = ${max_lines}"
    else
        _log "Config" "no .rustlint.toml found — using default (${max_lines} lines)"
    fi

    local source_label
    if [[ -n "${FILE_LIST}" ]]; then
        source_label="${FILE_LIST}"
    else
        source_label="all .rs files in repository"
    fi

    if "${DRY_RUN}"; then
        _log "Scanning" "${source_label}"
        local count
        count=$(_collect_files | wc -l | tr -d ' ')
        _log "Found" "${count} file(s) to check"
        printf "${C_YELLOW}%12s${C_RESET} no report written\n" "dry-run"
        exit 0
    fi

    local tmp_dir
    tmp_dir="$(mktemp -d)"
    local report="${tmp_dir}/rust-file-lines-report.txt"
    local violation_count=0

    _log "Scanning" "${source_label}"

    while IFS= read -r file; do
        [[ -z "${file}" ]] && continue
        if [[ "${file}" != /* ]]; then
            file="${REPO_ROOT}/${file}"
        fi
        if [[ ! -f "${file}" ]]; then
            _log_err "file not found: ${file}"
            continue
        fi
        local line_count
        line_count=$(wc -l < "${file}" | tr -d ' ')
        if (( line_count > max_lines )); then
            printf "%s\t%d\n" "${file}" "${line_count}" >> "${report}"
            violation_count=$((violation_count + 1))
        fi
    done < <(_collect_files)

    printf "\n"

    if (( violation_count == 0 )); then
        printf "${C_GREEN}%12s${C_RESET} all files within the %d-line limit\n" \
            "Finished" "${max_lines}"
        printf "${C_GREEN}%12s${C_RESET} %s\n" "Report" "${tmp_dir}"
        exit 0
    fi

    _log_err "${violation_count} file(s) exceed the ${max_lines}-line limit"
    printf "\n"
    while IFS=$'\t' read -r path lines; do
        printf "${C_RED}%12s${C_RESET} %s (%d lines)\n" \
            "overlimit" "${path#"${REPO_ROOT}"/}" "${lines}"
    done < "${report}"
    printf "\n"
    printf "${C_RED}%12s${C_RESET} %s\n" "Report" "${tmp_dir}"
    exit 1
}

main "$@"
