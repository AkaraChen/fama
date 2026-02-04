#!/bin/bash
# fetch_binary.sh - Download dart_style binaries from GitHub Actions artifacts
# This script downloads artifacts from the latest successful workflow run

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="$SCRIPT_DIR/bin"

# Repository information
REPO_OWNER="AkaraChen"
REPO_NAME="dart_style"
WORKFLOW_NAME="Build%20and%20Upload"

# Create bin directory
mkdir -p "$BIN_DIR"

echo "Fetching dart_style binaries from GitHub Actions..."
echo "Repository: $REPO_OWNER/$REPO_NAME"
echo ""

# Check for GitHub token
if [ -z "$GITHUB_TOKEN" ]; then
    echo "⚠️  Warning: GITHUB_TOKEN not set"
    echo "   You may hit API rate limits. To increase rate limits, set GITHUB_TOKEN:"
    echo "   export GITHUB_TOKEN=your_github_token"
    echo ""
fi

# Function to get the latest successful workflow run ID
get_latest_run_id() {
    local api_url="https://api.github.com/repos/$REPO_OWNER/$REPO_NAME/actions/workflows/$WORKFLOW_NAME.yml/runs?status=success&per_page=1"
    
    if [ -n "$GITHUB_TOKEN" ]; then
        curl -fsSL -H "Authorization: token $GITHUB_TOKEN" "$api_url" | grep -o '"id": [0-9]*' | head -1 | grep -o '[0-9]*'
    else
        curl -fsSL "$api_url" | grep -o '"id": [0-9]*' | head -1 | grep -o '[0-9]*'
    fi
}

# Function to download an artifact
download_artifact() {
    local artifact_name=$1
    local output_name=$2
    local output_path="$BIN_DIR/$output_name"
    
    echo "📦 Downloading $artifact_name..."
    
    # Get artifact download URL
    local artifacts_url="https://api.github.com/repos/$REPO_OWNER/$REPO_NAME/actions/runs/$RUN_ID/artifacts"
    
    if [ -n "$GITHUB_TOKEN" ]; then
        local download_url=$(curl -fsSL -H "Authorization: token $GITHUB_TOKEN" "$artifacts_url" | \
            grep -A 5 "\"name\": \"$artifact_name\"" | \
            grep '"archive_download_url":' | \
            sed 's/.*"archive_download_url": "\([^"]*\)".*/\1/')
    else
        local download_url=$(curl -fsSL "$artifacts_url" | \
            grep -A 5 "\"name\": \"$artifact_name\"" | \
            grep '"archive_download_url":' | \
            sed 's/.*"archive_download_url": "\([^"]*\)".*/\1/')
    fi
    
    if [ -z "$download_url" ]; then
        echo "   ❌ Artifact not found: $artifact_name"
        return 1
    fi
    
    # Download the artifact (it's a zip file)
    local temp_zip=$(mktemp)
    
    if [ -n "$GITHUB_TOKEN" ]; then
        curl -fsSL -H "Authorization: token $GITHUB_TOKEN" -L "$download_url" -o "$temp_zip"
    else
        curl -fsSL -L "$download_url" -o "$temp_zip"
    fi
    
    # Extract the zip file
    local temp_dir=$(mktemp -d)
    unzip -q "$temp_zip" -d "$temp_dir"
    
    # Find the executable (format or format.exe)
    local exe_file=$(find "$temp_dir" -name "format" -o -name "format.exe" | head -1)
    
    if [ -z "$exe_file" ]; then
        echo "   ❌ Executable not found in artifact"
        rm -rf "$temp_zip" "$temp_dir"
        return 1
    fi
    
    # Move to destination
    mv "$exe_file" "$output_path"
    chmod +x "$output_path"
    
    # Cleanup
    rm -rf "$temp_zip" "$temp_dir"
    
    echo "   ✅ Downloaded to $output_path"
    return 0
}

# Main execution

# Get the latest successful run
RUN_ID=$(get_latest_run_id)

if [ -z "$RUN_ID" ]; then
    echo "❌ Error: Could not find a successful workflow run"
    echo "   Please ensure the GitHub Actions workflow 'Build and Upload' has completed successfully"
    exit 1
fi

echo "📋 Latest successful workflow run ID: $RUN_ID"
echo ""

# Download artifacts for each platform
failed_count=0

# Linux x86_64
if ! download_artifact "format-linux-x86_64" "dart_style-linux-x86_64"; then
    failed_count=$((failed_count + 1))
fi

# Linux aarch64
if ! download_artifact "format-linux-aarch64" "dart_style-linux-aarch64"; then
    failed_count=$((failed_count + 1))
fi

# macOS x86_64
if ! download_artifact "format-macos-x86_64" "dart_style-macos-x86_64"; then
    failed_count=$((failed_count + 1))
fi

# macOS aarch64
if ! download_artifact "format-macos-aarch64" "dart_style-macos-aarch64"; then
    failed_count=$((failed_count + 1))
fi

# Windows x86_64 MSVC
if ! download_artifact "format-windows-x86_64-msvc" "dart_style-windows-x86_64-msvc.exe"; then
    failed_count=$((failed_count + 1))
fi

# Windows i686 MSVC
if ! download_artifact "format-windows-i686-msvc" "dart_style-windows-i686-msvc.exe"; then
    failed_count=$((failed_count + 1))
fi

# Windows aarch64 MSVC
if ! download_artifact "format-windows-aarch64-msvc" "dart_style-windows-aarch64-msvc.exe"; then
    failed_count=$((failed_count + 1))
fi

# Windows x86_64 GNU
if ! download_artifact "format-windows-x86_64-gnu" "dart_style-windows-x86_64-gnu.exe"; then
    failed_count=$((failed_count + 1))
fi

echo ""
echo "=========================================="
if [ $failed_count -eq 0 ]; then
    echo "✅ All binaries downloaded successfully!"
else
    echo "⚠️  Downloaded with $failed_count failures"
fi
echo "=========================================="
echo ""
echo "Downloaded files:"
ls -lh "$BIN_DIR/" 2>/dev/null || echo "  (none)"
echo ""

# Instructions for manual download
echo "If some downloads failed, you can manually download artifacts from:"
echo "  https://github.com/$REPO_OWNER/$REPO_NAME/actions/runs/$RUN_ID"
echo ""
echo "Or use nightly.link for unauthenticated downloads:"
echo "  https://nightly.link/$REPO_OWNER/$REPO_NAME/workflows/Build%20and%20Upload/main"
echo ""

exit $failed_count
