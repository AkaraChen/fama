# fama-clang

clang-format WASM-based formatter for C/C++/Java/C#/Objective-C/Protobuf.

## Building the WASM Binary

The WASM binary is checked into the repository at `wasm/clang-format.wasm`. If you need to rebuild it:

### Prerequisites

1. **Emscripten SDK 4.0.23** (specific version required)
2. **Ninja** build system
3. **Clang** (native compiler for building LLVM tools)

### Setup

```bash
# Install emsdk
git clone https://github.com/emscripten-core/emsdk.git ~/emsdk
cd ~/emsdk
./emsdk install 4.0.23
./emsdk activate 4.0.23

# Install ninja (macOS)
brew install ninja

# Install ninja (Ubuntu)
sudo apt-get install ninja-build
```

### Build

```bash
# Activate emsdk
source ~/emsdk/emsdk_env.sh

# Build WASM (takes 20-40 minutes)
cd formatters/clang
./build-wasm.sh
```

The build will:

1. Download and compile LLVM/Clang (requires ~8GB RAM)
2. Build `clang-format-standalone.wasm`
3. Copy the result to `wasm/clang-format.wasm`

### Troubleshooting

**"resource temporarily unavailable" error**

- Close other applications to free system resources
- Restart your computer and try again

**CMake libstdc++ version error**

- Make sure you're using emsdk 4.0.23 (not homebrew's emscripten)
- The `LLVM_COMPILER_CHECKED=ON` flag in CMakeLists.txt should skip this check

**Native compiler errors (libc++ incompatibility)**

- The build script uses `/usr/bin/clang` (Xcode clang) instead of homebrew's llvm
- If you have homebrew llvm in PATH, it may cause libc++ header incompatibilities
- The build script explicitly sets `CC=/usr/bin/clang` and `CXX=/usr/bin/clang++`

## Architecture

This crate uses [wasmi](https://github.com/wasmi-labs/wasmi) to run the clang-format WASM binary. The WASM binary is embedded at compile time and provides the following exports:

- `wasm_init()` - Initialize clang-format
- `wasm_set_style(ptr, len)` - Set formatting style
- `wasm_format(ptr, len, filename_ptr, filename_len)` - Format code
- `wasm_get_result_ptr()` / `wasm_get_result_len()` - Get formatted result
- `wasm_free_result()` - Free result memory

## Supported File Types

- C (`.c`, `.h`)
- C++ (`.cpp`, `.hpp`, `.cc`, `.cxx`, `.hxx`, `.c++`, `.h++`)
- Java (`.java`)
- C# (`.cs`)
- Objective-C (`.m`, `.mm`)
- Protobuf (`.proto`)
