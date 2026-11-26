#!/usr/bin/env bash
# Reproduce the vendored wasm/swiftformat.wasm.zst from source.
#
# Provenance of the vendored module:
#   host toolchain: Swift 6.3.0 (official Ubuntu 24.04 tarball; the wasm SDK's
#                   swiftmodules require this exact point version)
#   wasm SDK:       swift-wasm-6.3-RELEASE-wasm32-unknown-wasip1.artifactbundle
#                   from https://github.com/swiftwasm/swift/releases
#   swift-format:   tag 603.0.0, built UNMODIFIED (zero patches)
#   guest binding:  wasm-src/ (this crate), a ~150-line C-ABI shim
#
# The full recipe runs in CI (see .github/workflows/release.yml,
# build-swift-wasm job). Steps by hand on an Ubuntu 24.04 box:
#
#   curl -fsSL https://download.swift.org/swift-6.3-release/ubuntu2404/swift-6.3-RELEASE/swift-6.3-RELEASE-ubuntu24.04.tar.gz | sudo tar xz -C /opt
#   export PATH=/opt/swift-6.3-RELEASE-ubuntu24.04/usr/bin:$PATH
#   curl -fsSL -o /tmp/sdk.zip https://github.com/swiftwasm/swift/releases/download/swift-wasm-6.3-RELEASE/swift-wasm-6.3-RELEASE-wasm32-unknown-wasip1.artifactbundle.zip
#   unzip -q /tmp/sdk.zip -d /tmp/sdk
#   swift sdk install /tmp/sdk/swift-wasm-6.3-RELEASE-wasm32-unknown-wasip1.artifactbundle
#   git clone --depth 1 --branch 603.0.0 https://github.com/swiftlang/swift-format formatters/swift/swift-format
#   (cd formatters/swift/wasm-src && bash build-ci.sh)      # -> formatters/swift/swiftformat.wasm
#   wasm-opt -Oz --all-features -o formatters/swift/swiftformat.opt.wasm formatters/swift/swiftformat.wasm
#   mv formatters/swift/swiftformat.opt.wasm formatters/swift/swiftformat.wasm
#   python3 formatters/swift/wasm-src/strip.py formatters/swift/swiftformat.wasm
#   zstd -19 -T0 -o formatters/swift/wasm/swiftformat.wasm.zst formatters/swift/swiftformat.wasm
#
# The import table must contain only wasi_snapshot_preview1.* (check with
# any wasm tool; wasm-src/imports.py ships with the CI workflow): any js_*
# import means a JS bridge leaked into the link graph.
set -euo pipefail
echo "This module is built in CI (release.yml: build-swift-wasm). See header for the manual recipe."
