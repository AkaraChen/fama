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

**Universal.** One tool formats your JavaScript, Python, Rust, Markdown, YAML, Shell scripts, and more. Same command everywhere.

**Fast.** Formatting should never be the thing you're waiting for.

**Small.** A single binary. No runtime dependencies. No package managers. No plugins.

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

## Install

**macOS / Linux**

```bash
curl -fsSL https://raw.githubusercontent.com/AkaraChen/fama/master/install.sh | sh
```

**Windows**

Download from [Releases](https://github.com/AkaraChen/fama/releases), extract `fama.exe`, and add it to your PATH.

---

## Why "Fama"?

```
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
