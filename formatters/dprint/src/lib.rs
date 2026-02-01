// dprint-formatter - Dprint plugin formatting library
//
// Provides formatting API for Markdown, YAML, CSS, SCSS, LESS, and SASS
// using dprint plugins and Malva.

#![allow(clippy::all)]

use fama_common::{FileType, FormatConfig, IndentStyle, LineEnding, QuoteStyle};

/// Format Markdown source code with specified options
pub fn format_markdown(
	source: &str,
	_file_path: &str,
) -> Result<String, String> {
	use dprint_core::configuration::NewLineKind;
	use dprint_plugin_markdown::configuration::*;

	let fmt_config = FormatConfig::default();
	let new_line_kind = match fmt_config.line_ending {
		LineEnding::Lf => NewLineKind::LineFeed,
		LineEnding::Crlf => NewLineKind::CarriageReturnLineFeed,
	};

	let config = Configuration {
		line_width: fmt_config.line_width as u32,
		new_line_kind,
		text_wrap: TextWrap::Maintain,
		emphasis_kind: EmphasisKind::Underscores,
		strong_kind: StrongKind::Asterisks,
		unordered_list_kind: UnorderedListKind::Dashes,
		ignore_directive: "dprint-ignore".to_string(),
		ignore_file_directive: "dprint-ignore-file".to_string(),
		ignore_start_directive: "dprint-ignore-start".to_string(),
		ignore_end_directive: "dprint-ignore-end".to_string(),
	};

	// Create a closure that returns Ok(None) to not format code blocks
	let format_code_block =
		|_file_path: &str,
		 _code: &str,
		 _line_width: u32|
		 -> Result<Option<String>, anyhow::Error> { Ok(None) };

	match dprint_plugin_markdown::format_text(
		source,
		&config,
		format_code_block,
	) {
		Ok(Some(result)) => Ok(result),
		Ok(None) => {
			// No changes needed, return original content
			Ok(source.to_string())
		}
		Err(e) => Err(format!("Markdown formatting error: {}", e)),
	}
}

/// Format YAML source code with specified options
pub fn format_yaml(source: &str, _file_path: &str) -> Result<String, String> {
	use pretty_yaml::config::{
		FormatOptions, LanguageOptions, LayoutOptions, LineBreak,
	};

	let fmt_config = FormatConfig::default();
	let line_break = match fmt_config.line_ending {
		LineEnding::Lf => LineBreak::Lf,
		LineEnding::Crlf => LineBreak::Crlf,
	};

	let config = FormatOptions {
		layout: LayoutOptions {
			print_width: fmt_config.line_width as usize,
			indent_width: fmt_config.indent_width as usize,
			line_break,
		},
		language: LanguageOptions::default(),
	};

	pretty_yaml::format_text(source, &config)
		.map_err(|e| format!("YAML formatting error: {}", e))
}

/// Create Malva options from format config
fn malva_options() -> malva::config::FormatOptions {
	use malva::config::{LanguageOptions, LayoutOptions, Quotes};

	let fmt_config = FormatConfig::default();

	let line_break = match fmt_config.line_ending {
		LineEnding::Lf => malva::config::LineBreak::Lf,
		LineEnding::Crlf => malva::config::LineBreak::Crlf,
	};

	let quotes = match fmt_config.quote_style {
		QuoteStyle::Single => Quotes::AlwaysSingle,
		QuoteStyle::Double => Quotes::AlwaysDouble,
	};

	malva::config::FormatOptions {
		layout: LayoutOptions {
			print_width: fmt_config.line_width as usize,
			use_tabs: matches!(fmt_config.indent_style, IndentStyle::Tabs),
			indent_width: fmt_config.indent_width as usize,
			line_break,
		},
		language: LanguageOptions {
			quotes,
			..Default::default()
		},
	}
}

/// Format CSS source code using Malva formatter
pub fn format_css(source: &str, _file_path: &str) -> Result<String, String> {
	use malva::{format_text, Syntax};
	format_text(source, Syntax::Css, &malva_options())
		.map_err(|e| format!("CSS formatting error: {}", e))
}

/// Format SCSS source code using Malva formatter
pub fn format_scss(source: &str, _file_path: &str) -> Result<String, String> {
	use malva::{format_text, Syntax};
	format_text(source, Syntax::Scss, &malva_options())
		.map_err(|e| format!("SCSS formatting error: {}", e))
}

/// Format LESS source code using Malva formatter
pub fn format_less(source: &str, _file_path: &str) -> Result<String, String> {
	use malva::{format_text, Syntax};
	format_text(source, Syntax::Less, &malva_options())
		.map_err(|e| format!("LESS formatting error: {}", e))
}

/// Format SASS source code using Malva formatter
pub fn format_sass(source: &str, _file_path: &str) -> Result<String, String> {
	use malva::{format_text, Syntax};
	format_text(source, Syntax::Sass, &malva_options())
		.map_err(|e| format!("SASS formatting error: {}", e))
}

/// Format a file based on its file type
pub fn format_file(
	source: &str,
	file_path: &str,
	file_type: FileType,
) -> Result<String, String> {
	match file_type {
		FileType::Markdown => format_markdown(source, file_path),
		FileType::Yaml => format_yaml(source, file_path),
		FileType::Css => format_css(source, file_path),
		FileType::Scss => format_scss(source, file_path),
		FileType::Less => format_less(source, file_path),
		FileType::Sass => format_sass(source, file_path),
		_ => Err(format!(
			"File type {:?} is not supported by dprint-formatter",
			file_type
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
		// CSS formatting with Malva should format the code
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
		let result =
			format_file(source, "test.md", FileType::Markdown).unwrap();
		assert!(result.contains("Hello") || result.contains("#"));
	}

	#[test]
	fn test_format_file_with_unsupported_type() {
		let source = "test";
		let result = format_file(source, "test.js", FileType::JavaScript);
		assert!(result.is_err());
	}
}
