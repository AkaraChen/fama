# dprint Plugin Compatibility & Overlap Benchmark Report

Measured in `--release` on Linux `x86_64` via `fama-dprint-wasm` (`wasmtime` 51.0, Cranelift JIT + `.wasmu` precompilation cache) against Fama's existing implementations.

Reproduce:

```bash
cargo test -p fama-dprint-wasm --release --test benchmark -- --ignored --nocapture
```

---

## 1. Size, Load Time, and Per-File Latency

Each per-file latency figure is the mean of 20 warm iterations after 1 warmup call on a representative single-file snippet.

| Plugin / Case | Raw `.wasm` | `.wasm.zst` (`-19`) | `.wasmu` | Cold Compile | Warm Load (`.wasmu`) | dprint WASM / file | Fama Existing / file | Notes |
| :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | :--- |
| **csharpier** (`.cs`) `0.3.1` | 20.06 MB | 3.40 MB | 38.83 MB | 1654 ms | 31.6 ms | **0.07 ms** | 0.19 ms (`fama-clang`) | **2.7x faster** than clang-format + idiomatic CSharpier |
| **roslyn** (`.vb`) `0.24.1` | 22.98 MB | 3.55 MB | 40.40 MB | 1410 ms | 21.8 ms | **0.16 ms** | N/A | Zero-.NET-SDK **VB.NET** support |
| **roslyn** (`.cs`) `0.24.1` | 22.98 MB | 3.55 MB | 40.40 MB | *(cached 23 ms)* | 22.2 ms | 0.33 ms | 0.19–0.20 ms (`fama-clang`) | Shares `.wasm` with VB.NET; CSharpier is preferred for C# |
| **clang-format** (`.cpp`) `0.3.2` | 3.77 MB | 0.88 MB | 6.23 MB | 220 ms | 3.5 ms | 0.86 ms | **0.44 ms** (`fama-clang`) | Removes custom Emscripten submodule, ~2x slower per file |
| **zig** (`.zig`) `0.1.2` | 0.20 MB | **0.07 MB** | 0.45 MB | 43 ms | 0.5 ms | **0.01 ms** | 0.03 ms (`zigffi`) | **Smaller, faster, and eliminates `Setup Zig` in CI** |
| **terraform** (`.tf`) `0.2.0` | 1.29 MB | 0.41 MB | 2.32 MB | 148 ms | 1.8 ms | 0.05 ms | **0.02 ms** (`goffi`) | TinyGo WASM; eliminates `hclwrite` from Go CGO |
| **buildifier** (`BUILD.bazel`) `0.3.0` | 0.71 MB | 0.22 MB | 1.41 MB | 81 ms | 1.2 ms | 0.04 ms | **0.01 ms** (`goffi`) | TinyGo WASM; eliminates `buildtools` from Go CGO |
| **ruff** (`.py`) `0.12.3` | 11.68 MB | 2.39 MB | 21.72 MB | 1041 ms | 8.8 ms | 0.01 ms | **0.00 ms** (native Rust) | Keep native crate; WASM adds 21.7 MB `.wasmu` bloat |
| **typstyle** (`.typ`) `0.6.1` | 0.86 MB | 0.22 MB | 1.59 MB | 71 ms | 1.4 ms | 0.01 ms | **0.01 ms** (native Rust) | Keep native crate; zero JIT/WASM overhead |
| **alejandra** (`.nix`) `0.2.0` | 0.46 MB | 0.12 MB | 0.90 MB | 44 ms | 0.9 ms | 0.04 ms | **0.02 ms** (native Rust) | Keep native crate |
| **json** (`.json5`) `0.26.0` | 0.63 MB | 0.18 MB | 1.27 MB | 69 ms | 1.0 ms | 0.01 ms | **0.01 ms** (native Rust) | Keep native crate (`dprint-plugin-json`) |

*Reference existing artifacts in repo*: `clang-format.wasm.zst` (`890 KB` zstd / `2.60 MB` raw), `dart_fmt.wasm.zst` (`256 KB` zstd / `880 KB` raw), `swiftformat.wasm.zst` (`5.3 MB` zstd / `62 MB` raw), `zig.wasm.zst` (`76 KB` zstd / `209 KB` raw), `libgoffi-x86_64-unknown-linux-gnu.a` (`32 MB` static archive).

---

## 2. Build & CI Toolchains Saved

| Replacement | Current Build / CI Dependency | With `fama-dprint-wasm` |
| :--- | :--- | :--- |
| **`dprint-plugin-zig`** vs `zigffi` | Requires `zig 0.15.0` installed in CI (`mlugg/setup-zig@v2`) and local dev + `zig build-lib` static linking | **Zero Zig toolchain needed**; `76 KB` `.wasm.zst`, `0.5 ms` warm load, faster per-file |
| **`dprint-plugin-terraform` + `buildifier`** vs `goffi` | Contributes to the `32 MB` Go CGO `libgoffi.a` + `gcc-aarch64-linux-gnu` Linux ARM64 cross-compiler | Pure TinyGo WASM (`650 KB` combined `.wasm.zst`), zero CGO/cross-CC needed for HCL/Starlark |
| **`dprint-plugin-clang-format`** vs `fama-clang` | Custom Emscripten submodule (`formatters/clang/clang-format`) + bespoke C++/WASI glue | Official upstream `.wasm` (`0.88 MB` zst), ~2x slower per file (`0.86 ms` vs `0.44 ms`) |
| **`dprint-plugin-csharpier` & `roslyn`** (New/Upgrade) | C# used `clang-format` approximations; VB.NET unsupported without `.NET SDK` | NativeAOT-LLVM WASM (`3.40 MB` + `3.55 MB` `.wasm.zst`), zero `.NET SDK` required |

---

## 3. Architectural Takeaways & Recommendations

### Adopted
1. **Pure-Rust dprint crates in `fama-web`** (split out from `fama-builtin`):
   - **`dprint-plugin-json` (`0.26.0`)** → **JSON5** (`.json5`) with zero WASM overhead.
   - **`dprint-plugin-jupyter` (`0.6.0`)** → **Jupyter Notebook** (`.ipynb`), routing cells back through `fama_common::format_embedded` to Fama's full `Registry`.
2. **Schema v4 Host (`fama-dprint-wasm`) + C#, VB.NET & Zig**:
   - **C#** (`.cs`, `.csx`) upgraded from `clang-format` to **`dprint-plugin-csharpier`** (`3.40 MB` `.wasm.zst`, `0.07 ms/file` — faster than `clang-format` and idiomatic).
   - **VB.NET** (`.vb`) enabled via **`dprint-plugin-roslyn`** (`3.55 MB` `.wasm.zst`, `0.16 ms/file`).
   - **Zig** (`.zig`, `.zon`) migrated from `formatters/zigffi` to **`dprint-plugin-zig`** (`76 KB` `.wasm.zst`, `0.01 ms` vs `0.03 ms`, `43 ms` cold / `0.5 ms` warm — eliminates `formatters/zigffi` and removes `Setup Zig` from CI & Release workflows).
3. **Unified `zstd -19` compression in `fama-wasm`**:
   - Compressed `clang-format.wasm` (`2.60 MB` → `890 KB`) and `dart_fmt.wasm` (`880 KB` → `256 KB`) while keeping `fama-clang` (`0.44 ms/file`, 2x faster than `dprint-plugin-clang-format`).

### Notes & Decisions Made
1. **Watch Release `.wasmu` archive budget for `.NET` WASM**: `csharpier` and `roslyn` are compact as `.wasm.zst` (`3.40 MB` / `3.55 MB`), but their Cranelift `.wasmu` artifacts are `38.8 MB` and `40.4 MB` each. If release archive size becomes an issue, `fama --precompile` can exclude or split large `.wasmu` artifacts while relying on first-run `~/.cache/fama` JIT caching (~1.4–1.6s one-time).
2. **Keep `goffi` (HCL/Starlark + Go/Shell/CUE/Jsonnet/Rego)**: Go CGO is still required for Go, Shell, CUE, Jsonnet, and Rego, and native `goffi` formats HCL/Starlark 2–4x faster without adding `.wasmu` files.
3. **Keep all native Rust crates native**: `ruff`, `typstyle`, `alejandra`, `dprint-plugin-json`, `biome`, `stylua`, `mago` — native Rust crates are smaller, have zero JIT/WASM boundary overhead, and match or beat WASM speed.

### Engineering Lessons Learned from `dprint`
- **Shared-buffer Schema v4 ABI** (`clear_shared_bytes` / `get_shared_bytes_ptr`) avoids per-call `malloc`/`free` churn and works uniformly across Rust, TinyGo, Zig, C++, and .NET NativeAOT-LLVM guests.
- **Zstd-compressed embedded WASM + bounded `InstancePool` + `.wasmu` precompilation** keeps binary size compact (5–6x smaller on `.NET` / `Swift` WASM) while keeping steady-state formatting sub-millisecond.
- **TinyGo WASI reactors** produce sub-megabyte Go formatters (`230–420 KB` compressed) without CGO or platform-specific cross-compilers.
