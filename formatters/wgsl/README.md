# fama-wgsl

WGSL formatting via the **`wgsl-formatter`** crate from
[wgsl-analyzer](https://github.com/wgsl-analyzer/wgsl-analyzer)
(MIT OR Apache-2.0), used as a native Rust dependency.

## Provenance

- Upstream: <https://github.com/wgsl-analyzer/wgsl-analyzer>
- Pinned revision (both `wgsl-formatter` and `syntax`):
  `963ec16ba47e33a99e613ef6231f6d8dff72ab63` (main, 2026-11-25)
- Neither crate is published to crates.io, so they are git-pinned here the
  same way the biome and ruff dependencies are. Bump both entries in
  `Cargo.toml` to the same revision together.

The formatter core parses WGSL (and wesl) into a rowan syntax tree and
regenerates the source with a dprint-core layout engine, so line width,
comment placement and `// wgslfmt-ignore` pragmas are handled by the same
code that powers wgsl-analyzer's editor formatting. Inputs with parse
diagnostics are rejected instead of being reformatted lossily.

## Why not the wgslfmt WASM build?

The task brief suggested embedding
[wasm-fmt/wgslfmt](https://github.com/wasm-fmt/wgslfmt) as WASM (like
`formatters/clang`). That turned out to be strictly worse than this native
path:

- wgslfmt is only a thin `wasm-bindgen` wrapper (its `src/lib.rs` is a
  single call to `wgsl_formatter::format_str`). Its shipped
  `wgslfmt_bg.wasm` imports 20 JS host functions (`__wbg_*` from a
  `./wgslfmt_bg.js` glue module) and receives its config as a JS object
  deserialized through `serde-wasm-bindgen`, so a wasmtime host would need
  to emulate a JS heap — the same pain the dart2wasm host needed a custom
  shim layer for, but with no payoff.
- wgslfmt is effectively frozen (4 commits; it still wraps the *pre-rewrite*
  wgsl-analyzer formatter from commit `466476b999ccfc4035371fe5a74f2d878caea9e1`,
  whose trailing-comma handling was commented out and which had no
  line-width support or error reporting). Upstream replaced that formatter
  with the dprint-based engine vendored here.
- The native crate removes the WASM compile cost, the precompiled-artifact
  machinery and the per-thread instance reuse entirely.

## Benchmarks

`cargo test -p fama-wgsl --release -- --ignored --nocapture` prints µs/file
for a ~50-line and a ~1000-line shader.
