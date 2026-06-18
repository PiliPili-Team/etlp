#!/usr/bin/env bash
#
# run.sh — Run the etlp binary in the foreground, as a background daemon,
#          or install/remove the OS autostart entry.
#
# Usage:
#   bash scripts/run.sh [--daemon] [--install-autostart] [--remove-autostart]
#                       [--binary PATH] [--data-dir DIR] [--dry-run] [--help]
#
# Supported platforms: macOS, Linux (systemd or generic daemon via nohup).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# ── Colours (Cargo-style) ────────────────────────────────────────────────────

C_GREEN='\033[1;32m'
C_YELLOW='\033[1;33m'
C_RED='\033[1;31m'
C_RESET='\033[0m'

_log()     { printf "${C_GREEN}%12s${C_RESET} %s\n" "$1" "$2"; }
_log_skip(){ printf "${C_YELLOW}%12s${C_RESET} %s\n" "Skipping" "$1"; }
_log_err() { printf "${C_RED}%12s${C_RESET} %s\n" "error" "$1" >&2; }

_on_exit() {
    local code=$?
    if [ "${code}" -ne 0 ]; then
        printf "${C_RED}%12s${C_RESET} exited with code %s\n" "error" "${code}" >&2
    fi
}
trap _on_exit EXIT

# ── Defaults ─────────────────────────────────────────────────────────────────

DAEMON=false
INSTALL_AUTOSTART=false
REMOVE_AUTOSTART=false
DRY_RUN=false
BINARY=""
DATA_DIR=""

ETLP_BINARY_CANDIDATES=(
    "${REPO_ROOT}/target/release/etlp"
    "${REPO_ROOT}/target/x86_64-unknown-linux-gnu/release/etlp"
    "${REPO_ROOT}/target/aarch64-apple-darwin/release/etlp"
    "${REPO_ROOT}/target/x86_64-apple-darwin/release/etlp"
    "/usr/local/bin/etlp"
    "/opt/homebrew/bin/etlp"
)

# ── Helpers ───────────────────────────────────────────────────────────────────

_platform() {
    case "$(uname -s)" in
        Darwin) printf "macos" ;;
        Linux)  printf "linux" ;;
        *)      printf "unknown" ;;
    esac
}

_find_binary() {
    if [[ -n "${BINARY}" ]]; then
        if [[ -x "${BINARY}" ]]; then printf "%s" "${BINARY}"; return 0; fi
        _log_err "specified binary not found or not executable: ${BINARY}"
        return 1
    fi
    for candidate in "${ETLP_BINARY_CANDIDATES[@]}"; do
        if [[ -x "${candidate}" ]]; then printf "%s" "${candidate}"; return 0; fi
    done
    _log_err "etlp binary not found; build it first with scripts/build-macos-binary.sh"
    return 1
}

_run() {
    if "${DRY_RUN}"; then
        printf "${C_YELLOW}%12s${C_RESET} %s\n" "dry-run" "$*"
    else
        "$@"
    fi
}

# ── macOS LaunchAgent ─────────────────────────────────────────────────────────

_macos_plist_path() {
    printf "%s/Library/LaunchAgents/com.pilipili.etlp.plist" "${HOME}"
}

_macos_install_autostart() {
    local binary="$1"
    local plist
    plist="$(_macos_plist_path)"

    _log "Installing" "LaunchAgent → ${plist}"
    local data_dir_arg=""
    [[ -n "${DATA_DIR}" ]] && data_dir_arg="<string>--data-dir</string><string>${DATA_DIR}</string>"

    if ! "${DRY_RUN}"; then
        mkdir -p "$(dirname "${plist}")"
        cat > "${plist}" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
    "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>              <string>com.pilipili.etlp</string>
    <key>ProgramArguments</key>
    <array>
        <string>${binary}</string>
        ${data_dir_arg}
    </array>
    <key>RunAtLoad</key>          <true/>
    <key>KeepAlive</key>          <true/>
    <key>StandardOutPath</key>    <string>${HOME}/.local/share/etlp/stdout.log</string>
    <key>StandardErrorPath</key>  <string>${HOME}/.local/share/etlp/stderr.log</string>
</dict>
</plist>
PLIST
    fi

    _run launchctl load "${plist}"
    _log "Done" "etlp will start automatically at login (launchctl)"
}

_macos_remove_autostart() {
    local plist
    plist="$(_macos_plist_path)"
    if [[ ! -f "${plist}" ]]; then
        _log_skip "LaunchAgent not installed (${plist} not found)"; return
    fi
    _log "Removing" "LaunchAgent ${plist}"
    _run launchctl unload "${plist}" 2>/dev/null || true
    _run rm -f "${plist}"
    _log "Done" "autostart removed"
}

# ── Linux systemd user service ────────────────────────────────────────────────

_linux_service_path() {
    printf "%s/.config/systemd/user/etlp.service" "${HOME}"
}

_linux_install_autostart() {
    local binary="$1"
    local svc
    svc="$(_linux_service_path)"

    _log "Installing" "systemd user service → ${svc}"
    local exec_start="${binary}"
    [[ -n "${DATA_DIR}" ]] && exec_start="${binary} --data-dir ${DATA_DIR}"

    if ! "${DRY_RUN}"; then
        mkdir -p "$(dirname "${svc}")"
        cat > "${svc}" <<UNIT
[Unit]
Description=etlp — emby to local player relay
After=network.target

[Service]
Type=simple
ExecStart=${exec_start}
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
UNIT
    fi

    _run systemctl --user daemon-reload
    _run systemctl --user enable --now etlp
    _log "Done" "etlp systemd user service enabled and started"
}

_linux_remove_autostart() {
    local svc
    svc="$(_linux_service_path)"
    if [[ ! -f "${svc}" ]]; then
        _log_skip "systemd service not installed (${svc} not found)"; return
    fi
    _log "Removing" "systemd user service"
    _run systemctl --user disable --now etlp 2>/dev/null || true
    _run rm -f "${svc}"
    _run systemctl --user daemon-reload
    _log "Done" "autostart removed"
}

# ── Help ──────────────────────────────────────────────────────────────────────

show_help() {
    cat <<'EOF'
run.sh — Run the etlp binary or manage OS autostart

USAGE
    bash scripts/run.sh [OPTIONS]

OPTIONS
    --daemon              Run etlp in the background (nohup)
    --install-autostart   Install OS-level autostart (LaunchAgent / systemd)
    --remove-autostart    Remove OS-level autostart
    --binary PATH         Path to the etlp binary (auto-detected if absent)
    --data-dir DIR        Override data directory passed to etlp
    --dry-run             Print actions without executing
    --help, -h            Show this help

EXAMPLES
    # Run in foreground
    bash scripts/run.sh

    # Run as daemon
    bash scripts/run.sh --daemon

    # Install autostart (macOS LaunchAgent / Linux systemd)
    bash scripts/run.sh --install-autostart

    # Remove autostart
    bash scripts/run.sh --remove-autostart
EOF
}

# ── Argument parsing ──────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
    case "$1" in
        --daemon)              DAEMON=true ;;
        --install-autostart)   INSTALL_AUTOSTART=true ;;
        --remove-autostart)    REMOVE_AUTOSTART=true ;;
        --binary)              shift; BINARY="$1" ;;
        --binary=*)            BINARY="${1#*=}" ;;
        --data-dir)            shift; DATA_DIR="$1" ;;
        --data-dir=*)          DATA_DIR="${1#*=}" ;;
        --dry-run)             DRY_RUN=true ;;
        --help|-h)             show_help; exit 0 ;;
        *)                     _log_err "unknown option: $1"; show_help; exit 1 ;;
    esac
    shift
done

# ── Main ──────────────────────────────────────────────────────────────────────

main() {
    local platform
    platform="$(_platform)"
    local binary
    binary="$(_find_binary)"

    if "${DRY_RUN}"; then
        printf "${C_YELLOW}%12s${C_RESET} no changes will be made\n" "dry-run"
    fi

    if "${INSTALL_AUTOSTART}"; then
        case "${platform}" in
            macos) _macos_install_autostart "${binary}" ;;
            linux) _linux_install_autostart "${binary}" ;;
            *)     _log_err "autostart not supported on ${platform}"; exit 1 ;;
        esac
        return
    fi

    if "${REMOVE_AUTOSTART}"; then
        case "${platform}" in
            macos) _macos_remove_autostart ;;
            linux) _linux_remove_autostart ;;
            *)     _log_err "autostart not supported on ${platform}"; exit 1 ;;
        esac
        return
    fi

    # Build the argument list.
    local -a args=()
    [[ -n "${DATA_DIR}" ]] && args+=("--data-dir" "${DATA_DIR}")

    if "${DAEMON}"; then
        local log_dir="${DATA_DIR:-${HOME}/.local/share/etlp}"
        mkdir -p "${log_dir}"
        local log_out="${log_dir}/stdout.log"
        local log_err="${log_dir}/stderr.log"
        _log "Starting" "etlp as daemon (log → ${log_out})"
        _run nohup "${binary}" "${args[@]}" > "${log_out}" 2> "${log_err}" &
        _log "Running" "PID $!"
    else
        _log "Starting" "${binary} ${args[*]}"
        exec "${binary}" "${args[@]}"
    fi
}

main
