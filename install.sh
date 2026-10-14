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
    CYAN='\033[0;36m'
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    RED='\033[0;31m'
    BOLD='\033[1m'
    DIM='\033[2m'
    ITALIC='\033[3m'
    RESET='\033[0m'
else
    CYAN=''
    GREEN=''
    YELLOW=''
    RED=''
    BOLD=''
    DIM=''
    ITALIC=''
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

print_banner() {
    printf "${CYAN}${BOLD}${ITALIC}"
    cat <<'EOF'
 _                      _____    _             
| |                    |_   _|  (_)            
| | __ _ _____   _ ______| |_ __ _ _ __   ___  
| |/ _` |_  / | | |______| | '__| | '_ \ / _ \ 
| | (_| |/ /| |_| |      | | |  | | | | | (_) |
|_|\__,_/___|\__, |      \_/_|  |_|_| |_|\___/ 
              __/ |                            
             |___/                             

EOF
    printf "${RESET}\n"
    printf "${DIM}${ITALIC}Interactive Terminal UI for Trino & Presto${RESET}\n\n"
}

detect_os() {
    OS="$(uname -s)"
    case "$OS" in
    Linux | linux)
        OS="linux"
        ;;
    Darwin | darwin)
        OS="darwin"
        ;;
    *)
        error "Unsupported operating system: $OS"
        error "lazyTrino currently supports Linux and macOS (Darwin)."
        exit 1
        ;;
    esac
}

detect_arch() {
    ARCH="$(uname -m)"
    case "$ARCH" in
    x86_64 | amd64)
        ARCH="x86_64"
        ;;
    aarch64 | arm64)
        ARCH="aarch64"
        ;;
    *)
        error "Unsupported architecture: $ARCH"
        error "lazyTrino currently supports x86_64 and aarch64 (ARM64)."
        exit 1
        ;;
    esac
}

check_dependencies() {
    if command -v curl >/dev/null 2>&1; then
        HTTP_CLIENT="curl"
    elif command -v wget >/dev/null 2>&1; then
        HTTP_CLIENT="wget"
    else
        error "Neither 'curl' nor 'wget' was found on your system."
        error "Please install curl or wget and try running the installer again."
        exit 1
    fi

    if ! command -v tar >/dev/null 2>&1; then
        error "'tar' command not found. Please install tar to extract release packages."
        exit 1
    fi
}

http_get() {
    url="$1"
    if [ "$HTTP_CLIENT" = "curl" ]; then
        curl -fsSL -H "User-Agent: lazyTrino-installer" "$url"
    else
        wget -qO- --user-agent="lazyTrino-installer" "$url"
    fi
}

http_download() {
    url="$1"
    dest="$2"
    if [ "$HTTP_CLIENT" = "curl" ]; then
        curl -fL -H "User-Agent: lazyTrino-installer" --progress-bar -o "$dest" "$url"
    else
        wget -q --show-progress --user-agent="lazyTrino-installer" -O "$dest" "$url"
    fi
}

main() {
    print_banner
    check_dependencies

    info "Detecting system platform..."
    detect_os
    detect_arch
    printf "  ${DIM}OS:${RESET}   ${BOLD}%s${RESET}\n" "$OS"
    printf "  ${DIM}Arch:${RESET} ${BOLD}%s${RESET}\n\n" "$ARCH"

    REPO="Codesmith28/lazyTrino"
    TARGET_TAG="${LAZYTRINO_VERSION:-}"

    if [ -n "$TARGET_TAG" ]; then
        info "Using requested version: ${TARGET_TAG}"
        RELEASE_API="https://api.github.com/repos/${REPO}/releases/tags/${TARGET_TAG}"
    else
        info "Fetching latest release information from GitHub..."
        RELEASE_API="https://api.github.com/repos/${REPO}/releases/latest"
    fi

    RELEASE_JSON="$(http_get "$RELEASE_API" 2>/dev/null || true)"

    if [ -z "$RELEASE_JSON" ] || echo "$RELEASE_JSON" | grep -q '"message": "Not Found"'; then
        warn "Could not fetch release metadata from GitHub API (release may not be published yet or rate limit reached)."
        warn "Attempting fallback to build from source or default binary asset..."
        TAG="v0.1.0"
    else
        TAG="$(echo "$RELEASE_JSON" | grep -o '"tag_name": *"[^"]*"' | head -n 1 | cut -d'"' -f4)"
    fi

    [ -z "$TAG" ] && TAG="v0.1.0"
    success "Target release version: ${BOLD}${TAG}${RESET}"

    # Determine target triple
    if [ "$OS" = "linux" ] && [ "$ARCH" = "x86_64" ]; then
        TARGET="x86_64-unknown-linux-gnu"
    elif [ "$OS" = "linux" ] && [ "$ARCH" = "aarch64" ]; then
        TARGET="aarch64-unknown-linux-gnu"
    elif [ "$OS" = "darwin" ] && [ "$ARCH" = "x86_64" ]; then
        TARGET="x86_64-apple-darwin"
    elif [ "$OS" = "darwin" ] && [ "$ARCH" = "aarch64" ]; then
        TARGET="aarch64-apple-darwin"
    fi

    DOWNLOAD_URL=""
    if [ -n "$RELEASE_JSON" ]; then
        # Find asset matching target or os-arch
        ASSET_URL="$(echo "$RELEASE_JSON" | grep -o '"browser_download_url": *"[^"]*"' | cut -d'"' -f4 | grep -E "${TARGET}|${OS}-${ARCH}" | head -n 1 || true)"
        if [ -n "$ASSET_URL" ]; then
            DOWNLOAD_URL="$ASSET_URL"
        fi
    fi

    if [ -z "$DOWNLOAD_URL" ]; then
        # Standard convention: lazyTrino-<tag>-<target>.tar.gz
        DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${TAG}/lazyTrino-${TAG}-${TARGET}.tar.gz"
    fi

    TMP_DIR="$(mktemp -d 2>/dev/null || mktemp -d -t 'lazytrino')"
    cleanup() {
        rm -rf "$TMP_DIR"
    }
    trap cleanup EXIT INT TERM

    info "Downloading lazyTrino from ${DOWNLOAD_URL}..."
    ARCHIVE_FILE="${TMP_DIR}/archive"

    if ! http_download "$DOWNLOAD_URL" "$ARCHIVE_FILE"; then
        # Try alternate direct binary convention: lazyTrino-<os>-<arch>
        ALT_URL="https://github.com/${REPO}/releases/download/${TAG}/lazyTrino-${OS}-${ARCH}"
        info "Retrying with direct binary: ${ALT_URL}..."
        if ! http_download "$ALT_URL" "${TMP_DIR}/lazyTrino"; then
            error "Failed to download binary from GitHub releases."
            error "Please check available assets at: https://github.com/${REPO}/releases"
            exit 1
        fi
    fi

    BINARY_PATH="${TMP_DIR}/lazyTrino"
    if [ -f "$ARCHIVE_FILE" ]; then
        # Check if file is gzip/tar or raw executable
        if file "$ARCHIVE_FILE" 2>/dev/null | grep -qi "gzip"; then
            tar -xzf "$ARCHIVE_FILE" -C "$TMP_DIR"
        elif file "$ARCHIVE_FILE" 2>/dev/null | grep -qi "executable"; then
            mv "$ARCHIVE_FILE" "$BINARY_PATH"
        else
            # Try tar extraction by default
            if ! tar -xzf "$ARCHIVE_FILE" -C "$TMP_DIR" 2>/dev/null; then
                mv "$ARCHIVE_FILE" "$BINARY_PATH"
            fi
        fi
    fi

    # Find the executable in extracted files if not directly at BINARY_PATH
    if [ ! -f "$BINARY_PATH" ]; then
        FOUND="$(find "$TMP_DIR" -type f -name "lazyTrino" -o -name "lazytrino" | head -n 1 || true)"
        if [ -n "$FOUND" ]; then
            BINARY_PATH="$FOUND"
        else
            error "Downloaded archive did not contain 'lazyTrino' executable."
            exit 1
        fi
    fi

    chmod +x "$BINARY_PATH"

    # Select installation directory
    if [ -n "${LAZYTRINO_DIR:-}" ]; then
        INSTALL_DIR="$LAZYTRINO_DIR"
    elif [ "$(id -u)" -eq 0 ]; then
        INSTALL_DIR="/usr/local/bin"
    elif [ -w "/usr/local/bin" ]; then
        INSTALL_DIR="/usr/local/bin"
    else
        INSTALL_DIR="${HOME}/.local/bin"
    fi

    mkdir -p "$INSTALL_DIR"
    TARGET_PATH="${INSTALL_DIR}/lazyTrino"

    info "Installing lazyTrino to ${TARGET_PATH}..."
    if [ -w "$INSTALL_DIR" ]; then
        cp "$BINARY_PATH" "$TARGET_PATH"
        chmod +x "$TARGET_PATH"
    elif command -v sudo >/dev/null 2>&1; then
        info "Prompting for sudo to install to ${INSTALL_DIR}..."
        sudo cp "$BINARY_PATH" "$TARGET_PATH"
        sudo chmod +x "$TARGET_PATH"
    else
        INSTALL_DIR="${HOME}/.local/bin"
        mkdir -p "$INSTALL_DIR"
        TARGET_PATH="${INSTALL_DIR}/lazyTrino"
        info "Falling back to user-local directory: ${TARGET_PATH}"
        cp "$BINARY_PATH" "$TARGET_PATH"
        chmod +x "$TARGET_PATH"
    fi

    printf "\n"
    success "lazyTrino ${BOLD}${TAG}${RESET} has been installed to ${BOLD}${TARGET_PATH}${RESET}!"

    # Check if INSTALL_DIR is in PATH
    case ":${PATH}:" in
    *:"${INSTALL_DIR}":*)
        ;;
    *)
        warn "Note: ${INSTALL_DIR} is not currently in your PATH."
        printf "  Add it to your shell configuration by running:\n\n"
        printf "    ${BOLD}export PATH=\"%s:\$PATH\"${RESET}\n\n" "$INSTALL_DIR"
        ;;
    esac

    printf "${CYAN}➜${RESET} Run ${BOLD}lazyTrino --help${RESET} or ${BOLD}lazyTrino${RESET} to get started.\n\n"
}

main "$@"
