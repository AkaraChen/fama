# Fama

[中文](./README_zh-CN.md) | English

**One command. Every language. Zero config.**

## Installation

**macOS / Linux**

```bash
curl -fsSL https://raw.githubusercontent.com/AkaraChen/fama/master/install.sh | sh
```

**Windows**

Download from [Releases](https://github.com/AkaraChen/fama/releases), extract `fama.exe`, and add it to your PATH.

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

**Universal.** One tool formats 60+ languages: JavaScript, TypeScript, JSX, TSX, JSON, JSONC, JSON5, CSS, SCSS, Less, Sass, HTML, Vue, Svelte, Astro, GraphQL, Jinja2, Twig, Nunjucks, Mustache, Handlebars, Vento, YAML, TOML, KDL, Markdown, MDX (`.mdx`), Jupyter (`.ipynb`), LaTeX, BibTeX, Typst, Rust, Python, Lua, Ruby, PHP, Shell, Fish, Nushell, Go, Zig, HCL, Starlark/Bazel, CUE, Jsonnet, Rego, Dockerfile, SQL, XML, Kotlin, Scala, Nix, Nickel, OpenSCAD, CMake, Meson, Just, R, Elixir, Crystal, QML, Mojo, Bicep, V, WGSL, Dart, Swift, C, C++, C#, VB.NET, Objective-C, Java, Protobuf, CUDA, HLSL, Metal, GLSL, SystemVerilog/Verilog, GDScript, Solidity, Gleam, Prisma, Cairo, and Move. Same command everywhere.

**Fast.** Formatting should never be the thing you're waiting for.

**Small.** A single static binary for built-in formatters — no toolchains to install at runtime. Process-mode languages can also rely on host-installed formatter CLIs, such as `ktfmt-worker`/`ktfmt` for Kotlin, `scalafmt` for Scala, `air` for R, `mix` for Elixir, `forge` for Solidity, `gleam` for Gleam, `prisma-fmt` for Prisma, `just` for Just, `cairo-format` for Cairo, `movefmt` for Move, `nufmt` for Nushell, `fish_indent` for Fish, `meson` for Meson, `crystal` for Crystal, `qmlformat` for QML, `mojo` for Mojo, `bicep` for Bicep, and `v` for V — or the bundled `ktfmt-worker` native image for the fastest Kotlin runs. Drop it in your PATH and it just works. Update anytime by running the install script again.

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

| Option         | Default | Description                                                |
| -------------- | ------- | ---------------------------------------------------------- |
| `indent_style` | `Tabs`  | Indentation style: `Tabs` or `Spaces`                      |
| `indent_width` | `4`     | Number of spaces per indentation level (when using spaces) |
| `line_width`   | `80`    | Maximum line length                                        |
| `line_ending`  | `Lf`    | Line ending: `Lf` or `Crlf`                                |

### Language-Specific Options

| Option            | Default    | Description                                         | Languages                                           |
| ----------------- | ---------- | --------------------------------------------------- | --------------------------------------------------- |
| `quote_style`     | `Double`   | Quote preference: `Single` or `Double`              | JavaScript, TypeScript, Python, Lua, CSS, SCSS, PHP |
| `trailing_comma`  | `All`      | Trailing comma style: `All` or `None`               | JavaScript, TypeScript, JSON, PHP                   |
| `semicolons`      | `Always`   | Semicolon usage: `Always` or `AsNeeded`             | JavaScript, TypeScript                              |
| `bracket_spacing` | `true`     | Spaces inside object brackets                       | JavaScript, TypeScript                              |
| `brace_style`     | `SameLine` | Brace style: `SameLine` (K&R) or `NewLine` (Allman) | CSS, SCSS, C-family                                 |

### Language Support Matrix

| Language        | Formatter    | Core Options | Quote | Trailing Comma | Semicolons | Bracket Spacing | Notes                             |
| --------------- | ------------ | ------------ | ----- | -------------- | ---------- | --------------- | --------------------------------- |
| **JavaScript**  | Biome        | ✅           | ✅    | ✅             | ✅         | ✅              | Includes JSX                      |
| **TypeScript**  | Biome        | ✅           | ✅    | ✅             | ✅         | ✅              | Includes TSX                      |
| **JSON**        | Biome        | ✅           | ❌    | ✅             | N/A        | N/A             | Trailing comma = All/None         |
| **JSONC**       | Biome        | ✅           | ❌    | ❌             | N/A        | N/A             | Comments allowed                  |
| **JSON5**       | dprint-json  | ✅           | N/A   | ✅             | N/A        | N/A             | Native crate (`dprint-plugin-json`) |
| **Jupyter**     | dprint-jupyter | ✅*        | N/A   | N/A            | N/A        | N/A             | `.ipynb` cells routed through Fama formatters |
| **HTML**        | Biome        | ✅           | N/A   | N/A            | N/A        | N/A             | Includes Vue/Svelte/Astro         |
| **GraphQL**     | Biome        | ✅           | N/A   | N/A            | N/A        | N/A             |                                   |
| **CSS**         | dprint/Malva | ✅           | ✅    | ✅             | N/A        | N/A             | Includes SCSS, LESS, Sass         |
| **Markdown**    | dprint       | ✅*          | N/A   | N/A            | N/A        | N/A             | *line_width, line_ending only     |
| **MDX**         | markdown-rs + Biome + dprint | ✅ | ✅ | ✅             | ✅         | ✅              | Native `.mdx` (Markdown, JSX/ESM, YAML/TOML frontmatter) |
| **YAML**        | dprint       | ✅           | N/A   | N/A            | N/A        | N/A             |                                   |
| **Dockerfile**  | dprint       | ✅           | N/A   | N/A            | N/A        | N/A             |                                   |
| **TOML**        | Taplo        | ✅           | N/A   | N/A            | N/A        | N/A             | Uses CONFIG.indent_width          |
| **Rust**        | rustfmt      | ✅           | N/A   | N/A            | N/A        | N/A             | Uses rustfmt config env vars      |
| **Python**      | Ruff         | ✅           | ✅    | N/A            | N/A        | N/A             |                                   |
| **Lua**         | StyLua       | ✅           | ✅    | N/A            | N/A        | N/A             |                                   |
| **PHP**         | Mago         | ✅           | ✅    | ✅             | ✅         | N/A             |                                   |
| **Ruby**        | rubyfmt      | ❌           | ❌    | ❌             | ❌         | ❌              | Embedded Ruby, no config          |
| **Shell**       | goffi        | ✅*          | N/A   | N/A            | N/A        | N/A             | *indent_style, indent_width only  |
| **Go**          | goffi        | ❌           | ❌    | ❌             | ❌         | ❌              | Uses gofmt defaults (tabs)        |
| **HCL**         | goffi        | ❌           | ❌    | ❌             | ❌         | ❌              | Uses hclwrite defaults (2 spaces) |
| **Zig**         | dprint-zig   | ❌           | ❌    | ❌             | ❌         | ❌              | Via dprint WASM (`dprint-plugin-zig`, `.zig` / `.zon`) |
| **SQL**         | sqruff       | ✅           | N/A   | N/A            | N/A        | N/A             | Keywords capitalized              |
| **XML**         | quick-xml    | ✅           | N/A   | N/A            | N/A        | N/A             |                                   |
| **Kotlin**      | ktfmt (worker)  | ✅*       | N/A   | N/A            | N/A        | N/A             | *Uses generated `.editorconfig`; via the persistent `ktfmt-worker` (recommended) or host `ktfmt` CLI |
| **Scala**       | scalafmt     | ✅*          | N/A   | N/A            | N/A        | N/A             | *maxColumn, indent only; official native binary; spaces (scalafmt has no tab mode) |
| **Nix**         | alejandra    | ❌           | N/A   | N/A            | N/A        | N/A             | Opinionated alejandra style       |
| **CMake**       | cmakefmt     | ❌           | N/A   | N/A            | N/A        | N/A             | cmakefmt defaults                 |
| **R**           | Air          | ✅*          | N/A   | N/A            | N/A        | N/A             | *indent, line-width, line-ending via generated `air.toml`; host `air` binary |
| **Elixir**      | mix format   | ✅*          | N/A   | N/A            | N/A        | N/A             | *line_length via generated `.formatter.exs`; requires Elixir on PATH |
| **Dart**        | dart_style   | ✅*          | N/A   | N/A            | N/A        | N/A             | *line_width, line_ending only; via WASM (dart2wasm + wasmtime) |
| **C/C++**       | clang-format | ✅           | N/A   | N/A            | N/A        | N/A             | Via WASM                          |
| **C#**          | CSharpier    | ✅*          | N/A   | N/A            | N/A        | N/A             | Via dprint WASM (`dprint-plugin-csharpier`) |
| **VB.NET**      | Roslyn       | ✅*          | N/A   | N/A            | N/A        | N/A             | Via dprint WASM (`dprint-plugin-roslyn`) |
| **Objective-C** | clang-format | ✅           | N/A   | N/A            | N/A        | N/A             | Via WASM                          |
| **Java**        | clang-format | ✅           | N/A   | N/A            | N/A        | N/A             | Via WASM                          |
| **Protobuf**    | clang-format | ✅           | N/A   | N/A            | N/A        | N/A             | Via WASM                          |
| **CUDA**        | clang-format | ✅           | N/A   | N/A            | N/A        | N/A             | Via WASM (`.cu`, `.cuh`)          |
| **HLSL / GLSL / Metal** | clang-format | ✅   | N/A   | N/A            | N/A        | N/A             | Via WASM (`.hlsl`, `.glsl`, `.vert`, `.frag`, `.comp`, `.metal`, etc.) |
| **SystemVerilog / Verilog** | clang-format | ✅ | N/A | N/A            | N/A        | N/A             | Via WASM; `.v` uses content disambiguation |
| **Starlark / Bazel** | goffi   | ❌           | ❌    | ❌             | ❌         | ❌              | Uses `buildifier` defaults (`BUILD`, `WORKSPACE`, `MODULE.bazel`, `.bzl`, `.star`) |
| **CUE**         | goffi        | ✅*          | N/A   | N/A            | N/A        | N/A             | *indent_style, indent_width via `cue/format` |
| **Jsonnet**     | goffi        | ✅*          | N/A   | N/A            | N/A        | N/A             | *indent_width via `go-jsonnet`    |
| **Rego**        | goffi        | ❌           | ❌    | ❌             | ❌         | ❌              | Uses `opa fmt` defaults (v1 + v0 fallback) |
| **Nickel**      | topiary      | ✅*          | N/A   | N/A            | N/A        | N/A             | *indent_style, indent_width via Topiary |
| **OpenSCAD**    | topiary      | ✅*          | N/A   | N/A            | N/A        | N/A             | *indent_style, indent_width via Topiary |
| **Typst**       | typstyle     | ✅*          | N/A   | N/A            | N/A        | N/A             | *indent_width, line_width, line_ending |
| **Templates**   | markup_fmt   | ✅           | N/A   | N/A            | N/A        | N/A             | Jinja2, Twig, Nunjucks, Mustache, Handlebars, Vento |
| **KDL**         | kdl          | ✅*          | N/A   | N/A            | N/A        | N/A             | *indent_style, indent_width       |
| **GDScript**    | gdck-format  | ✅*          | N/A   | N/A            | N/A        | N/A             | *indent_style, indent_width, line_width |
| **LaTeX / BibTeX** | tex-fmt   | ✅*          | N/A   | N/A            | N/A        | N/A             | *indent_style, indent_width, line_width; `.cls` uses content disambiguation |
| **Solidity**    | forge fmt    | ❌           | N/A   | N/A            | N/A        | N/A             | Process mode; host `forge` or `FAMA_FORGE_PATH` |
| **Gleam**       | gleam format | ❌           | N/A   | N/A            | N/A        | N/A             | Process mode; host `gleam` or `FAMA_GLEAM_PATH` |
| **Prisma**      | prisma-fmt   | ❌           | N/A   | N/A            | N/A        | N/A             | Process mode; host `prisma-fmt` or `FAMA_PRISMA_FMT_PATH` |
| **Just**        | just --fmt   | ❌           | N/A   | N/A            | N/A        | N/A             | Process mode; host `just` or `FAMA_JUST_PATH` |
| **Cairo**       | cairo-format | ❌           | N/A   | N/A            | N/A        | N/A             | Process mode; host `cairo-format` or `FAMA_CAIRO_FORMAT_PATH` |
| **Move**        | movefmt      | ❌           | N/A   | N/A            | N/A        | N/A             | Process mode; host `movefmt` or `FAMA_MOVEFMT_PATH` |
| **Nushell**     | nufmt        | ❌           | N/A   | N/A            | N/A        | N/A             | Process mode; host `nufmt` or `FAMA_NUFMT_PATH` |
| **Fish**        | fish_indent  | ❌           | N/A   | N/A            | N/A        | N/A             | Process mode; host `fish_indent` or `FAMA_FISH_INDENT_PATH` |
| **Meson**       | meson format | ❌           | N/A   | N/A            | N/A        | N/A             | Process mode (`meson.build`, `meson.options`, `meson_options.txt`); host `meson` >= 1.5 or `FAMA_MESON_PATH` |
| **Crystal**     | crystal tool format | ❌    | N/A   | N/A            | N/A        | N/A             | Process mode; host `crystal` or `FAMA_CRYSTAL_PATH` |
| **QML**         | qmlformat    | ✅*          | N/A   | N/A            | N/A        | N/A             | *indent_style, indent_width, line_ending; process mode; host `qmlformat` or `FAMA_QMLFORMAT_PATH` |
| **Mojo**        | mojo format  | ✅*          | N/A   | N/A            | N/A        | N/A             | *line_width (`.mojo`, `.🔥`); process mode; host `mojo` or `FAMA_MOJO_PATH` |
| **Bicep**       | bicep format | ✅*          | N/A   | N/A            | N/A        | N/A             | *indent_style, indent_width, line_ending (`.bicep`, `.bicepparam`); process mode; host `bicep` or `FAMA_BICEP_PATH` |
| **V**           | v fmt        | ❌           | N/A   | N/A            | N/A        | N/A             | Process mode; `.v` uses content disambiguation vs Verilog/Coq; host `v` or `FAMA_V_PATH` |
| **WGSL**        | wgsl-formatter | ✅         | N/A   | N/A            | N/A        | N/A             | Native crate from wgsl-analyzer   |
| **Swift**       | swift-format | ✅*          | N/A   | N/A            | N/A        | N/A             | *indent style/width, line_width via generated config JSON, line_ending host-side; via WASM (SwiftWasm + wasmtime) |

### Notes on Hardcoded Styles

Some formatters use hardcoded styles that cannot be configured:

- **Go**: Uses `gofmt` defaults (tabs for indentation)
- **HCL**: Uses `hclwrite` defaults (2 spaces)
- **Starlark / Bazel**: Uses `buildifier` canonical defaults
- **Rego**: Uses `opa fmt` canonical defaults
- **Nix**: Uses alejandra's fixed, opinionated style
- **CMake**: Uses cmakefmt defaults (lowercased commands, capitalized spec keywords)
- **Scala**: scalafmt has no tab mode — Scala is indented with 4 spaces even though fama's default is tabs; `maxColumn` and indent follow fama's config
- **Kotlin**: Uses ktfmt in kotlinlang style; the optional `ktfmt-worker` native image speeds up large runs (see `ktfmt-worker/README.md`)
- **Dart**: Uses dart_style with fama's `line_width` and `line_ending` settings; runs in-process as a WASM module
- **Swift**: Uses Apple's swift-format (603.0.0, unpatched) with fama's `indent_style`/`indent_width`/`line_width` via a generated config JSON; `line_ending` is applied host-side because swift-format has no line-ending knob; runs in-process as a WASM module
- **Zig**: Uses Zig's built-in formatter with default style
- **Ruby**: Uses embedded `rubyfmt` with fixed style
- **Solidity / Gleam / Prisma / Just / Cairo / Move / Nushell / Fish / Meson / Crystal / QML / Mojo / Bicep / V**: Process-mode formatters delegating to their official ecosystem CLIs (with canonical defaults where the CLI does not expose style knobs, and fama config flags forwarded for `qmlformat`, `mojo format`, and `bicep format`)

### Configuration Export

Fama can export configuration files for tools that need them:

```bash
fama --export
```

This generates:

- `.editorconfig` - Editor-agnostic configuration, also used by process-based formatters like `ktfmt`
- `rustfmt.toml` - Rust-specific formatting rules

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
