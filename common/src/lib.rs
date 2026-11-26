// fama-common - Shared types and utilities for fama formatters
//
// Provides the unified Formatter trait, format configuration, and shared enums
// used across all formatter crates.
//
// Configuration follows go-fmt style defaults:
// - Tabs for indentation
// - 80 character line width
// - LF line endings

use std::path::{Path, PathBuf};

/// Indent style for formatting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IndentStyle {
	Spaces,
	#[default]
	Tabs,
}

/// Line ending style
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineEnding {
	#[default]
	Lf,
	Crlf,
}

/// Quote style for strings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuoteStyle {
	Single,
	#[default]
	Double,
}

/// Trailing comma style (JS/TS/Python)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrailingComma {
	/// Add trailing commas everywhere (default)
	#[default]
	All,
	/// Never add trailing commas
	None,
}

/// Semicolon style (JS/TS)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Semicolons {
	/// Always add semicolons (default)
	#[default]
	Always,
	/// Only add semicolons when necessary (ASI)
	AsNeeded,
}

/// Brace style for blocks (SameLine/NewLine)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BraceStyle {
	/// Same line style (K&R) - braces on same line as control statement
	#[default]
	SameLine,
	/// New line style (Allman) - braces on new line
	NewLine,
}

/// Centralized format configuration
///
/// All formatters should use this config to ensure consistent formatting
/// across the codebase. Defaults follow go-fmt style:
/// - Tabs for indentation
/// - 80 character line width
/// - LF line endings
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatConfig {
	// === Core options (all formatters) ===
	/// Indent style: tabs or spaces (default: Tabs)
	pub indent_style: IndentStyle,
	/// Indent width when using spaces (default: 4)
	pub indent_width: u8,
	/// Maximum line width (default: 80)
	pub line_width: u16,
	/// Line ending style (default: Lf)
	pub line_ending: LineEnding,

	// === String options (JS/TS/CSS/Lua) ===
	/// Quote style for strings (default: Double)
	pub quote_style: QuoteStyle,

	// === JS/TS options (Biome) ===
	/// Trailing comma style (default: All)
	pub trailing_comma: TrailingComma,
	/// Semicolon style (default: Always)
	pub semicolons: Semicolons,
	/// Spaces inside brackets in objects (default: true)
	pub bracket_spacing: bool,

	// === Brace style (CSS, C-family) ===
	/// Brace style for blocks (default: SameLine)
	pub brace_style: BraceStyle,
}

/// Global format configuration constant
///
/// This constant is used by all formatters to ensure consistent formatting.
/// Using a const allows compile-time optimization and is inherently thread-safe.
pub const CONFIG: FormatConfig = FormatConfig {
	// Core - go-fmt style
	indent_style: IndentStyle::Tabs,
	indent_width: 4,
	line_width: 80,
	line_ending: LineEnding::Lf,
	// Strings
	quote_style: QuoteStyle::Double,
	// JS/TS
	trailing_comma: TrailingComma::All,
	semicolons: Semicolons::Always,
	bracket_spacing: true,
	// Brace style
	brace_style: BraceStyle::SameLine,
};

impl Default for FormatConfig {
	fn default() -> Self {
		CONFIG
	}
}

/// Tool-specific config file emitted by `fama --export` when a project marker exists.
#[derive(Clone, Copy)]
pub struct ExportedConfig {
	/// Project marker filename whose presence triggers export (e.g. `"Cargo.toml"`).
	pub marker_file: &'static str,
	/// Output config filename to write (e.g. `"rustfmt.toml"`).
	pub output_file: &'static str,
	/// Render function producing the config file contents.
	pub render: fn(&FormatConfig) -> String,
}

/// Unified interface implemented by every language formatter in fama.
pub trait Formatter: Sync {
	/// Human-readable formatter name (e.g. `"biome"`, `"rust"`, `"swift"`).
	fn name(&self) -> &'static str;

	/// File extensions handled by this formatter (without leading dot, exact match as in `Path::extension`).
	fn extensions(&self) -> &'static [&'static str];

	/// Match extensionless or special filenames (e.g. `"Dockerfile"`, `"CMakeLists.txt"`, `"Gemfile"`).
	fn matches_filename(&self, _name: &str) -> bool {
		false
	}

	/// Format a single file's content.
	fn format(&self, content: &str, path: &Path) -> Result<String, String>;

	/// Whether this formatter benefits from formatting multiple files per invocation.
	fn supports_batch(&self) -> bool {
		false
	}

	/// Format a batch of files in one invocation (only called when `supports_batch()` is true).
	fn format_batch(&self, contents: &[&str]) -> Vec<Result<String, String>> {
		contents
			.iter()
			.map(|c| self.format(c, Path::new("")))
			.collect()
	}

	/// Optional project-level config file to emit on `fama --export`.
	fn exported_config(&self) -> Option<ExportedConfig> {
		None
	}

	/// Optional precompilation step for WASM-backed formatters (`fama --precompile <DIR>`).
	fn precompile(&self, _output_dir: &Path) -> Option<Result<PathBuf, String>> {
		None
	}
}

/// Render EditorConfig contents from the shared format configuration.
pub fn editorconfig_contents(config: &FormatConfig) -> String {
	let indent_style = match config.indent_style {
		IndentStyle::Tabs => "tab",
		IndentStyle::Spaces => "space",
	};

	let end_of_line = match config.line_ending {
		LineEnding::Lf => "lf",
		LineEnding::Crlf => "crlf",
	};

	let quote_type = match config.quote_style {
		QuoteStyle::Single => "single",
		QuoteStyle::Double => "double",
	};

	format!(
		r#"# EditorConfig - generated by fama (go-fmt style)
# https://editorconfig.org

root = true

[*]
charset = utf-8
end_of_line = {end_of_line}
insert_final_newline = true
trim_trailing_whitespace = true
indent_style = {indent_style}
indent_size = {indent_size}
tab_width = {indent_size}
max_line_length = {line_width}

# JavaScript/TypeScript
[*.{{js,jsx,ts,tsx,mjs,cjs}}]
quote_type = {quote_type}

# Shell scripts
[*.{{sh,bash,zsh}}]
indent_style = {indent_style}
indent_size = {indent_size}
"#,
		indent_size = config.indent_width,
		line_width = config.line_width
	)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_format_config_default() {
		let config = FormatConfig::default();
		// Core options - go-fmt style
		assert_eq!(config.indent_style, IndentStyle::Tabs);
		assert_eq!(config.indent_width, 4);
		assert_eq!(config.line_width, 80);
		assert_eq!(config.line_ending, LineEnding::Lf);
		// String options
		assert_eq!(config.quote_style, QuoteStyle::Double);
		// JS/TS options
		assert_eq!(config.trailing_comma, TrailingComma::All);
		assert_eq!(config.semicolons, Semicolons::Always);
		assert!(config.bracket_spacing);
	}

	#[test]
	fn test_editorconfig_contents() {
		let content = editorconfig_contents(&FormatConfig::default());

		assert!(content.contains("indent_style = tab"));
		assert!(content.contains("indent_size = 4"));
		assert!(content.contains("max_line_length = 80"));
		assert!(content.contains("quote_type = double"));
	}
}
