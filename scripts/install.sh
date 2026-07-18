#!/usr/bin/env bash
# Tokenwise installer — macOS and Linux
# Usage: curl -fsSL https://raw.githubusercontent.com/somarimapps/tokenwise/main/scripts/install.sh | bash
# Or:   bash install.sh [version]
set -euo pipefail

VERSION="${1:-latest}"
REPO="somarimapps/tokenwise"
INSTALL_DIR="/usr/local/bin"
BINARY_NAME="tokenwise"

# Detect OS
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
case "$OS" in
    linux*)  OS="linux" ;;
    darwin*) OS="macos" ;;
    *)
        echo "Unsupported OS: $OS" >&2
        exit 1
        ;;
esac

# Detect architecture
ARCH="$(uname -m)"
case "$ARCH" in
    x86_64)          ARCH="x64" ;;
    aarch64 | arm64) ARCH="arm64" ;;
    *)
        echo "Unsupported architecture: $ARCH" >&2
        exit 1
        ;;
esac

ARTIFACT="${BINARY_NAME}-${OS}-${ARCH}"

# Resolve latest version tag from GitHub API if not pinned
if [ "$VERSION" = "latest" ]; then
    echo "Resolving latest version from GitHub..."
    VERSION="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' \
        | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/')"
    if [ -z "$VERSION" ]; then
        echo "Could not determine latest version. Pass a version explicitly: bash install.sh v0.1.0" >&2
        exit 1
    fi
fi

URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARTIFACT}"

echo "Downloading tokenwise ${VERSION} for ${OS}-${ARCH}..."
TMP="$(mktemp)"
if ! curl -fsSL "$URL" -o "$TMP"; then
    echo "Download failed: $URL" >&2
    rm -f "$TMP"
    exit 1
fi

chmod +x "$TMP"

# Install — try without sudo first; fall back to sudo
if [ -w "$INSTALL_DIR" ]; then
    mv "$TMP" "${INSTALL_DIR}/${BINARY_NAME}"
else
    echo "Installing to ${INSTALL_DIR} (requires sudo)..."
    sudo mv "$TMP" "${INSTALL_DIR}/${BINARY_NAME}"
fi

echo "Tokenwise ${VERSION} installed at ${INSTALL_DIR}/${BINARY_NAME}"

# Run initial stack setup
if command -v tokenwise >/dev/null 2>&1; then
    echo "Running: tokenwise install"
    tokenwise install
else
    echo "Warning: tokenwise not found in PATH. Add ${INSTALL_DIR} to your PATH and run 'tokenwise install'."
fi
