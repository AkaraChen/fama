# fama-swift

swift-format WASM-based formatter for Swift, hosted in-process on wasmtime.

The formatter is Apple's real [swift-format](https://github.com/swiftlang/swift-format)
(603.0.0, zero patches) cross-compiled to `wasm32-unknown-wasip1` with the
official SwiftWasm 6.3 SDK, wrapped by a ~150-line C-ABI shim
([wasm-src/](wasm-src/)). The guest imports only `wasi_snapshot_preview1`
(34 functions, zero JavaScript bridges); the Rust side hand-rolls that import
surface ([src/wasi.rs](src/wasi.rs)). Formatting touches no filesystem at all:
the shim hands the formatter a `nil` file URL, so the WASI filesystem calls
exist only to satisfy wasi-libc startup.

## Size and startup cost

The stripped module is ~62MB (~15MB zstd-compressed, embedded in the binary).
Cranelift-compiling it takes ~5s, so releases ship per-platform precompiled
artifacts (`swiftformat-<hash>.wasmu`, deserialized in ~70ms). Without one,
the first run JITs once and caches the artifact to `~/.cache/fama`.

## Rebuilding the WASM Binary

The module is checked in at `wasm/swiftformat.wasm.zst`. To rebuild it
(usually just by dispatching the `SwiftWasm build` workflow):

1. Ubuntu 24.04 with the official **Swift 6.3.0** tarball — the SDK's
   precompiled swiftmodules reject every other point release
2. The **swift-wasm-6.3-RELEASE** wasip1 artifactbundle from
   <https://github.com/swiftwasm/swift/releases>, installed via `swift sdk install`
3. A checkout of **swift-format 603.0.0** at `formatters/swift/swift-format`
   (a sibling of `wasm-src/`, which references it as a path dependency)

```bash
(cd wasm-src && bash build-ci.sh)                        # -> ../swiftformat.wasm
python3 wasm-src/strip.py ../swiftformat.wasm            # drop debug/name sections
zstd -19 -T0 -o wasm/swiftformat.wasm.zst ../swiftformat.wasm
```

The import table must stay WASI-only: `python3 wasm-src/imports.py
../swiftformat.wasm` must print `JS_IMPORTS: NONE`. A `js_*` import means a
JavaScript bridge leaked into the link graph (e.g. by pulling in
JavaScriptKit) and the module can no longer run under a plain WASI host.
