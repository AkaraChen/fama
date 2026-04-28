# Fama

中文 | [English](./README.md)

**一条命令。所有语言。零配置。**

## 安装

**macOS / Linux**

```bash
curl -fsSL https://raw.githubusercontent.com/AkaraChen/fama/master/install.sh | sh
```

**Windows**

从 [Releases](https://github.com/AkaraChen/fama/releases) 下载，解压 `fama.exe`，并添加到 PATH。

## 理念

Fama 的存在是因为代码格式化应该是无感知的。

你不应该花时间配置格式化工具。你不应该争论 Tab 还是空格。你不应该为不同语言维护不同的设置。你不应该在多台机器之间同步 IDE 配置。

你应该写代码，运行 `fama`，然后继续工作。

## 它能做什么

```
fama
```

就这样。在任何项目中运行它。它会格式化它理解的所有内容，并保持其他内容不变。

## 为什么存在

每个格式化工具都有自己的配置文件。每个 IDE 都有自己的设置。每个团队都有自己的风格指南。每个新项目都需要设置。

这是浪费。

Fama 帮你做决定，所以你不必做。它选择合理的默认值并普遍应用。不需要 `.pretierrc`。不需要 `settings.json`。不需要争论分号。

## 原则

**约定优于配置。** 只有一种风格。它有效。使用它。

**通用。** 一个工具格式化 30+ 种语言：JavaScript、TypeScript、JSX、TSX、JSON、JSONC、CSS、SCSS、Less、Sass、HTML、Vue、Svelte、Astro、GraphQL、YAML、TOML、Markdown、Rust、Python、Lua、Ruby、PHP、Shell、Go、Zig、HCL、Dockerfile、SQL、XML、Kotlin、C、C++、C#、Objective-C、Java 和 Protobuf。到处都是相同的命令。

**快速。** 格式化永远不应该是你等待的东西。

**小巧。** 一个 13 MB 的二进制文件承载内置 formatter。process mode 的语言也可以依赖宿主机已安装的 formatter CLI，例如 Kotlin 使用 `ktfmt`。放在你的 PATH 里就能工作。随时可以通过再次运行安装脚本更新。

**安静。** 它格式化改变的内容并告诉你它做了什么。仅此而已。

## 使用方法

```bash
# 格式化所有内容
fama

# 格式化特定文件
fama "src/**/*.ts"

# 为需要的工具导出设置
fama --export
```

## Fama 风格

- Tab 用于缩进
- 80 字符行宽
- 双引号
- 尾随逗号
- 分号

这是不可协商的。这就是重点。

## 配置选项

Fama 使用适用于所有格式化工具的统一配置。以下是支持的完整选项列表以及哪些语言支持它们：

### 核心选项

| 选项           | 默认值  | 描述                             |
| -------------- | ------- | -------------------------------- |
| `indent_style` | `Tabs`  | 缩进风格：`Tabs` 或 `Spaces`     |
| `indent_width` | `4`     | 每个缩进级别的空格数（使用空格时）|
| `line_width`   | `80`    | 最大行长度                       |
| `line_ending`  | `Lf`    | 换行符：`Lf` 或 `Crlf`           |

### 语言特定选项

| 选项             | 默认值     | 描述                                         | 语言                                                 |
| ---------------- | ---------- | -------------------------------------------- | ---------------------------------------------------- |
| `quote_style`    | `Double`   | 引号偏好：`Single` 或 `Double`               | JavaScript, TypeScript, Python, Lua, CSS, SCSS, PHP |
| `trailing_comma` | `All`      | 尾随逗号风格：`All` 或 `None`                | JavaScript, TypeScript, JSON, PHP                   |
| `semicolons`     | `Always`   | 分号使用：`Always` 或 `AsNeeded`             | JavaScript, TypeScript                              |
| `bracket_spacing`| `true`     | 对象括号内的空格                             | JavaScript, TypeScript                              |
| `brace_style`    | `SameLine` | 大括号风格：`SameLine` (K&R) 或 `NewLine` (Allman) | CSS, SCSS, C 系列                              |

### 语言支持矩阵

| 语言            | 格式化工具   | 核心选项 | 引号 | 尾随逗号 | 分号 | 括号间距 | 备注                             |
| --------------- | ------------ | -------- | ---- | -------- | ---- | -------- | --------------------------------- |
| **JavaScript**  | Biome        | ✅       | ✅   | ✅       | ✅   | ✅       | 包括 JSX                         |
| **TypeScript**  | Biome        | ✅       | ✅   | ✅       | ✅   | ✅       | 包括 TSX                          |
| **JSON**        | Biome        | ✅       | ❌   | ✅       | N/A  | N/A      | 尾随逗号 = All/None              |
| **JSONC**       | Biome        | ✅       | ❌   | ❌       | N/A  | N/A      | 允许注释                          |
| **HTML**        | Biome        | ✅       | N/A  | N/A      | N/A  | N/A      | 包括 Vue/Svelte/Astro            |
| **GraphQL**     | Biome        | ✅       | N/A  | N/A      | N/A  | N/A      |                                   |
| **CSS**         | dprint/Malva | ✅       | ✅   | ✅       | N/A  | N/A      | 包括 SCSS, LESS, Sass            |
| **Markdown**    | dprint       | ✅*      | N/A  | N/A      | N/A  | N/A      | *仅 line_width, line_ending      |
| **YAML**        | dprint       | ✅       | N/A  | N/A      | N/A  | N/A      |                                   |
| **Dockerfile**  | dprint       | ✅       | N/A  | N/A      | N/A  | N/A      |                                   |
| **TOML**        | Taplo        | ✅       | N/A  | N/A      | N/A  | N/A      | 使用 CONFIG.indent_width          |
| **Rust**        | rustfmt      | ✅       | N/A  | N/A      | N/A  | N/A      | 使用 rustfmt 配置环境变量        |
| **Python**      | Ruff         | ✅       | ✅   | N/A      | N/A  | N/A      |                                   |
| **Lua**         | StyLua       | ✅       | ✅   | N/A      | N/A  | N/A      |                                   |
| **PHP**         | Mago         | ✅       | ✅   | ✅       | ✅   | N/A      |                                   |
| **Ruby**        | rubyfmt      | ❌       | ❌   | ❌       | ❌   | ❌       | 嵌入式 Ruby，无配置              |
| **Shell**       | goffi        | ✅*      | N/A  | N/A      | N/A  | N/A      | *仅 indent_style, indent_width   |
| **Go**          | goffi        | ❌       | ❌   | ❌       | ❌   | ❌       | 使用 gofmt 默认值（tabs）        |
| **HCL**         | goffi        | ❌       | ❌   | ❌       | ❌   | ❌       | 使用 hclwrite 默认值（2 空格）   |
| **Zig**         | zigffi       | ❌       | ❌   | ❌       | ❌   | ❌       | 使用 Zig 默认值                  |
| **SQL**         | sqruff       | ✅       | N/A  | N/A      | N/A  | N/A      | 关键字大写                        |
| **XML**         | quick-xml    | ✅       | N/A  | N/A      | N/A  | N/A      |                                   |
| **Kotlin**      | ktfmt（process） | ✅*  | N/A  | N/A      | N/A  | N/A      | *在支持的范围内读取生成的 `.editorconfig`；要求 PATH 中可用 `ktfmt` |
| **C/C++**       | clang-format | ✅       | N/A  | N/A      | N/A  | N/A      | 通过 WASM                         |
| **C#**          | clang-format | ✅       | N/A  | N/A      | N/A  | N/A      | 通过 WASM                         |
| **Objective-C** | clang-format | ✅       | N/A  | N/A      | N/A  | N/A      | 通过 WASM                         |
| **Java**        | clang-format | ✅       | N/A  | N/A      | N/A  | N/A      | 通过 WASM                         |
| **Protobuf**    | clang-format | ✅       | N/A  | N/A      | N/A  | N/A      | 通过 WASM                         |

### 关于硬编码风格的说明

某些格式化工具使用无法配置的硬编码风格：

- **Go**: 使用 `gofmt` 默认值（tab 用于缩进）
- **HCL**: 使用 `hclwrite` 默认值（2 空格）
- **Kotlin**: 通过 process mode 调用宿主机上的 `ktfmt`
- **Zig**: 使用 Zig 内置格式化工具的默认风格
- **Ruby**: 使用嵌入的 `rubyfmt` 固定风格

### 配置导出

Fama 可以为需要的工具生成配置文件：

```bash
fama --export
```

这会生成：

- `.editorconfig` - 编辑器无关的配置，也会被 `ktfmt` 这类 process-mode formatter 使用
- `rustfmt.toml` - Rust 特定的格式化规则

---

## 为什么叫 "Fama"？

```plaintext
format
   ↓     太长了，程序员很懒
  fmt
   ↓     还是太冷，需要些元音
famata
   ↓     等等，这又太长了
 fama
   ↓
  :)
```
