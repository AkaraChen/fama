# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Fama is a unified cross-language code formatter written in Rust that aggregates multiple specialized formatters into a single CLI tool. It formats diverse file types (JavaScript, TypeScript, Python, Rust, YAML, Markdown, Shell, Dockerfile, and more) through a unified interface while respecting a centralized configuration.

## Build Commands

```bash
make build      # Debug build
make release    # Optimized release build (includes macOS dylib fix)
make test       # Run all tests (cargo test)
make dev        # Debug build with dylib fix
make install    # Install to /usr/local/bin (requires sudo)
make clean      # Remove build artifacts
```

## CLI Usage

```bash
fama [PATTERN]   # Format files matching glob pattern (default: **/*)
fama --export    # Generate .editorconfig and rustfmt.toml files
```

## Architecture

### Workspace Structure

The project is a Cargo workspace with 8 crates:
- `cli/` - Main CLI application with file discovery and routing
- `common/` - Shared types: `FileType` enum, `FormatConfig`, indentation/quote styles
- `formatters/` - Language-specific formatter implementations:
  - `biome/` - JS/TS/JSX/TSX/HTML/Vue/Svelte/Astro (via Biome crates)
  - `dprint/` - Markdown, YAML, CSS/SCSS/LESS/Sass (via dprint + Malva)
  - `rustfmt/` - Rust (via rust-format crate)
  - `python/` - Python (via ruff crates)
  - `lua/` - Lua (via stylua crate)
  - `shfmt/` - Shell scripts (Go FFI wrapper around mvdan/sh)
  - `dockerfile/` - Dockerfile formatting

### Data Flow

1. **Discovery** (`cli/src/discovery.rs`): Walk filesystem, filter by supported extensions, respect `.gitignore`
2. **Type Detection** (`common/src/lib.rs`): Map file extension → `FileType` enum
3. **Routing** (`cli/src/formatter.rs`): Match `FileType` → call appropriate formatter
4. **Formatting**: Each formatter receives content string, returns formatted string
5. **Write-back**: If changed, write to disk; track stats (formatted, unchanged, errors)

### Formatter Interface

All formatters implement the same pattern:
```rust
pub fn format_file(content: &str, path: &str, file_type: FileType) -> Result<String, String>
```

### Configuration

Centralized `FormatConfig` in `common/src/lib.rs` with go-fmt style defaults:
- Tabs for indentation (width: 4)
- 80 character line width
- LF line endings
- Double quotes, trailing commas, semicolons always

### Go FFI (shfmt)

The shell formatter wraps mvdan/sh via CGO:
- Go source in `formatters/shfmt/go/`
- Compiled as platform-specific shared library (.dylib/.so/.dll)
- Rust FFI bindings in `formatters/shfmt/src/lib.rs`
- `build.rs` handles Go compilation and library linking

The macOS build requires `install_name_tool` to fix dylib paths (handled by `make release`).

## Testing

```bash
cargo test                           # All workspace tests
cargo test -p fama-common            # Test specific crate
cargo test -p fama-cli               # Test CLI crate
```

## Adding a New Formatter

1. Create new crate under `formatters/`
2. Add to workspace members in root `Cargo.toml`
3. Add `FileType` variant(s) to `common/src/lib.rs`
4. Add extension detection in `detect_file_type()`
5. Add routing case in `cli/src/formatter.rs`
6. Update the `cli/Cargo.toml` dependencies

## Key Dependencies

- **Biome**: Git-pinned for HTML support (specific commit)
- **Ruff**: Git-pinned formatters from Astral's ruff repo
- **dprint + Malva**: Published crates for data/style formats
- **mvdan/sh**: Go library for shell formatting via FFI
