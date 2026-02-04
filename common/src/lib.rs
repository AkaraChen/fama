// fama-common - Shared types and utilities for fama formatters
//
// Provides common file type detection, format configuration, and shared
// enums used across all formatter crates.
//
// Configuration follows go-fmt style defaults:
// - Tabs for indentation
// - 80 character line width
// - LF line endings

use std::path::Path;

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
};

impl Default for FormatConfig {
	fn default() -> Self {
		CONFIG
	}
}

/// File type enum for language detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
	JavaScript,
	TypeScript,
	Jsx,
	Tsx,
	Json,
	Jsonc,
	Css,
	Scss,
	Less,
	Sass,
	Html,
	Vue,
	Svelte,
	Astro,
	Yaml,
	Toml,
	Markdown,
	Rust,
	Python,
	Lua,
	Ruby,
	Shell,
	Go,
	Zig,
	Hcl,
	Dockerfile,
	GraphQL,
	Sql,
	Xml,
	Dart,
	Php,
	// C-family languages (clang-format)
	C,
	Cpp,
	CSharp,
	ObjectiveC,
	Java,
	Protobuf,
	Unknown,
}

/// Detect file type from extension
pub fn detect_file_type(path: &str) -> FileType {
	let path = Path::new(path);
	match path.extension().and_then(|ext| ext.to_str()) {
		Some("js") | Some("cjs") | Some("mjs") => FileType::JavaScript,
		Some("ts") | Some("mts") => FileType::TypeScript,
		Some("jsx") | Some("mjsx") => FileType::Jsx,
		Some("tsx") => FileType::Tsx,
		Some("json") => FileType::Json,
		Some("jsonc") => FileType::Jsonc,
		Some("css") => FileType::Css,
		Some("scss") => FileType::Scss,
		Some("less") => FileType::Less,
		Some("sass") => FileType::Sass,
		Some("html") | Some("htm") => FileType::Html,
		Some("vue") => FileType::Vue,
		Some("svelte") => FileType::Svelte,
		Some("astro") => FileType::Astro,
		Some("yaml") | Some("yml") => FileType::Yaml,
		Some("toml") => FileType::Toml,
		Some("md") | Some("markdown") => FileType::Markdown,
		Some("rs") => FileType::Rust,
		Some("py") => FileType::Python,
		Some("lua") => FileType::Lua,
		Some("rb") | Some("rake") | Some("gemspec") | Some("ru") => {
			FileType::Ruby
		}
		Some("sh") | Some("bash") | Some("zsh") => FileType::Shell,
		Some("go") => FileType::Go,
		Some("zig") => FileType::Zig,
		Some("hcl") | Some("tf") | Some("tfvars") => FileType::Hcl,
		Some("graphql") | Some("gql") => FileType::GraphQL,
		Some("sql") => FileType::Sql,
		Some("xml") => FileType::Xml,
		Some("dart") => FileType::Dart,
		Some("php") | Some("phtml") => FileType::Php,
		// C-family languages
		Some("c") | Some("h") => FileType::C,
		Some("cpp") | Some("cc") | Some("cxx") | Some("hpp") | Some("hxx")
		| Some("hh") => FileType::Cpp,
		Some("cs") => FileType::CSharp,
		Some("m") | Some("mm") => FileType::ObjectiveC,
		Some("java") => FileType::Java,
		Some("proto") => FileType::Protobuf,
		_ => {
			// Check for special filenames
			if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
				// Dockerfile
				if name == "Dockerfile" || name.starts_with("Dockerfile.") {
					return FileType::Dockerfile;
				}
				// Ruby files without extensions
				if matches!(
					name,
					"Rakefile"
						| "Gemfile" | "Guardfile"
						| "Vagrantfile" | "Berksfile"
						| "Capfile" | "Thorfile"
						| "Fastfile" | "Appfile"
						| "Matchfile" | "Snapfile"
						| "Deliverfile" | "Scanfile"
						| "Gymfile"
				) {
					return FileType::Ruby;
				}
			}
			FileType::Unknown
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_detect_javascript() {
		assert_eq!(detect_file_type("test.js"), FileType::JavaScript);
		assert_eq!(detect_file_type("test.cjs"), FileType::JavaScript);
		assert_eq!(detect_file_type("test.mjs"), FileType::JavaScript);
	}

	#[test]
	fn test_detect_typescript() {
		assert_eq!(detect_file_type("test.ts"), FileType::TypeScript);
		assert_eq!(detect_file_type("test.mts"), FileType::TypeScript);
	}

	#[test]
	fn test_detect_jsx() {
		assert_eq!(detect_file_type("test.jsx"), FileType::Jsx);
		assert_eq!(detect_file_type("test.mjsx"), FileType::Jsx);
	}

	#[test]
	fn test_detect_tsx() {
		assert_eq!(detect_file_type("test.tsx"), FileType::Tsx);
	}

	#[test]
	fn test_detect_json() {
		assert_eq!(detect_file_type("test.json"), FileType::Json);
		assert_eq!(detect_file_type("package.json"), FileType::Json);
		assert_eq!(detect_file_type("tsconfig.jsonc"), FileType::Jsonc);
	}

	#[test]
	fn test_detect_css_variants() {
		assert_eq!(detect_file_type("test.css"), FileType::Css);
		assert_eq!(detect_file_type("test.scss"), FileType::Scss);
		assert_eq!(detect_file_type("test.less"), FileType::Less);
		assert_eq!(detect_file_type("test.sass"), FileType::Sass);
	}

	#[test]
	fn test_detect_html_variants() {
		assert_eq!(detect_file_type("test.html"), FileType::Html);
		assert_eq!(detect_file_type("test.htm"), FileType::Html);
		assert_eq!(detect_file_type("test.vue"), FileType::Vue);
		assert_eq!(detect_file_type("test.svelte"), FileType::Svelte);
		assert_eq!(detect_file_type("test.astro"), FileType::Astro);
	}

	#[test]
	fn test_detect_yaml() {
		assert_eq!(detect_file_type("test.yaml"), FileType::Yaml);
		assert_eq!(detect_file_type("test.yml"), FileType::Yaml);
	}

	#[test]
	fn test_detect_toml() {
		assert_eq!(detect_file_type("test.toml"), FileType::Toml);
		assert_eq!(detect_file_type("Cargo.toml"), FileType::Toml);
		assert_eq!(detect_file_type("path/to/config.toml"), FileType::Toml);
	}

	#[test]
	fn test_detect_markdown() {
		assert_eq!(detect_file_type("test.md"), FileType::Markdown);
		assert_eq!(detect_file_type("test.markdown"), FileType::Markdown);
	}

	#[test]
	fn test_detect_rust() {
		assert_eq!(detect_file_type("test.rs"), FileType::Rust);
	}

	#[test]
	fn test_detect_python() {
		assert_eq!(detect_file_type("test.py"), FileType::Python);
	}

	#[test]
	fn test_detect_lua() {
		assert_eq!(detect_file_type("test.lua"), FileType::Lua);
	}

	#[test]
	fn test_detect_shell() {
		assert_eq!(detect_file_type("test.sh"), FileType::Shell);
		assert_eq!(detect_file_type("test.bash"), FileType::Shell);
		assert_eq!(detect_file_type("test.zsh"), FileType::Shell);
	}

	#[test]
	fn test_detect_go() {
		assert_eq!(detect_file_type("test.go"), FileType::Go);
		assert_eq!(detect_file_type("main.go"), FileType::Go);
		assert_eq!(detect_file_type("path/to/file.go"), FileType::Go);
	}

	#[test]
	fn test_detect_zig() {
		assert_eq!(detect_file_type("test.zig"), FileType::Zig);
		assert_eq!(detect_file_type("main.zig"), FileType::Zig);
		assert_eq!(detect_file_type("path/to/file.zig"), FileType::Zig);
	}

	#[test]
	fn test_detect_hcl() {
		assert_eq!(detect_file_type("test.hcl"), FileType::Hcl);
		assert_eq!(detect_file_type("main.tf"), FileType::Hcl);
		assert_eq!(detect_file_type("variables.tfvars"), FileType::Hcl);
		assert_eq!(detect_file_type("path/to/config.hcl"), FileType::Hcl);
	}

	#[test]
	fn test_detect_dockerfile() {
		assert_eq!(detect_file_type("Dockerfile"), FileType::Dockerfile);
		assert_eq!(detect_file_type("Dockerfile.dev"), FileType::Dockerfile);
		assert_eq!(detect_file_type("Dockerfile.prod"), FileType::Dockerfile);
		assert_eq!(
			detect_file_type("path/to/Dockerfile"),
			FileType::Dockerfile
		);
		assert_eq!(
			detect_file_type("path/to/Dockerfile.test"),
			FileType::Dockerfile
		);
	}

	#[test]
	fn test_detect_sql() {
		assert_eq!(detect_file_type("test.sql"), FileType::Sql);
		assert_eq!(detect_file_type("query.sql"), FileType::Sql);
		assert_eq!(detect_file_type("path/to/schema.sql"), FileType::Sql);
	}

	#[test]
	fn test_detect_dart() {
		assert_eq!(detect_file_type("test.dart"), FileType::Dart);
		assert_eq!(detect_file_type("main.dart"), FileType::Dart);
		assert_eq!(detect_file_type("lib/my_app.dart"), FileType::Dart);
	}

	#[test]
	fn test_detect_php() {
		assert_eq!(detect_file_type("test.php"), FileType::Php);
		assert_eq!(detect_file_type("index.php"), FileType::Php);
		assert_eq!(detect_file_type("template.phtml"), FileType::Php);
		assert_eq!(detect_file_type("path/to/file.php"), FileType::Php);
	}

	#[test]
	fn test_detect_unknown() {
		assert_eq!(detect_file_type("unknown.xyz"), FileType::Unknown);
		assert_eq!(detect_file_type("test.unknown"), FileType::Unknown);
	}

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
}
