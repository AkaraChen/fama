#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUTPUT_DIR="$SCRIPT_DIR/wasm"
REPO="AkaraChen/fama"

echo "Fetching latest clang-format WASM from GitHub..."

# Check if gh CLI is available
if ! command -v gh &> /dev/null; then
    echo "Error: gh CLI not found. Please install GitHub CLI first."
    echo "  brew install gh"
    echo "  # or see https://cli.github.com/"
    exit 1
fi

# Check if authenticated
if ! gh auth status &> /dev/null; then
    echo "Error: Not authenticated with GitHub CLI."
    echo "  Run: gh auth login"
    exit 1
fi

# Try to download from the latest workflow run artifact
echo "Looking for latest successful workflow run..."

RUN_ID=$(gh run list --repo "$REPO" --workflow "build-clang-wasm.yml" --status success --limit 1 --json databaseId --jq '.[0].databaseId')

if [ -z "$RUN_ID" ] || [ "$RUN_ID" = "null" ]; then
    echo "No successful workflow runs found."
    echo ""
    echo "You can trigger a build manually:"
    echo "  gh workflow run build-clang-wasm.yml --repo $REPO"
    echo ""
    echo "Or download from a release:"
    echo "  gh release download --repo $REPO --pattern 'clang-format.wasm' --dir $OUTPUT_DIR"
    exit 1
fi

echo "Found workflow run: $RUN_ID"
echo "Downloading artifact..."

# Create temp directory for download
TEMP_DIR=$(mktemp -d)
trap "rm -rf $TEMP_DIR" EXIT

# Download the artifact
gh run download "$RUN_ID" --repo "$REPO" --name "clang-format-wasm" --dir "$TEMP_DIR"

# Move to output directory
mkdir -p "$OUTPUT_DIR"
mv "$TEMP_DIR/clang-format.wasm" "$OUTPUT_DIR/clang-format.wasm"

echo "Done! WASM binary saved to $OUTPUT_DIR/clang-format.wasm"
ls -lh "$OUTPUT_DIR/clang-format.wasm"
