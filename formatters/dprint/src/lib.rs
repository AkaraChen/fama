// dprint-formatter - Dprint plugin formatting library
//
// Provides formatting API for Markdown, YAML, CSS, SCSS, LESS, and SASS
// using dprint plugins and Malva.

#![allow(clippy::all)]

use dprint_core::configuration::NewLineKind;
use fama_common::{FileType, CONFIG};

// Module-level constants - pre-converted config values
const DPRINT_LINE_WIDTH: u16 = CONFIG.line_width;
const DPRINT_INDENT_WIDTH: u8 = CONFIG.indent_width;
const DPRINT_NEW_LINE_KIND: NewLineKind = match CONFIG.line_ending {
	fama_common::LineEnding::Lf => NewLineKind::LineFeed,
	fama_common::LineEnding::Crlf => NewLineKind::CarriageReturnLineFeed,
};
const DPRINT_USE_TABS: bool =
	matches!(CONFIG.indent_style, fama_common::IndentStyle::Tabs);

// Malva constants
const MALVA_LINE_BREAK: malva::config::LineBreak = match CONFIG.line_ending {
	fama_common::LineEnding::Lf => malva::config::LineBreak::Lf,
	fama_common::LineEnding::Crlf => malva::config::LineBreak::Crlf,
};
const MALVA_QUOTES: malva::config::Quotes = match CONFIG.quote_style {
	fama_common::QuoteStyle::Single => malva::config::Quotes::AlwaysSingle,
	fama_common::QuoteStyle::Double => malva::config::Quotes::AlwaysDouble,
};
const MALVA_TRAILING_COMMA: bool = matches!(CONFIG.trailing_comma, fama_common::TrailingComma::All);

// YAML constants
const YAML_LINE_BREAK: pretty_yaml::config::LineBreak = match CONFIG.line_ending
{
	fama_common::LineEnding::Lf => pretty_yaml::config::LineBreak::Lf,
	fama_common::LineEnding::Crlf => pretty_yaml::config::LineBreak::Crlf,
};

/// Format Markdown source code with specified options
pub fn format_markdown(
	source: &str,
	_file_path: &str,
) -> Result<String, String> {
	use dprint_plugin_markdown::configuration::*;

	let config = Configuration {
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
	};

	// Create a closure that returns Ok(None) to not format code blocks
	let format_code_block =
		|_file_path: &str,
		 _code: &str,
		 _line_width: u32|
		 -> Result<Option<String>, anyhow::Error> { Ok(None) };

	let formatted = match dprint_plugin_markdown::format_text(
		source,
		&config,
		format_code_block,
	) {
		Ok(Some(result)) => result,
		Ok(None) => return Ok(source.to_string()),
		Err(e) => return Err(format!("Markdown formatting error: {}", e)),
	};

	Ok(normalize_table_padding(&formatted))
}

/// Strip excessive column padding from markdown tables
fn normalize_table_padding(source: &str) -> String {
	let lines: Vec<&str> = source.lines().collect();
	let mut result = Vec::with_capacity(lines.len());
	let len = lines.len();

	let mut i = 0;
	while i < len {
		if i + 1 < len && is_table_row(lines[i]) && is_separator_row(lines[i + 1])
		{
			let table_start = i;
			while i < len && (is_table_row(lines[i]) || is_separator_row(lines[i]))
			{
				i += 1;
			}
			let table_lines = &lines[table_start..i];
			normalize_table(table_lines, &mut result);
		} else {
			result.push(lines[i].to_string());
			i += 1;
		}
	}

	let mut output = result.join("\n");
	if source.ends_with('\n') {
		output.push('\n');
	}
	output
}

fn is_table_row(line: &str) -> bool {
	let trimmed = line.trim();
	trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.len() > 1
}

fn is_separator_row(line: &str) -> bool {
	let trimmed = line.trim();
	if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
		return false;
	}
	trimmed[1..trimmed.len() - 1].split('|').all(|cell| {
		let c = cell.trim();
		!c.is_empty()
			&& c.chars()
				.all(|ch| ch == '-' || ch == ':' || ch == ' ')
	})
}

fn normalize_table(table_lines: &[&str], result: &mut Vec<String>) {
	for line in table_lines {
		if is_separator_row(line) {
			let cells: Vec<&str> =
				line.trim()[1..line.trim().len() - 1].split('|').collect();
			let normalized: Vec<String> = cells
				.iter()
				.map(|cell| {
					let c = cell.trim();
					let left_colon = c.starts_with(':');
					let right_colon = c.ends_with(':');
					match (left_colon, right_colon) {
						(true, true) => " :---: ".to_string(),
						(true, false) => " :--- ".to_string(),
						(false, true) => " ---: ".to_string(),
						(false, false) => " --- ".to_string(),
					}
				})
				.collect();
			result.push(format!("|{}|", normalized.join("|")));
		} else {
			let cells: Vec<&str> =
				line.trim()[1..line.trim().len() - 1].split('|').collect();
			let normalized: Vec<String> = cells
				.iter()
				.map(|cell| {
					let trimmed = cell.trim_end();
					if trimmed.is_empty() {
						" ".to_string()
					} else if trimmed.starts_with(' ') {
						format!("{} ", trimmed)
					} else {
						format!(" {} ", trimmed)
					}
				})
				.collect();
			result.push(format!("|{}|", normalized.join("|")));
		}
	}
}

/// Format YAML source code with specified options
pub fn format_yaml(source: &str, _file_path: &str) -> Result<String, String> {
	use pretty_yaml::config::{FormatOptions, LanguageOptions, LayoutOptions};

	let config = FormatOptions {
		layout: LayoutOptions {
			print_width: DPRINT_LINE_WIDTH as usize,
			indent_width: DPRINT_INDENT_WIDTH as usize,
			line_break: YAML_LINE_BREAK,
		},
		language: LanguageOptions::default(),
	};

	pretty_yaml::format_text(source, &config)
		.map_err(|e| format!("YAML formatting error: {}", e))
}

/// Create Malva options from format config
fn malva_options() -> malva::config::FormatOptions {
	use malva::config::{LanguageOptions, LayoutOptions};

	malva::config::FormatOptions {
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

	#[test]
	fn test_normalize_table_strips_excessive_padding() {
		// Simulate what dprint produces: separator padded to match long cell
		let input = "| Name | Link                                                                                                           |\n| :--- | :------------------------------------------------------------------------------------------------------------- |\n| foo  | [very long badge url](https://img.shields.io/badge/Download-x64-0078D6.svg?logo=windows&logoColor=white)       |\n";
		let result = normalize_table_padding(input);
		// Separator should be reduced to minimum dashes
		assert!(result.contains("| :--- | :--- |"));
		// Cell content should be preserved without trailing spaces
		assert!(result.contains("| foo |"));
	}

	#[test]
	fn test_normalize_table_preserves_alignment() {
		let input = "| Left | Center | Right |\n| :----------- | :-----------: | -----------: |\n| a            | b             | c            |\n";
		let result = normalize_table_padding(input);
		assert!(result.contains(":---"));
		assert!(result.contains(":---:"));
		assert!(result.contains("---:"));
		// Should not contain long dashes
		assert!(!result.contains("-------"));
	}

	#[test]
	fn test_normalize_table_preserves_non_table_content() {
		let input = "# Title\n\nSome paragraph text.\n\n- list item\n";
		let result = normalize_table_padding(input);
		assert_eq!(result, input);
	}

	#[test]
	fn test_normalize_table_real_world_badge_table() {
		let long_url = "[![Badge](https://img.shields.io/badge/Download-x64-0078D6.svg?logo=windows)](https://github.com/user/repo/releases/download/v1.0.0/app-1.0.0-windows-x86_64.zip)";
		let padded_sep = "-".repeat(long_url.len());
		let padded_spaces = " ".repeat(long_url.len() - 2);
		let input = format!(
			"| OS{}| Link{}|\n| :{}| :{}|\n| **Win**{}| {} |\n",
			" ".repeat(long_url.len() - 2),
			" ".repeat(long_url.len() - 4),
			padded_sep,
			padded_sep,
			padded_spaces,
			long_url,
		);
		let result = normalize_table_padding(&input);
		// No line should have excessive dashes
		for line in result.lines() {
			if is_separator_row(line) {
				assert!(
					line.len() < 40,
					"separator too long: {} chars",
					line.len()
				);
			}
		}
	}

	#[test]
	fn test_normalize_table_mixed_content() {
		let input = "# Header\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n\nParagraph after table.\n";
		let result = normalize_table_padding(input);
		assert!(result.contains("# Header"));
		assert!(result.contains("Paragraph after table."));
		assert!(result.contains("| A |"));
	}
}
