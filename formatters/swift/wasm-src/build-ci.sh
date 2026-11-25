#!/usr/bin/env bash
# Build swiftformat_wasm for wasm32-unknown-wasip1 with the SwiftWasm SDK.
# Runs on a plain ubuntu-24.04 runner (no Docker): the swift.org tarball is
# built for exactly this distro. Layout expected (paths relative to this
# crate's root, formatters/swift/):
#   wasm-src/         this package
#   swift-format/     sibling checkout at tag 603.0.0
#   $PATH             contains the swift-6.3.0 toolchain's usr/bin
# Output: ../swiftformat.wasm (i.e. the crate root), to be stripped with
# strip.py and zstd'd into wasm/swiftformat.wasm.zst.
set -ux
export HOME="${HOME:-/home/runner}"

cd "$(dirname "$0")"

swift --version

SDK_ID=$(swift sdk list 2>/dev/null | grep -m1 wasip1 | awk '{print $1}')
SDK_ID=${SDK_ID:-wasm32-unknown-wasip1}
echo "USING SDK_ID: $SDK_ID"

for i in 1 2 3 4 5; do
  swift package resolve && break
  echo "resolve attempt $i failed"; sleep 15
done
test -d ../swift-format/.git || { echo "MISSING ../swift-format"; exit 1; }

rc=1
for i in 1 2 3; do
  swift build -c release --swift-sdk "$SDK_ID" --scratch-path .build-wasm 2>&1 | tee build-wasm.log
  rc=${PIPESTATUS[0]}
  [ "$rc" = 0 ] && break
  echo "build attempt $i failed rc=$rc"; sleep 5
done

if [ "$rc" = 0 ]; then
  find .build-wasm -name '*.wasm' | head -10
  BIN=$(find .build-wasm -name 'swiftformat_wasm.wasm' \( -type f -o -type l \) | head -1)
  if [ -z "$BIN" ]; then echo "BUILD_OK BUT NO BINARY FOUND"; exit 1; fi
  cp "$BIN" ../swiftformat.wasm
  ls -la ../swiftformat.wasm
  echo "BUILD_OK"
else
  echo "BUILD_FAILED"
  tail -120 build-wasm.log
  exit 1
fi
