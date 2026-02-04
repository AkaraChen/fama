#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CLANG_FORMAT_DIR="$SCRIPT_DIR/clang-format"
OUTPUT_DIR="$SCRIPT_DIR/wasm"

echo "Building clang-format WASM..."

# Check if emsdk is available
if ! command -v emcmake &> /dev/null; then
    echo "Error: emcmake not found. Please install and activate emsdk first."
    echo "  git clone https://github.com/emscripten-core/emsdk.git"
    echo "  cd emsdk && ./emsdk install latest && ./emsdk activate latest"
    echo "  source emsdk_env.sh"
    exit 1
fi

# Create build directory
BUILD_DIR="$CLANG_FORMAT_DIR/build"
mkdir -p "$BUILD_DIR"

# Configure with CMake
cd "$BUILD_DIR"
emcmake cmake .. \
    -DCMAKE_BUILD_TYPE=Release \
    -DLLVM_TARGETS_TO_BUILD="" \
    -DLLVM_ENABLE_PROJECTS="clang"

# Build the standalone WASM target
emmake make clang-format-standalone -j$(nproc 2>/dev/null || sysctl -n hw.ncpu)

# Copy the output
mkdir -p "$OUTPUT_DIR"
cp "$BUILD_DIR/clang-format-standalone.wasm" "$OUTPUT_DIR/clang-format.wasm"

echo "Done! WASM binary copied to $OUTPUT_DIR/clang-format.wasm"
ls -lh "$OUTPUT_DIR/clang-format.wasm"
