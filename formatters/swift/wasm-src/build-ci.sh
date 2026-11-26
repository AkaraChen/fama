#!/usr/bin/env bash
# Build swiftformat_wasm for wasm32-unknown-wasip1 with the SwiftWasm SDK.
# Runs on a plain ubuntu-24.04 runner (no Docker): the swift.org tarball is
# built for exactly this distro. Layout expected (paths relative to this
# crate's root, formatters/swift/):
#   wasm-src/         this package
#   swift-format/     sibling checkout at tag 603.0.0
#   $PATH             contains the swift-6.3.0 toolchain's usr/bin
# Output: ../swiftformat.wasm (i.e. the crate root), to be optimized with
# wasm-opt, stripped with strip.py, and zstd'd into wasm/swiftformat.wasm.zst.
set -ux
export HOME="${HOME:-/home/runner}"

cd "$(dirname "$0")"

swift --version

SDK_ID=$(swift sdk list 2>/dev/null | grep -m1 wasip1 | awk '{print $1}')
SDK_ID=${SDK_ID:-wasm32-unknown-wasip1}
echo "USING SDK_ID: $SDK_ID"

# Detect the exact icudt*_dat symbol name(s) exported by the installed SwiftWasm
# SDK so Sources/ICUStub/icudt_stub.c overrides them regardless of ICU version
# or swift-foundation-icu symbol prefixing.
NM_BIN=$(command -v llvm-nm || command -v nm || true)
SDK_ROOT=$(find "$HOME/.swiftpm/swift-sdks" /root/.swiftpm/swift-sdks -maxdepth 3 -type d -name '*wasip1*' 2>/dev/null | head -1 || true)
if [ -n "$NM_BIN" ] && [ -n "$SDK_ROOT" ]; then
  ICUDT_SYMS=$(
    find "$SDK_ROOT" -name '*.a' -print0 2>/dev/null \
      | xargs -0 -r "$NM_BIN" --defined-only 2>/dev/null \
      | awk 'NF >= 2 && $(NF - 1) ~ /^[DRBSdrbs]$/ && $NF ~ /^_?icudt.*_dat$/ { sub(/^_/, "", $NF); print $NF }' \
      | sort -u
  )
  if [ -n "$ICUDT_SYMS" ]; then
    echo "DETECTED SDK ICU DATA SYMBOLS:"
    echo "$ICUDT_SYMS"
    python3 - "$ICUDT_SYMS" <<'PY'
import pathlib, sys

syms = [s.strip() for s in sys.argv[1].splitlines() if s.strip()]
path = pathlib.Path("Sources/ICUStub/icudt_stub.c")
src = path.read_text()
marker = "DEFINE_ICUDT_STUB("
prefix = src[: src.index(marker)]
defs = "\n".join(f"DEFINE_ICUDT_STUB({s});" for s in syms) + "\n"
path.write_text(prefix + defs)
PY
  else
    echo "WARNING: could not detect icudt*_dat symbol in $SDK_ROOT; using default icudt76_dat stub"
  fi
fi

for i in 1 2 3 4 5; do
  swift package resolve && break
  echo "resolve attempt $i failed"; sleep 15
done
test -d ../swift-format/.git || { echo "MISSING ../swift-format"; exit 1; }

rc=1
for i in 1 2 3; do
  swift build -c release \
    --swift-sdk "$SDK_ID" \
    --scratch-path .build-wasm \
    -Xswiftc -Osize \
    -Xlinker --gc-sections \
    -Xlinker --strip-debug \
    2>&1 | tee build-wasm.log
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
  if grep -a -q "icudt76l/af.res" ../swiftformat.wasm; then
    echo "WARNING: full icudt76l archive is still present in ../swiftformat.wasm"
  else
    echo "ICU_STUB_OK: full icudt archive eliminated from ../swiftformat.wasm"
  fi
  echo "BUILD_OK"
else
  echo "BUILD_FAILED"
  tail -120 build-wasm.log
  exit 1
fi
