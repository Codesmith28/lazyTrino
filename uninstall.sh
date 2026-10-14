#!/usr/bin/env sh
# Copyright 2026 Sarthak Siddhpura
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

set -eu

# Terminal color codes
if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
    ESC="$(printf '\033')"
    CYAN="${ESC}[0;36m"
    GREEN="${ESC}[0;32m"
    YELLOW="${ESC}[0;33m"
    RED="${ESC}[0;31m"
    BOLD="${ESC}[1m"
    DIM="${ESC}[2m"
    RESET="${ESC}[0m"
else
    CYAN=''
    GREEN=''
    YELLOW=''
    RED=''
    BOLD=''
    DIM=''
    RESET=''
fi

info() {
    printf "${CYAN}${BOLD}➜${RESET} %s\n" "$1"
}

success() {
    printf "${GREEN}${BOLD}✔${RESET} %s\n" "$1"
}

warn() {
    printf "${YELLOW}${BOLD}⚠${RESET} %s\n" "$1"
}

error() {
    printf "${RED}${BOLD}✖${RESET} %s\n" "$1" >&2
}

main() {
    PURGE=0
    for arg in "$@"; do
        case "$arg" in
        --purge | -p | --all)
            PURGE=1
            ;;
        --help | -h)
            printf "Usage: uninstall.sh [--purge]\n"
            printf "  --purge, -p    Also remove configuration and saved views (~/.config/lazytrino)\n"
            exit 0
            ;;
        esac
    done

    printf "\n${BOLD}lazyTrino Uninstaller${RESET}\n\n"

    FOUND=0
    LOCATIONS="${HOME}/.local/bin/lazyTrino /usr/local/bin/lazyTrino"

    for bin in $LOCATIONS; do
        if [ -f "$bin" ]; then
            FOUND=1
            info "Removing binary: ${bin}..."
            if [ -w "$bin" ] || [ -w "$(dirname "$bin")" ]; then
                rm -f "$bin"
                success "Removed ${bin}"
            elif command -v sudo >/dev/null 2>&1; then
                info "Prompting for sudo to remove ${bin}..."
                sudo rm -f "$bin"
                success "Removed ${bin}"
            else
                error "Permission denied: unable to remove ${bin}. Try running with sudo."
            fi
        fi
    done

    # Also check if any lazyTrino remains in PATH
    REMAINING="$(command -v lazyTrino 2>/dev/null || true)"
    if [ -n "$REMAINING" ] && [ -f "$REMAINING" ]; then
        warn "Found additional lazyTrino binary at: ${REMAINING}"
        if [ -w "$REMAINING" ] || [ -w "$(dirname "$REMAINING")" ]; then
            rm -f "$REMAINING"
            success "Removed ${REMAINING}"
        elif command -v sudo >/dev/null 2>&1; then
            info "Prompting for sudo to remove ${REMAINING}..."
            sudo rm -f "$REMAINING"
            success "Removed ${REMAINING}"
        fi
    fi

    CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/lazytrino"
    if [ -d "$CONFIG_DIR" ]; then
        if [ "$PURGE" -eq 1 ]; then
            info "Removing configuration directory: ${CONFIG_DIR}..."
            rm -rf "$CONFIG_DIR"
            success "Removed ${CONFIG_DIR}"
        else
            printf "\n"
            info "Configuration directory preserved: ${CONFIG_DIR}"
            printf "  ${DIM}To also remove configuration and saved views, run with --purge:${RESET}\n"
            printf "  ${BOLD}rm -rf \"%s\"${RESET}\n" "$CONFIG_DIR"
        fi
    fi

    printf "\n"
    if [ "$FOUND" -eq 1 ] || [ -n "$REMAINING" ]; then
        success "lazyTrino has been successfully uninstalled."
    else
        warn "lazyTrino was not found in standard installation directories."
    fi
    printf "\n"
}

main "$@"
