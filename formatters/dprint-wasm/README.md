# fama-dprint-wasm

Compatibility runtime for hosting **dprint WASM Plugin Schema v4** modules in-process on `wasmtime` (`fama-wasm`).

## Architecture

- `host.rs`: Implements dprint Schema v4 guest lifecycle (`dprint_plugin_version_4`, `_initialize`, `register_config`, `set_file_path`, `format`, `get_formatted_text`, `get_error_text`, `get_plugin_info`), `dprint::host_*` callbacks (delegating embedded formatting back to `fama_common::format_embedded`), `.wasmu` precompilation caching, and a bounded per-plugin `InstancePool`.
- `wasi.rs`: Hermetic `wasi_snapshot_preview1` host (zero filesystem mutation, automatic stubbing of extra WASI imports).
- `lib.rs`: `DprintWasmFormatter` implementing `fama_common::Formatter` for raw `.wasm`, zstd-compressed `.wasm.zst`, or disk-loaded plugin paths.

## Vendored Plugins

| Plugin | Extensions | Compressed Size | Notes |
| :--- | :--- | ---: | :--- |
| `dprint-plugin-csharpier` (`0.3.1`) | `.cs`, `.csx` | 3.40 MB | Idiomatic C# formatter replacing `clang-format` |
| `dprint-plugin-roslyn` (`0.24.1`) | `.vb` | 3.55 MB | Official Roslyn-based VB.NET formatter |
| `dprint-plugin-zig` (`0.1.2`) | `.zig`, `.zon` | 76 KB | Official Zig AST formatter replacing `zigffi` (zero Zig toolchain needed) |
