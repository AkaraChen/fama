#!/usr/bin/env bash
# Rebuild the embedded dart_style WASM module (wasm/dart_fmt.wasm).
#
# The module is dart_style compiled to WebAssembly with dart2wasm, hosted
# in-process by wasmtime. It only needs rebuilding when dart_style (or the
# wasm-fmt/dart_fmt binding) is updated.
#
# Requirements:
#   - Dart SDK >= 3.14 (dart2wasm emits standard `try_table` exception
#     handling there; older SDKs emit legacy-exceptions bytecode that
#     wasmtime refuses to compile)
#   - git and network access
#
# Usage: ./build-wasm.sh [output.wasm]
set -euo pipefail

DART_BIN="${DART_BIN:-dart}"
OUT="${1:-$(dirname "$0")/wasm/dart_fmt.wasm}"
SRC_DIR="${DART_FMT_SRC:-$PWD/target/dart_fmt-src}"

git clone https://github.com/wasm-fmt/dart_fmt.git "$SRC_DIR" ||
	(cd "$SRC_DIR" && git pull)

cd "$SRC_DIR"
"$DART_BIN" pub get
"$DART_BIN" compile wasm ./lib/binding.dart -o "$OUT"

echo "Rebuilt $(du -h "$OUT" | cut -f1) module at $OUT"
