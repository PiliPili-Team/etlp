#!/usr/bin/env bash

# gen-changelog.sh — Build GitHub release notes from the commits between the
# previous tag and the current one, grouped by Conventional Commit type.
#
# The changelog is printed to stdout. When no previous tag can be resolved
# (e.g. the very first release) nothing is printed and the script exits 0, so
# callers can treat an empty result as "do not write a changelog".

set -euo pipefail

# ── Logging (Cargo-style, to stderr so stdout stays clean) ─────────────────────

readonly C_GREEN=$'\033[1;32m'
readonly C_RED=$'\033[1;31m'
readonly C_DIM=$'\033[2m'
readonly C_RESET=$'\033[0m'

log_step() { printf '%s%12s%s %s\n' "$C_GREEN" "$1" "$C_RESET" "$2" >&2; }
log_error() { printf '%serror%s %s\n' "$C_RED" "$C_RESET" "$1" >&2; }
log_note() { printf '%s%12s %s%s\n' "$C_DIM" "$1" "$2" "$C_RESET" >&2; }

# ── CLI ────────────────────────────────────────────────────────────────────────

usage() {
    cat <<'EOF'
Usage: gen-changelog.sh [--tag <tag>] [--repo <owner/repo>] [--dry-run] [--help]

Generate grouped release notes between the previous git tag and the current one.

Options:
  --tag <tag>          Current tag (default: $GITHUB_REF_NAME, else the tag at HEAD)
  --repo <owner/repo>  Repository slug for the compare link
                       (default: $GITHUB_REPOSITORY, else parsed from origin)
  --dry-run            Print the resolved tags/range to stderr and exit without
                       emitting the changelog
  --help               Show this help

Prints nothing and exits 0 when no previous tag exists.
EOF
}

TAG=""
REPO=""
DRY_RUN=0

parse_args() {
    while [ $# -gt 0 ]; do
        case "$1" in
            --tag) TAG="${2:-}"; shift 2 ;;
            --repo) REPO="${2:-}"; shift 2 ;;
            --dry-run) DRY_RUN=1; shift ;;
            --help | -h) usage; exit 0 ;;
            *) log_error "unknown argument: $1"; usage; exit 2 ;;
        esac
    done
}

# ── Resolution helpers ─────────────────────────────────────────────────────────

# Resolve the current tag from --tag, the CI ref, or the tag pointing at HEAD.
resolve_current_tag() {
    if [ -n "$TAG" ]; then
        printf '%s' "$TAG"
        return 0
    fi
    if [ -n "${GITHUB_REF_NAME:-}" ] && [ "${GITHUB_REF_TYPE:-}" = "tag" ]; then
        printf '%s' "$GITHUB_REF_NAME"
        return 0
    fi
    git describe --tags --exact-match HEAD 2>/dev/null || true
}

# Resolve the most recent tag reachable before <current>. Empty when none.
resolve_previous_tag() {
    local current="$1"
    git describe --tags --abbrev=0 "${current}^" 2>/dev/null || true
}

# Resolve the owner/repo slug for the compare URL.
resolve_repo() {
    if [ -n "$REPO" ]; then
        printf '%s' "$REPO"
        return 0
    fi
    if [ -n "${GITHUB_REPOSITORY:-}" ]; then
        printf '%s' "$GITHUB_REPOSITORY"
        return 0
    fi
    local url
    url="$(git remote get-url origin 2>/dev/null || true)"
    url="${url%.git}"
    case "$url" in
        git@*:*) printf '%s' "${url##*:}" ;;
        https://*) printf '%s' "${url#https://*/}" ;;
        *) printf '%s' "" ;;
    esac
}

# Section header for a Conventional Commit type; unknown types map to "other".
section_for() {
    case "$1" in
        feat) printf '✨ Features' ;;
        fix) printf '🐛 Bug Fixes' ;;
        perf) printf '⚡ Performance' ;;
        refactor) printf '♻️ Refactoring' ;;
        docs) printf '📝 Documentation' ;;
        test) printf '✅ Tests' ;;
        build) printf '👷 Build System' ;;
        ci) printf '👷 CI' ;;
        chore) printf '🔧 Chores' ;;
        style) printf '🎨 Styles' ;;
        revert) printf '⏪ Reverts' ;;
        *) printf '📦 Other Changes' ;;
    esac
}

# Display order of sections (keys); "other" is always last.
readonly SECTION_ORDER=(
    feat fix perf refactor docs test build ci chore style revert other
)

# Known Conventional Commit types, alternated for matching the "other" bucket.
readonly KNOWN_TYPES_RE='feat|fix|perf|refactor|docs|test|build|ci|chore|style|revert'

# ── Changelog generation ───────────────────────────────────────────────────────

# Print "- message" bullets for one section <type> from the commit subjects on
# stdin's <commits>. For a known type, lines are matched by their prefix and the
# prefix is stripped; for "other", lines that match no known type are kept (any
# stray prefix is stripped). Uses portable grep/sed (no bash 4 features).
bullets_for() {
    local type="$1" commits="$2"
    if [ "$type" = "other" ]; then
        printf '%s\n' "$commits" \
            | grep -Ev "^(${KNOWN_TYPES_RE})(\([^)]*\))?!?:" \
            | sed -E 's/^[a-zA-Z]+(\([^)]*\))?!?:[[:space:]]*//' \
            | sed -E '/^[[:space:]]*$/d; s/^/- /' || true
    else
        printf '%s\n' "$commits" \
            | grep -E "^${type}(\([^)]*\))?!?:" \
            | sed -E "s/^${type}(\([^)]*\))?!?:[[:space:]]*//" \
            | sed -E '/^[[:space:]]*$/d; s/^/- /' || true
    fi
}

# Emit the changelog for <prev>..<tag> to stdout.
generate() {
    local prev="$1" tag="$2" repo="$3"
    local server="${GITHUB_SERVER_URL:-https://github.com}"

    local commits
    commits="$(git log --no-merges --pretty=format:%s "${prev}..${tag}")"

    printf "## What's Changed\n\n"
    local k body
    for k in "${SECTION_ORDER[@]}"; do
        body="$(bullets_for "$k" "$commits")"
        if [ -n "$body" ]; then
            printf '### %s\n%s\n\n' "$(section_for "$k")" "$body"
        fi
    done
    if [ -n "$repo" ]; then
        printf '**Full Changelog**: %s/%s/compare/%s...%s\n' \
            "$server" "$repo" "$prev" "$tag"
    fi
}

# ── Orchestration ──────────────────────────────────────────────────────────────

main() {
    parse_args "$@"

    local current prev repo
    current="$(resolve_current_tag)"
    if [ -z "$current" ]; then
        log_error "could not resolve the current tag"
        exit 1
    fi
    prev="$(resolve_previous_tag "$current")"
    repo="$(resolve_repo)"

    log_note "current" "$current"
    log_note "previous" "${prev:-<none>}"
    log_note "repo" "${repo:-<unknown>}"

    if [ -z "$prev" ]; then
        log_step "Skipping" "no previous tag — changelog omitted"
        exit 0
    fi

    if [ "$DRY_RUN" -eq 1 ]; then
        log_step "Dry-run" "would diff ${prev}..${current}"
        exit 0
    fi

    log_step "Generating" "changelog for ${prev}..${current}"
    generate "$prev" "$current" "$repo"
}

main "$@"
