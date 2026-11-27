// dprint-formatter - Markdown, YAML, CSS, SCSS, LESS, and SASS formatting

use dprint_core::configuration::NewLineKind;
use fama_common::{Formatter, CONFIG};
use std::path::Path;
use std::sync::OnceLock;

/// Formatter implementation for Markdown, YAML, and CSS-family files via dprint/Malva.
pub struct DprintFormatter;

impl Formatter for DprintFormatter {
	fn name(&self) -> &'static str {
		"dprint"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["yaml", "yml", "md", "markdown", "css", "scss", "less", "sass"]
	}

	fn format(&self, content: &str, path: &Path) -> Result<String, String> {
		format_file(content, path)
	}
}

const DPRINT_LINE_WIDTH: u16 = CONFIG.line_width;
const DPRINT_INDENT_WIDTH: u8 = CONFIG.indent_width;
const DPRINT_NEW_LINE_KIND: NewLineKind = match CONFIG.line_ending {
	fama_common::LineEnding::Lf => NewLineKind::LineFeed,
	fama_common::LineEnding::Crlf => NewLineKind::CarriageReturnLineFeed,
};
const DPRINT_USE_TABS: bool =
	matches!(CONFIG.indent_style, fama_common::IndentStyle::Tabs);

const MALVA_LINE_BREAK: malva::config::LineBreak = match CONFIG.line_ending {
	fama_common::LineEnding::Lf => malva::config::LineBreak::Lf,
	fama_common::LineEnding::Crlf => malva::config::LineBreak::Crlf,
};
const MALVA_QUOTES: malva::config::Quotes = match CONFIG.quote_style {
	fama_common::QuoteStyle::Single => malva::config::Quotes::AlwaysSingle,
	fama_common::QuoteStyle::Double => malva::config::Quotes::AlwaysDouble,
};
const MALVA_TRAILING_COMMA: bool =
	matches!(CONFIG.trailing_comma, fama_common::TrailingComma::All);

const YAML_LINE_BREAK: pretty_yaml::config::LineBreak = match CONFIG.line_ending
{
	fama_common::LineEnding::Lf => pretty_yaml::config::LineBreak::Lf,
	fama_common::LineEnding::Crlf => pretty_yaml::config::LineBreak::Crlf,
};

fn markdown_config(
) -> &'static dprint_plugin_markdown::configuration::Configuration {
	use dprint_plugin_markdown::configuration::*;
	static CFG: OnceLock<Configuration> = OnceLock::new();
	CFG.get_or_init(|| Configuration {
		line_width: DPRINT_LINE_WIDTH as u32,
		new_line_kind: DPRINT_NEW_LINE_KIND,
		text_wrap: TextWrap::Maintain,
		emphasis_kind: EmphasisKind::Underscores,
		strong_kind: StrongKind::Asterisks,
		unordered_list_kind: UnorderedListKind::Dashes,
		ignore_directive: "dprint-ignore".to_string(),
		ignore_file_directive: "dprint-ignore-file".to_string(),
		ignore_start_directive: "dprint-ignore-start".to_string(),
		ignore_end_directive: "dprint-ignore-end".to_string(),
	})
}

pub fn format_markdown(
	source: &str,
	_file_path: &str,
) -> Result<String, String> {
	let config = markdown_config();

	let format_code_block =
		|_file_path: &str,
		 _code: &str,
		 _line_width: u32|
		 -> Result<Option<String>, anyhow::Error> { Ok(None) };

	match dprint_plugin_markdown::format_text(
		source,
		config,
		format_code_block,
	) {
		Ok(Some(result)) => Ok(result),
		Ok(None) => Ok(source.to_string()),
		Err(e) => Err(format!("Markdown formatting error: {}", e)),
	}
}

fn yaml_config() -> &'static pretty_yaml::config::FormatOptions {
	use pretty_yaml::config::{FormatOptions, LanguageOptions, LayoutOptions};
	static CFG: OnceLock<FormatOptions> = OnceLock::new();
	CFG.get_or_init(|| FormatOptions {
		layout: LayoutOptions {
			print_width: DPRINT_LINE_WIDTH as usize,
			indent_width: DPRINT_INDENT_WIDTH as usize,
			line_break: YAML_LINE_BREAK,
		},
		language: LanguageOptions::default(),
	})
}

pub fn format_yaml(source: &str, _file_path: &str) -> Result<String, String> {
	pretty_yaml::format_text(source, yaml_config())
		.map_err(|e| format!("YAML formatting error: {}", e))
}

fn malva_options() -> &'static malva::config::FormatOptions {
	use malva::config::{FormatOptions, LanguageOptions, LayoutOptions};
	static CFG: OnceLock<FormatOptions> = OnceLock::new();
	CFG.get_or_init(|| FormatOptions {
		layout: LayoutOptions {
			print_width: DPRINT_LINE_WIDTH as usize,
			use_tabs: DPRINT_USE_TABS,
			indent_width: DPRINT_INDENT_WIDTH as usize,
			line_break: MALVA_LINE_BREAK,
		},
		language: LanguageOptions {
			quotes: MALVA_QUOTES,
			trailing_comma: MALVA_TRAILING_COMMA,
			..Default::default()
		},
	})
}

pub fn format_css(source: &str, _file_path: &str) -> Result<String, String> {
	use malva::{format_text, Syntax};
	format_text(source, Syntax::Css, malva_options())
		.map_err(|e| format!("CSS formatting error: {}", e))
}

pub fn format_scss(source: &str, _file_path: &str) -> Result<String, String> {
	use malva::{format_text, Syntax};
	format_text(source, Syntax::Scss, malva_options())
		.map_err(|e| format!("SCSS formatting error: {}", e))
}

pub fn format_less(source: &str, _file_path: &str) -> Result<String, String> {
	use malva::{format_text, Syntax};
	format_text(source, Syntax::Less, malva_options())
		.map_err(|e| format!("LESS formatting error: {}", e))
}

pub fn format_sass(source: &str, _file_path: &str) -> Result<String, String> {
	use malva::{format_text, Syntax};
	format_text(source, Syntax::Sass, malva_options())
		.map_err(|e| format!("SASS formatting error: {}", e))
}

pub fn format_file(source: &str, path: &Path) -> Result<String, String> {
	let file_path = path.to_str().unwrap_or("");
	match path.extension().and_then(|e| e.to_str()) {
		Some("md" | "markdown") => format_markdown(source, file_path),
		Some("yaml" | "yml") => format_yaml(source, file_path),
		Some("css") => format_css(source, file_path),
		Some("scss") => format_scss(source, file_path),
		Some("less") => format_less(source, file_path),
		Some("sass") => format_sass(source, file_path),
		_ => Err(format!(
			"File {} is not supported by dprint",
			path.display()
		)),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_format_markdown() {
		let source = "# Hello World";
		let result = format_markdown(source, "test.md").unwrap();
		assert!(result.contains("Hello") || result.contains("#"));
	}

	#[test]
	fn test_format_yaml() {
		let source = "name: test\nage: 30";
		let result = format_yaml(source, "test.yaml").unwrap();
		assert!(result.contains("name") || result.contains("age"));
	}

	#[test]
	fn test_format_css() {
		let source = "body{margin:0;padding:0;}";
		let result = format_css(source, "test.css").unwrap();
		assert!(result.contains("margin") && result.contains("padding"));
	}

	#[test]
	fn test_format_scss() {
		let source = ".foo{margin:0;}";
		let result = format_scss(source, "test.scss").unwrap();
		assert!(result.contains("margin"));
	}

	#[test]
	fn test_format_less() {
		let source = ".foo{margin:0;}";
		let result = format_less(source, "test.less").unwrap();
		assert!(result.contains("margin"));
	}

	#[test]
	fn test_format_sass() {
		let source = ".foo\n  margin: 0";
		let result = format_sass(source, "test.sass").unwrap();
		assert!(result.contains("margin"));
	}

	#[test]
	fn test_format_file_with_markdown() {
		let source = "# Hello World";
		let result = format_file(source, Path::new("test.md")).unwrap();
		assert!(result.contains("Hello") || result.contains("#"));
	}

	#[test]
	fn test_format_file_with_unsupported_type() {
		let source = "test";
		let result = format_file(source, Path::new("test.js"));
		assert!(result.is_err());
	}
}
