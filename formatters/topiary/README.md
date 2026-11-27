# fama-topiary

Tree-sitter-driven declarative code formatters powered by [`topiary-core`](https://crates.io/crates/topiary-core).

## Supported Languages

| Language | Extensions | Grammar Crate | Query Source |
| :--- | :--- | :--- | :--- |
| **Nickel** | `.ncl` | `tree-sitter-nickel` | `topiary-queries::nickel` |
| **OpenSCAD** | `.scad` | `tree-sitter-openscad-ng` | `topiary-queries::openscad` |

## Adding a New Topiary Language

1. Add the Tree-sitter grammar crate (targeting `tree-sitter = 0.26`) to `formatters/topiary/Cargo.toml` and enable its feature in `topiary-queries` (or add an `.scm` query under `formatters/topiary/queries/`).
2. Add a grammar helper and `pub static <LANG>: TopiaryLangFormatter` entry in `formatters/topiary/src/lib.rs`.
3. Append `&<LANG>` to `FORMATTERS` (automatically picked up by `cli/src/registry.rs`) and add a unit test.
