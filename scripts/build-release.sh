#!/bin/bash
# Build release binaries for all platforms
# Requires: cross (cargo install cross)
#
# Usage: ./scripts/build-release.sh
#
# Environment variables for macOS signing:
#   APPLE_DEVELOPER_ID    - Developer ID Application certificate name
#                           e.g., "Developer ID Application: Your Name (TEAMID)"
#   APPLE_ID              - Apple ID email for notarization
#   APPLE_APP_PASSWORD    - App-specific password for notarization
#   APPLE_TEAM_ID         - Your Apple Developer Team ID

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
OUTPUT_DIR="$PROJECT_ROOT/server/public/releases"

# Targets to build
TARGETS=(
    "x86_64-apple-darwin"
    "aarch64-apple-darwin"
    "x86_64-unknown-linux-gnu"
    "aarch64-unknown-linux-gnu"
)

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info() { printf "${BLUE}[INFO]${NC} %s\n" "$1"; }
success() { printf "${GREEN}[SUCCESS]${NC} %s\n" "$1"; }
warn() { printf "${YELLOW}[WARN]${NC} %s\n" "$1"; }
error() { printf "${RED}[ERROR]${NC} %s\n" "$1" >&2; exit 1; }

# Check dependencies
check_deps() {
    if ! command -v cargo &>/dev/null; then
        error "cargo not found. Install Rust first."
    fi

    # Check for cross (needed for cross-compilation)
    if ! command -v cross &>/dev/null; then
        warn "cross not found. Installing..."
        cargo install cross
    fi
}

# Detect current platform
detect_platform() {
    local os arch
    case "$(uname -s)" in
        Darwin) os="apple-darwin" ;;
        Linux) os="unknown-linux-gnu" ;;
        *) error "Unsupported OS" ;;
    esac
    case "$(uname -m)" in
        x86_64) arch="x86_64" ;;
        arm64|aarch64) arch="aarch64" ;;
        *) error "Unsupported architecture" ;;
    esac
    echo "${arch}-${os}"
}

# Sign macOS binary
sign_macos_binary() {
    local binary_path="$1"

    if [ -z "$APPLE_DEVELOPER_ID" ]; then
        warn "APPLE_DEVELOPER_ID not set, skipping code signing"
        return 0
    fi

    info "Signing binary with Developer ID..."
    codesign --force --options runtime --sign "$APPLE_DEVELOPER_ID" "$binary_path"

    # Verify signature
    codesign --verify --verbose "$binary_path"
    success "Binary signed successfully"
}

# Notarize macOS binary
notarize_macos_binary() {
    local binary_path="$1"
    local target="$2"

    if [ -z "$APPLE_ID" ] || [ -z "$APPLE_APP_PASSWORD" ] || [ -z "$APPLE_TEAM_ID" ]; then
        warn "Notarization credentials not set, skipping notarization"
        warn "Set APPLE_ID, APPLE_APP_PASSWORD, and APPLE_TEAM_ID to enable"
        return 0
    fi

    info "Preparing for notarization..."

    local tmp_dir
    tmp_dir=$(mktemp -d)
    local zip_path="$tmp_dir/nosh-${target}.zip"

    # Create zip for notarization
    zip -j "$zip_path" "$binary_path"

    info "Submitting to Apple for notarization..."
    xcrun notarytool submit "$zip_path" \
        --apple-id "$APPLE_ID" \
        --password "$APPLE_APP_PASSWORD" \
        --team-id "$APPLE_TEAM_ID" \
        --wait

    rm -rf "$tmp_dir"
    success "Notarization complete"
}

# Build for a single target
build_target() {
    local target="$1"
    local current_platform="$2"

    info "Building for $target..."

    # Check if this is a macOS target
    local is_macos_target=false
    if [[ "$target" == *"apple-darwin"* ]]; then
        is_macos_target=true
    fi

    # Check if we can build this target
    if [ "$is_macos_target" = true ] && [[ "$(uname -s)" != "Darwin" ]]; then
        warn "Skipping $target (requires macOS to build)"
        return 0
    fi

    if [ "$target" = "$current_platform" ]; then
        # Native build
        cargo build --release --target "$target"
    elif [ "$is_macos_target" = true ]; then
        # macOS cross-compile (Intel <-> ARM on macOS)
        rustup target add "$target" 2>/dev/null || true
        cargo build --release --target "$target"
    else
        # Cross-compile to Linux
        cross build --release --target "$target"
    fi

    local binary_path="$PROJECT_ROOT/target/$target/release/nosh"
    if [ ! -f "$binary_path" ]; then
        error "Binary not found at $binary_path"
    fi

    # Sign and notarize macOS binaries
    if [ "$is_macos_target" = true ]; then
        sign_macos_binary "$binary_path"
        notarize_macos_binary "$binary_path" "$target"
    fi

    # Create archive
    local archive_name="nosh-${target}.tar.gz"
    local archive_path="$OUTPUT_DIR/$archive_name"

    info "Creating $archive_name..."
    mkdir -p "$OUTPUT_DIR"

    # Create tarball with just the binary
    tar -czf "$archive_path" -C "$(dirname "$binary_path")" nosh

    success "Created $archive_path ($(du -h "$archive_path" | cut -f1))"
}

main() {
    info "Building nosh release binaries"
    info "Output: $OUTPUT_DIR"
    echo

    # Check signing setup
    if [ -n "$APPLE_DEVELOPER_ID" ]; then
        info "macOS signing enabled: $APPLE_DEVELOPER_ID"
    else
        warn "macOS signing disabled (set APPLE_DEVELOPER_ID to enable)"
    fi
    echo

    check_deps

    local current_platform
    current_platform=$(detect_platform)
    info "Current platform: $current_platform"
    echo

    # Build for all targets
    for target in "${TARGETS[@]}"; do
        build_target "$target" "$current_platform"
        echo
    done

    success "All builds complete!"
    echo
    info "Release files:"
    ls -lh "$OUTPUT_DIR"/*.tar.gz
    echo
    info "To deploy, commit and push the server/public/releases/ directory"
}

main "$@"
