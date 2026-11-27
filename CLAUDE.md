# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Fama is a unified cross-language code formatter written in Rust that aggregates multiple specialized formatters into a single CLI tool. It formats 60+ languages (JavaScript, TypeScript, JSX, TSX, JSON, JSONC, JSON5, CSS, SCSS, Less, Sass, HTML, Vue, Svelte, Astro, GraphQL, Jinja2, Twig, Nunjucks, Mustache, Handlebars, Vento, YAML, TOML, KDL, Markdown, MDX, Jupyter `.ipynb`, LaTeX, BibTeX, Typst, Rust, Python, Lua, Ruby, PHP, Shell, Fish, Nushell, Go, Zig, HCL, Starlark/Bazel, CUE, Jsonnet, Rego, Dockerfile, SQL, XML, Kotlin, Dart, Scala, Nix, Nickel, OpenSCAD, CMake, Meson, Just, R, Elixir, Crystal, QML, Mojo, Bicep, V, Swift, WGSL, C, C++, C#, VB.NET, Objective-C, Java, Protobuf, CUDA, HLSL, Metal, GLSL, SystemVerilog/Verilog, GDScript, Solidity, Gleam, Prisma, Cairo, Move) through a unified interface while respecting a centralized configuration.

## Build Commands

```bash
make build      # Debug build
make release    # Optimized release build
make test       # Run all tests (cargo test)
make dev        # Debug build
make install    # Install to /usr/local/bin (requires sudo)
make clean      # Remove build artifacts
```

## CLI Usage

```bash
fama [PATTERN]   # Format files matching glob pattern (default: **/*)
fama --export    # Generate .editorconfig plus tool configs (rustfmt.toml,
                 # analysis_options.yaml, .swift-format) when their markers exist
```

## Architecture

### Workspace Structure

The project is a Cargo workspace with 14 crates:

- `cli/` - Main CLI application with file discovery, formatter `Registry` (with fail-closed Linguist content-based extension disambiguation), routing, and `--export` / `--precompile`
- `common/` - Shared types: `Formatter` trait (including `linguist_languages` & `needs_content_check`), `format_embedded` host callback, `ExportedConfig`, `FormatConfig`, indentation/quote styles, and `.editorconfig` renderer
- `formatters/` - Language-specific formatter implementations:
  - `builtin/` (`fama-builtin`) - Native pure-Rust formatters (1 module each): TOML (Taplo), Python (Ruff), Lua (StyLua), Dockerfile, XML (`quick-xml`), SQL (`sqruff`), PHP (Mago), CMake (`cmakefmt-rust`), Nix (Alejandra), WGSL (`wgsl-formatter`), Typst (`typstyle-core`), KDL (`kdl`), GDScript (`gdck-format`), LaTeX/BibTeX (`tex-fmt`)
  - `web/` (`fama-web`) - Web, style, markup, data, and document-container formatters: Biome (JS/TS/JSX/TSX/JSON/JSONC/HTML/Vue/Svelte/Astro/GraphQL), dprint/Malva (Markdown/YAML/CSS/SCSS/Less/Sass), native MDX (`.mdx` via `markdown-rs` + Biome + `dprint-plugin-markdown`), `dprint-plugin-json` (JSON5), `dprint-plugin-jupyter` (`.ipynb` via `format_embedded`), and `markup_fmt` (Jinja2/Twig/Nunjucks/Mustache/Handlebars/Vento)
  - `process/` (`fama-process`) - External CLI ("process mode") wrappers sharing `process.rs`: batched wrappers (`rustfmt`, `scalafmt`, `air`, `mix format`) and single-file wrappers (`forge` Solidity, `gleam`, `prisma-fmt`, `just`, `cairo-format`, `movefmt`, `nufmt`, `fish_indent`, `meson format`, `crystal tool format`, `qmlformat`, `mojo format`, `bicep format`, `v fmt`)
  - `dprint-wasm/` (`fama-dprint-wasm`) - Compatibility runtime for dprint Schema v4 WASM plugins on `wasmtime` (`fama-wasm`), hosting `dprint-plugin-csharpier` (C#), `dprint-plugin-roslyn` (VB.NET), and `dprint-plugin-zig` (Zig / `.zon`)
  - `topiary/` (`fama-topiary`) - Declarative Tree-sitter formatters powered by `topiary-core` (Nickel, OpenSCAD; see `formatters/topiary/README.md`)
  - `wasm-runtime/` (`fama-wasm`) - Shared wasmtime engine configuration, zstd decompression (`ruzstd`), `.wasmu` artifact caching, and precompilation helpers
  - `clang/` - C/C++/Objective-C/Java/Protobuf/CUDA/HLSL/Metal/GLSL/SystemVerilog/Verilog (via zstd-compressed clang-format WASM)
  - `dart/` - Dart via dart_style compiled to WASM (zstd-compressed dart2wasm output hosted in-process on wasmtime)
  - `swift/` - Swift via swift-format compiled to WASM (SwiftWasm wasip1 build of swift-format 603.0.0 plus a C-ABI shim, hosted in-process on wasmtime with a hand-rolled WASI p1 host; see formatters/swift/README.md)
  - `goffi/` - Shell (`mvdan/sh`), Go (`go/format`), HCL/Terraform (`hclwrite`), Starlark/Bazel (`buildifier`), CUE (`cue/format`), Jsonnet (`go-jsonnet`), and Rego (`opa/v1/format`) via Go FFI
  - `ruby/` - Ruby (via `rubyfmt` submodule)
  - `kotlin-worker/` - Kotlin via the persistent `ktfmt-worker` protocol (with built-in process fallback via `ktfmt`)

### Data Flow

1. **Discovery** (`cli/src/discovery.rs`): Walk filesystem, filter via `cli/src/registry.rs`, respect `.gitignore`
2. **Resolution** (`cli/src/registry.rs`): Map extension or special filename → `&'static dyn Formatter`. Unambiguous extensions take the zero-overhead `ResolveOutcome::Unique` fast path without touching `linguist`. Shared or content-guarded extensions (`ResolveOutcome::NeedsContent`, e.g. `.v`, `.m`, `.cls`) read a 4KB prefix and run pure-Rust `linguist::detect_language_by_modeline` → `linguist::disambiguate`, matching against each candidate's `Formatter::linguist_languages(ext)` with a fail-closed policy (no match or unclaimed language → skip file).
3. **Routing** (`cli/src/formatter.rs`): Run batchable formatters (`supports_batch()`) in chunks concurrently with LPT-ordered single-file formatters
4. **Formatting**: Each `Formatter::format` / `Formatter::format_batch` receives content `&str` and returns formatted `String`
5. **Write-back**: If changed, write to disk; track stats (formatted, unchanged, errors)

### Formatter Interface

All formatters implement `fama_common::Formatter` (`Send + Sync`):

```rust
pub trait Formatter: Sync {
	fn name(&self) -> &'static str;
	fn extensions(&self) -> &'static [&'static str];
	fn matches_filename(&self, _name: &str) -> bool { false }
	fn linguist_languages(&self, _ext: &str) -> &'static [&'static str] { &[] }
	fn needs_content_check(&self, ext: &str) -> bool { !self.linguist_languages(ext).is_empty() }
	fn format(&self, content: &str, path: &Path) -> Result<String, String>;
	fn supports_batch(&self) -> bool { false }
	fn format_batch(&self, contents: &[&str]) -> Vec<Result<String, String>>;
	fn exported_config(&self) -> Option<ExportedConfig> { None }
	fn precompile(&self, _output_dir: &Path) -> Option<Result<PathBuf, String>> { None }
}
```

### Configuration

Centralized `FormatConfig` in `common/src/lib.rs` with go-fmt style defaults:

- Tabs for indentation (width: 4)
- 80 character line width
- LF line endings
- Double quotes, trailing commas, semicolons always

### Go FFI (goffi)

The `goffi` crate provides both Shell and Go formatting via CGO:

- Go source in `formatters/goffi/go/`
- Wraps `mvdan.cc/sh/v3/syntax` for shell formatting
- Wraps `go/format` for Go formatting
- Compiled as static library (`libgoffi.a`) and linked into the binary
- Rust FFI bindings in `formatters/goffi/src/lib.rs`
- `build.rs` handles Go compilation and library linking
- Pre-compiled libraries are checked in for supported platforms

## Testing

```bash
cargo test                           # All workspace tests
cargo test -p fama-common            # Test specific crate
cargo test -p fama-cli               # Test CLI crate
cargo test <test_name>               # Run a single test
cargo test -p fama-common <test_name> # Run single test in specific crate
```

## Adding a New Formatter

Choose the crate matching the formatter's dependency model:
1. Pure-Rust language/config formatter → `formatters/builtin/` (`src/<lang>.rs` + `FORMATTERS` in `src/lib.rs`)
2. Web/style/markup/document formatter → `formatters/web/` (`src/<lang>.rs` + `FORMATTERS` in `src/lib.rs`)
3. External CLI ("process mode") wrapper → `formatters/process/` (`src/<lang>.rs` reusing `process.rs` + `FORMATTERS` in `src/lib.rs`)
4. Official dprint Schema v4 `.wasm` plugin → compress with `zstd -19` into `formatters/dprint-wasm/wasm/<name>.wasm.zst` and register a `DprintWasmFormatter` static in `formatters/dprint-wasm/src/lib.rs`
5. Custom FFI, custom WASM runtime, Tree-sitter query (`topiary`), or sidecar → create a crate under `formatters/<lang>/`, add it to `Cargo.toml` and `cli/Cargo.toml`, and register it in `cli/src/registry.rs`

## Key Dependencies

- **Biome**: Git-pinned for HTML support (specific commit)
- **Ruff**: Git-pinned formatters from Astral's ruff repo
- **dprint + Malva**: Published crates for data/style formats
- **swift-format**: Apple's formatter compiled to wasm32-unknown-wasip1 (SwiftWasm 6.3 SDK) and hosted on wasmtime, like clang-format and dart_style
- **mvdan/sh**: Go library for shell formatting via FFI
- **stylua**: Lua formatter
- **linguist**: Pure-Rust GitHub Linguist heuristics/modelines (only in `cli/`; lazily initialized on `ResolveOutcome::NeedsContent` for ambiguous extensions)
