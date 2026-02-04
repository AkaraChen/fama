set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CLANG_FORMAT_DIR="$SCRIPT_DIR/clang-format"
OUTPUT_DIR="$SCRIPT_DIR/wasm"

echo "Building clang-format WASM..."

if ! command -v emcmake &>/dev/null; then
	echo "Error: emcmake not found. Please install and activate emsdk first."
	echo "  git clone https://github.com/emscripten-core/emsdk.git ~/emsdk"
	echo "  cd ~/emsdk && ./emsdk install 4.0.23 && ./emsdk activate 4.0.23"
	echo "  source ~/emsdk/emsdk_env.sh"
	exit 1
fi

if ! command -v ninja &>/dev/null; then
	echo "Error: ninja not found. Please install ninja first."
	echo "  brew install ninja"
	exit 1
fi

export CC=/usr/bin/clang
export CXX=/usr/bin/clang++

BUILD_DIR="$CLANG_FORMAT_DIR/build"
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR"

cd "$BUILD_DIR"
emcmake cmake -G Ninja .. \
	-DCMAKE_BUILD_TYPE=Release \
	-DLLVM_TARGETS_TO_BUILD="" \
	-DLLVM_ENABLE_PROJECTS="clang"

ninja clang-format-standalone

mkdir -p "$OUTPUT_DIR"
cp "$BUILD_DIR/clang-format-standalone.wasm" "$OUTPUT_DIR/clang-format.wasm"

echo "Done! WASM binary copied to $OUTPUT_DIR/clang-format.wasm"
ls -lh "$OUTPUT_DIR/clang-format.wasm"
