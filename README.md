# Fama

**One command. Every language. Zero config.**

## Philosophy

Fama exists because code formatting should be invisible.

You shouldn't spend time configuring formatters. You shouldn't debate tabs versus spaces. You shouldn't maintain different settings for different languages. You shouldn't sync IDE configurations across machines.

You should write code, run `fama`, and move on.

## What It Does

```
fama
```

That's it. Run it in any project. It formats everything it understands and leaves everything else untouched.

## Why It Exists

Every formatter has its own configuration file. Every IDE has its own settings. Every team has its own style guide. Every new project requires setup.

This is waste.

Fama makes a decision so you don't have to. It picks sensible defaults and applies them universally. No `.prettierrc`. No `settings.json`. No arguments about semicolons.

## Principles

**Convention over configuration.** There is one style. It works. Use it.

**Universal.** One tool formats your JavaScript, TypeScript, JSX, TSX, JSON, CSS, SCSS, Less, Sass, HTML, Vue, Svelte, Astro, YAML, TOML, Markdown, Rust, Python, Lua, Shell, Go, and Dockerfile. Same command everywhere.

**Fast.** Formatting should never be the thing you're waiting for.

**Small.** A single 13 MB binary. Download is just a 5 MB tar.gz. No runtime dependencies. No package managers. No plugins. Drop it in your PATH and it just works. Update anytime by running the install script again.

**Quiet.** It formats what changed and tells you what it did. Nothing more.

## Usage

```bash
# Format everything
fama

# Format specific files
fama "src/**/*.ts"

# Export settings for tools that need them
fama --export
```

## The Fama Style

- Tabs for indentation
- 80 character lines
- Double quotes
- Trailing commas
- Semicolons

This is not negotiable. That's the point.

## Configuration Options

Fama uses a unified configuration that applies across all formatters. Below is the complete list of supported options and which languages support them:

### Core Options

| Option | Default | Description |
|--------|---------|-------------|
| `indent_style` | `Tabs` | Indentation style: `Tabs` or `Spaces` |
| `indent_width` | `4` | Number of spaces per indentation level (when using spaces) |
| `line_width` | `80` | Maximum line length |
| `line_ending` | `Lf` | Line ending: `Lf` or `Crlf` |

### Language-Specific Options

| Option | Default | Description | Languages |
|--------|---------|-------------|-----------|
| `quote_style` | `Double` | Quote preference: `Single` or `Double` | JavaScript, TypeScript, Python, Lua, CSS, SCSS, PHP |
| `trailing_comma` | `All` | Trailing comma style: `All` or `None` | JavaScript, TypeScript, JSON, PHP |
| `semicolons` | `Always` | Semicolon usage: `Always` or `AsNeeded` | JavaScript, TypeScript |
| `bracket_spacing` | `true` | Spaces inside object brackets | JavaScript, TypeScript |
| `brace_style` | `SameLine` | Brace style: `SameLine` (K&R) or `NewLine` (Allman) | CSS, SCSS, C-family |

### Language Support Matrix

| Language | Formatter | Core Options | Quote | Trailing Comma | Semicolons | Bracket Spacing | Notes |
|----------|-----------|------------|-------|----------------|------------|-----------------|-------|
| **JavaScript** | Biome | ✅ | ✅ | ✅ | ✅ | ✅ | Includes JSX |
| **TypeScript** | Biome | ✅ | ✅ | ✅ | ✅ | ✅ | Includes TSX |
| **JSON** | Biome | ✅ | ❌ | ✅ | N/A | N/A | Trailing comma = All/None |
| **JSONC** | Biome | ✅ | ❌ | ❌ | N/A | N/A | Comments allowed |
| **HTML** | Biome | ✅ | N/A | N/A | N/A | N/A | Includes Vue/Svelte/Astro |
| **GraphQL** | Biome | ✅ | N/A | N/A | N/A | N/A | |
| **CSS** | dprint/Malva | ✅ | ✅ | ✅ | N/A | N/A | Includes SCSS, LESS, Sass |
| **Markdown** | dprint | ✅* | N/A | N/A | N/A | N/A | *line_width, line_ending only |
| **YAML** | dprint | ✅ | N/A | N/A | N/A | N/A | |
| **Dockerfile** | dprint | ✅ | N/A | N/A | N/A | N/A | |
| **TOML** | Taplo | ✅ | N/A | N/A | N/A | N/A | Uses CONFIG.indent_width |
| **Rust** | rustfmt | ✅ | N/A | N/A | N/A | N/A | Uses rustfmt config env vars |
| **Python** | Ruff | ✅ | ✅ | N/A | N/A | N/A | |
| **Lua** | StyLua | ✅ | ✅ | N/A | N/A | N/A | |
| **PHP** | Mago | ✅ | ✅ | ✅ | ✅ | N/A | |
| **Ruby** | rubyfmt | ❌ | ❌ | ❌ | ❌ | ❌ | Embedded Ruby, no config |
| **Shell** | goffi | ✅* | N/A | N/A | N/A | N/A | *indent_style, indent_width only |
| **Go** | goffi | ❌ | ❌ | ❌ | ❌ | ❌ | Uses gofmt defaults (tabs) |
| **HCL** | goffi | ❌ | ❌ | ❌ | ❌ | ❌ | Uses hclwrite defaults (2 spaces) |
| **Zig** | zigffi | ❌ | ❌ | ❌ | ❌ | ❌ | Uses Zig defaults |
| **SQL** | sqruff | ✅ | N/A | N/A | N/A | N/A | Keywords capitalized |
| **XML** | quick-xml | ✅ | N/A | N/A | N/A | N/A | |
| **C/C++** | clang-format | ✅ | N/A | N/A | N/A | N/A | Via WASM |
| **C#** | clang-format | ✅ | N/A | N/A | N/A | N/A | Via WASM |
| **Objective-C** | clang-format | ✅ | N/A | N/A | N/A | N/A | Via WASM |
| **Java** | clang-format | ✅ | N/A | N/A | N/A | N/A | Via WASM |
| **Protobuf** | clang-format | ✅ | N/A | N/A | N/A | N/A | Via WASM |

### Notes on Hardcoded Styles

Some formatters use hardcoded styles that cannot be configured:

- **Go**: Uses `gofmt` defaults (tabs for indentation)
- **HCL**: Uses `hclwrite` defaults (2 spaces)
- **Zig**: Uses Zig's built-in formatter with default style
- **Ruby**: Uses embedded `rubyfmt` with fixed style

### Configuration Export

Fama can export configuration files for tools that need them:

```bash
fama --export
```

This generates:
- `.editorconfig` - Editor-agnostic configuration
- `rustfmt.toml` - Rust-specific formatting rules

## Install

**macOS / Linux**

```bash
curl -fsSL https://raw.githubusercontent.com/AkaraChen/fama/master/install.sh | sh
```

**Windows**

Download from [Releases](https://github.com/AkaraChen/fama/releases), extract `fama.exe`, and add it to your PATH.

---

## Why "Fama"?

```plaintext
format
   ↓     too long, programmers are lazy
  fmt
   ↓     still too cold, needs some vowels
famata
   ↓     wait, that's too long again
 fama
   ↓
  :)
```
