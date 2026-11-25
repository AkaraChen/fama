# fama-dart

Dart formatting for fama using **real dart_style** compiled to WebAssembly.

The WASM binary is [wasm-fmt/dart_fmt](https://github.com/wasm-fmt/dart_fmt)
(dart_style plus a tiny JS binding) compiled with **dart2wasm**. fama hosts it
in-process with **wasmtime** (Cranelift JIT) - no sidecar process, no temp
files, no Dart SDK needed at runtime.

## Architecture

The module is built with js-string builtins and the dart2wasm js-interop
runtime, which normally requires a JavaScript engine to host. Without one,
this crate supplies three pieces:

| Import surface | What it is | How it's hosted |
| --- | --- | --- |
| ~4200 `(import "" "<content>" (global (ref extern)))` | Constant strings, one global per literal (import name = string content) | Created per instantiation in `lib.rs` as `ExternRef` globals |
| `wasm:js-string::*` (9 functions) | The WebAssembly js-string builtins | Implemented natively over UTF-16 strings in `shims.rs` |
| `dart2wasm::*` (single-letter names) | dart2wasm's js-interop runtime helpers (property access, closures wrapped for JS, RegExp, string ops not in js-string builtins) | Implemented over a small `JsVal` Rust model (strings, objects, arrays, functions, regexes) in `shims.rs` |

Flow per format call:

1. On instantiation, the module's `main` registers the Dart `format` closure
   via the `globalThis.format` helper; the host captures the closure's
   trampoline funcref in a per-thread registry.
2. `format_file` builds options JSON (`pageWidth` from fama's `line_width`,
   `lineEnding` from `line_ending`), allocates three JS strings
   (source, filename, options) as `externref`s, and invokes the trampoline
   funcref with `(closure, argc=3, source, filename, options)`.
3. The Dart side returns a `{success, code, error}` JS object; the host reads
   the fields back out of its `JsVal` model.

Strings are stored as UTF-16 code units because every JS string operation -
and therefore every `String` operation dart_style performs - is indexed in
UTF-16 code units. Objects mutated across calls (the result object, RegExp
`lastIndex`) mutate the `ExternRef` payload in place, matching JS object
identity semantics.

Performance follows the clang-format crate's patterns: one engine+compile per
process (with the compiled module cached/deserialized from a
sha256-named `.wasmu` artifact when available) and one reusable instance per
rayon worker thread.

Helper functions dart_style never reaches (typed-data DataView accessors,
promises, Math interop) trap loudly on call, so surprises surface as
formatting errors instead of silent corruption.

## Rebuilding the WASM module

The module only needs rebuilding when dart_style (or the wasm-fmt/dart_fmt
binding) is updated. Requires **Dart SDK >= 3.14**: dart2wasm started
emitting standard `try_table` exception handling there; older SDKs emit
legacy-exceptions bytecode that wasmtime refuses to compile.

```bash
cd formatters/dart
./build-wasm.sh
```

The script clones wasm-fmt/dart_fmt and runs
`dart pub get && dart compile wasm ./lib/binding.dart -o wasm/dart_fmt.wasm`.

To ship a precompiled artifact for faster startup, release CI runs
`cargo run --bin dart-precompile -- <dir>`, producing
`dart_fmt-<sha256-prefix>.wasmu` next to the fama binary.
