#!/bin/sh
set -e

# Fama installer
# Usage: curl -fsSL https://raw.githubusercontent.com/AkaraChen/fama/master/install.sh | sh

REPO="AkaraChen/fama"
INSTALL_DIR="/usr/local/bin"

main() {
    os=$(uname -s | tr '[:upper:]' '[:lower:]')
    arch=$(uname -m)

    case "$os" in
        linux)  os="unknown-linux-gnu" ;;
        darwin) os="apple-darwin" ;;
        *)
            echo "Unsupported OS: $os"
            echo "Windows users: download from https://github.com/$REPO/releases and add to PATH manually"
            exit 1
            ;;
    esac

    case "$arch" in
        x86_64|amd64)  arch="x86_64" ;;
        arm64|aarch64) arch="aarch64" ;;
        *)
            echo "Unsupported architecture: $arch"
            exit 1
            ;;
    esac

    target="${arch}-${os}"

    echo "Detecting system... $target"

    # Get latest release tag
    tag=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | cut -d'"' -f4)

    if [ -z "$tag" ]; then
        echo "Failed to fetch latest release"
        exit 1
    fi

    echo "Latest version: $tag"

    filename="fama-${tag}-${target}.tar.gz"
    url="https://github.com/$REPO/releases/download/${tag}/${filename}"

    tmpdir=$(mktemp -d)
    trap 'rm -rf "$tmpdir"' EXIT

    echo "Downloading $filename..."
    curl -fsSL "$url" -o "$tmpdir/fama.tar.gz"

    echo "Extracting..."
    tar -xzf "$tmpdir/fama.tar.gz" -C "$tmpdir"

    echo "Installing to $INSTALL_DIR..."
    if [ -w "$INSTALL_DIR" ]; then
        mv "$tmpdir/fama" "$INSTALL_DIR/fama"
    else
        sudo mv "$tmpdir/fama" "$INSTALL_DIR/fama"
    fi

    chmod +x "$INSTALL_DIR/fama"

    echo ""
    echo "Done! fama installed to $INSTALL_DIR/fama"
    echo "Run 'fama --help' to get started"
}

main
