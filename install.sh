#!/bin/sh
# nosh installer
# Usage: curl -fsSL https://noshell.dev/install.sh | sh
#
# Installs the latest version of nosh to ~/.local/bin (or /usr/local/bin with sudo)

set -e

BASE_URL="https://noshell.dev"
INSTALL_DIR="${NOSH_INSTALL_DIR:-$HOME/.local/bin}"

# Colors (disabled if not a terminal)
if [ -t 1 ]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    BLUE='\033[0;34m'
    BOLD='\033[1m'
    NC='\033[0m'
else
    RED=''
    GREEN=''
    YELLOW=''
    BLUE=''
    BOLD=''
    NC=''
fi

info() {
    printf "${BLUE}info${NC}: %s\n" "$1"
}

success() {
    printf "${GREEN}success${NC}: %s\n" "$1"
}

warn() {
    printf "${YELLOW}warn${NC}: %s\n" "$1"
}

error() {
    printf "${RED}error${NC}: %s\n" "$1" >&2
    exit 1
}

# Detect OS
detect_os() {
    case "$(uname -s)" in
        Linux*)  echo "linux" ;;
        Darwin*) echo "macos" ;;
        MINGW*|MSYS*|CYGWIN*) echo "windows" ;;
        *) error "Unsupported operating system: $(uname -s)" ;;
    esac
}

# Detect architecture
detect_arch() {
    case "$(uname -m)" in
        x86_64|amd64) echo "x86_64" ;;
        aarch64|arm64) echo "aarch64" ;;
        *) error "Unsupported architecture: $(uname -m)" ;;
    esac
}

# Check for required commands
check_deps() {
    for cmd in curl tar; do
        if ! command -v "$cmd" >/dev/null 2>&1; then
            error "Required command not found: $cmd"
        fi
    done
}

# Main installation
main() {
    printf "\n"
    printf "${BOLD}  nosh installer${NC}\n"
    printf "  The shell that understands you\n"
    printf "\n"

    check_deps

    OS=$(detect_os)
    ARCH=$(detect_arch)

    info "Detected OS: $OS, Architecture: $ARCH"

    # Construct download URL
    # Format: nosh-{arch}-{os}.tar.gz
    case "$OS" in
        macos)
            TARGET="${ARCH}-apple-darwin"
            ;;
        linux)
            TARGET="${ARCH}-unknown-linux-gnu"
            ;;
        windows)
            error "Windows is not yet supported. Please use WSL."
            ;;
    esac

    ARCHIVE_NAME="nosh-${TARGET}.tar.gz"
    DOWNLOAD_URL="${BASE_URL}/releases/${ARCHIVE_NAME}"

    # Create temp directory
    TMP_DIR=$(mktemp -d)
    trap 'rm -rf "$TMP_DIR"' EXIT

    info "Downloading from $DOWNLOAD_URL..."
    if ! curl -fsSL "$DOWNLOAD_URL" -o "$TMP_DIR/nosh.tar.gz"; then
        error "Failed to download nosh. The release might not exist for your platform."
    fi

    info "Extracting..."
    tar -xzf "$TMP_DIR/nosh.tar.gz" -C "$TMP_DIR"

    # Find the binary
    BINARY=$(find "$TMP_DIR" -name "nosh" -type f -perm -u+x 2>/dev/null | head -1)
    if [ -z "$BINARY" ]; then
        # Try without exec permission check (macOS sometimes strips it)
        BINARY=$(find "$TMP_DIR" -name "nosh" -type f 2>/dev/null | head -1)
    fi

    if [ -z "$BINARY" ]; then
        error "Could not find nosh binary in archive"
    fi

    # Create install directory if needed
    if [ ! -d "$INSTALL_DIR" ]; then
        info "Creating $INSTALL_DIR..."
        mkdir -p "$INSTALL_DIR"
    fi

    # Check if we can write to install dir
    if [ ! -w "$INSTALL_DIR" ]; then
        warn "Cannot write to $INSTALL_DIR, trying with sudo..."
        sudo mv "$BINARY" "$INSTALL_DIR/nosh"
        sudo chmod +x "$INSTALL_DIR/nosh"
    else
        mv "$BINARY" "$INSTALL_DIR/nosh"
        chmod +x "$INSTALL_DIR/nosh"
    fi

    success "Installed nosh to $INSTALL_DIR/nosh"

    # Check if install dir is in PATH
    case ":$PATH:" in
        *":$INSTALL_DIR:"*)
            ;;
        *)
            printf "\n"
            warn "$INSTALL_DIR is not in your PATH"
            printf "\n"
            printf "Add this to your shell config (~/.bashrc, ~/.zshrc, etc.):\n"
            printf "\n"
            printf "    ${BOLD}export PATH=\"\$HOME/.local/bin:\$PATH\"${NC}\n"
            printf "\n"
            ;;
    esac

    # Verify installation
    if command -v nosh >/dev/null 2>&1 || [ -x "$INSTALL_DIR/nosh" ]; then
        printf "\n"
        success "Installation complete!"
        printf "\n"
        printf "To get started, run:\n"
        printf "\n"
        printf "    ${BOLD}nosh${NC}\n"
        printf "\n"
        printf "To make nosh your default shell:\n"
        printf "\n"
        printf "    ${BOLD}chsh -s \$(which nosh)${NC}\n"
        printf "\n"
        printf "Documentation: ${BLUE}https://noshell.dev/docs${NC}\n"
        printf "\n"
    fi
}

main "$@"
