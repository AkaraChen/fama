# Kotlin formatter (`fama-kotlin`)

Formats Kotlin source using [ktfmt](https://github.com/Kotlin/ktfmt) v0.65
native images published by the Kotlin project.

## Why ktfmt?

- Deterministic google-java-format style engine
- Two bundled styles: Meta (2-space default) and Kotlin language guidelines
  (`--kotlinlang-style`, 4-space blocks)
- Supports `.kt` regular files and `.kts` scripts (including
  `build.gradle.kts`, `settings.gradle.kts`)

## Provenance

| Field | Value |
| --- | --- |
| Upstream project | https://github.com/kotlin/ktfmt |
| Version | v0.65 |
| License | Apache-2.0 |
| Release | https://github.com/kotlin/ktfmt/releases/tag/v0.65 |

Native images are downloaded by `scripts/kotlin/download.py` (called manually by
maintainers) with pinned SHA-256 checksums and embedded into Fama's release
binary via `include_bytes!`.

## Configuration behavior

ktfmt is intentionally non-customizable. It does not honor tabs, so Kotlin is
formatted with spaces under this mapping:

| Fama style option | Applied to Kotlin | Notes |
| --- | --- | --- |
| indentation | spaces (4) | upstream `--kotlinlang-style` |
| line width | 80 | `max_line_length` via generated EditorConfig |
| line endings | LF | input separators are normalized to `\n` |
| unused imports | removed | ktfmt default |

You do not need to configure ktfmt per project; Fama ships an isolated
generated `.editorconfig` so ambient repository config cannot change results.

## Supported platforms

Native images: Linux x86_64, Linux ARM64 (built separately by maintainers),
macOS ARM64, Windows x86_64. On any target without a bundled image, Kotlin
formatting fails explicitly rather than silently falling back.

## Usage

```bash
fama "src/**/*.kt"
fama --check "src/main.kt"
```
