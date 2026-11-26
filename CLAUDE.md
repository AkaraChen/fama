# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Fama is a unified cross-language code formatter written in Rust that aggregates multiple specialized formatters into a single CLI tool. It formats 30+ languages (JavaScript, TypeScript, JSX, TSX, JSON, JSONC, CSS, SCSS, Less, Sass, HTML, Vue, Svelte, Astro, GraphQL, YAML, TOML, Markdown, Rust, Python, Lua, Ruby, PHP, Shell, Go, Zig, HCL, Dockerfile, SQL, XML, Kotlin, Dart, Scala, Nix, CMake, R, Elixir, Swift, WGSL, C, C++, C#, Objective-C, Java, Protobuf) through a unified interface while respecting a centralized configuration.

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

The project is a Cargo workspace with 11 crates:

- `cli/` - Main CLI application with file discovery, formatter `Registry`, routing, and `--export` / `--precompile`
- `common/` - Shared types: `Formatter` trait, `ExportedConfig`, `FormatConfig`, indentation/quote styles, and `.editorconfig` renderer
- `formatters/` - Language-specific formatter implementations:
  - `builtin/` (`fama-builtin`) - Consolidated pure-Rust & lightweight CLI formatters (1 module each): Biome (JS/TS/JSX/TSX/JSON/JSONC/HTML/Vue/Svelte/Astro/GraphQL), dprint/Malva (Markdown/YAML/CSS/SCSS/Less/Sass), TOML (Taplo), Rust (`rustfmt`, batched), Python (Ruff), Lua (StyLua), Dockerfile, XML (`quick-xml`), SQL (`sqruff`), PHP (Mago), CMake (`cmakefmt-rust`), Nix (Alejandra), Scala (`scalafmt`, batched), R (`air`, batched), Elixir (`mix format`, batched), WGSL (`wgsl-formatter`)
  - `wasm-runtime/` (`fama-wasm`) - Shared wasmtime engine configuration, `.wasmu` artifact caching, and precompilation helpers
  - `clang/` - C/C++/C#/Objective-C/Java/Protobuf (via clang-format WASM)
  - `dart/` - Dart via dart_style compiled to WASM (dart2wasm output hosted in-process on wasmtime)
  - `swift/` - Swift via swift-format compiled to WASM (SwiftWasm wasip1 build of swift-format 603.0.0 plus a C-ABI shim, hosted in-process on wasmtime with a hand-rolled WASI p1 host; see formatters/swift/README.md)
  - `goffi/` - Shell scripts, Go, and HCL/Terraform (Go FFI wrapper around mvdan/sh, go/format, and hclwrite)
  - `zigffi/` - Zig (Zig FFI wrapper around `zig fmt`)
  - `ruby/` - Ruby (via `rubyfmt` submodule)
  - `kotlin-worker/` - Kotlin via the persistent `ktfmt-worker` protocol (with built-in process fallback via `ktfmt`)

### Data Flow

1. **Discovery** (`cli/src/discovery.rs`): Walk filesystem, filter via `cli/src/registry.rs`, respect `.gitignore`
2. **Resolution** (`cli/src/registry.rs`): Map extension or special filename → `&'static dyn Formatter`
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

For pure-Rust or lightweight CLI formatters:
1. Add any new crate dependency to `formatters/builtin/Cargo.toml`
2. Add a module under `formatters/builtin/src/<lang>.rs` implementing `fama_common::Formatter`
3. Export the module and register `&<lang>::<Lang>Formatter` in `FORMATTERS` in `formatters/builtin/src/lib.rs`

For formatters requiring custom FFI, WASM, submodules, or sidecar binaries:
1. Create a crate under `formatters/<lang>/` implementing `fama_common::Formatter` (and exporting `pub static FORMATTER`)
2. Add it to workspace `members` in root `Cargo.toml` and `cli/Cargo.toml`
3. Register `&<crate>::FORMATTER` in `cli/src/registry.rs`

## Key Dependencies

- **Biome**: Git-pinned for HTML support (specific commit)
- **Ruff**: Git-pinned formatters from Astral's ruff repo
- **dprint + Malva**: Published crates for data/style formats
- **swift-format**: Apple's formatter compiled to wasm32-unknown-wasip1 (SwiftWasm 6.3 SDK) and hosted on wasmtime, like clang-format and dart_style
- **mvdan/sh**: Go library for shell formatting via FFI
- **stylua**: Lua formatter
