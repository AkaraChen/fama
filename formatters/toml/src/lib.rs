// toml-fmt - TOML formatting library using Taplo

use fama_common::CONFIG;

// Module-level constants - pre-converted config values
const TAPLO_COLUMN_WIDTH: usize = CONFIG.line_width as usize;
const TAPLO_CRLF: bool =
	matches!(CONFIG.line_ending, fama_common::LineEnding::Crlf);
// For indent_string, we use a static str to avoid allocation
// Tabs use "\t", Spaces use "    " (4 spaces matching CONFIG.indent_width)
const TAPLO_INDENT_STRING: &str =
	if matches!(CONFIG.indent_style, fama_common::IndentStyle::Tabs) {
		"\t"
	} else {
		"    " // 4 spaces - matches CONFIG.indent_width default
	};

/// Format TOML source code using Taplo formatter
pub fn format_toml(source: &str, _file_path: &str) -> Result<String, String> {
	use taplo::formatter::{format_syntax, Options};
	use taplo::parser::parse;

	let parsed = parse(source);
	if !parsed.errors.is_empty() {
		return Err(parsed
			.errors
			.iter()
			.map(|e| e.message.as_str())
			.collect::<Vec<_>>()
			.join("; "));
	}

	let options = Options {
		column_width: TAPLO_COLUMN_WIDTH,
		indent_string: TAPLO_INDENT_STRING.to_string(),
		crlf: TAPLO_CRLF,
		trailing_newline: true,
		align_entries: false,
		align_comments: true,
		array_trailing_comma: true,
		array_auto_expand: true,
		array_auto_collapse: true,
		compact_arrays: false,
		compact_inline_tables: false,
		indent_tables: false,
		indent_entries: false,
		reorder_keys: false,
		reorder_arrays: false,
		allowed_blank_lines: 1,
		..Default::default()
	};

	Ok(format_syntax(parsed.into_syntax(), options))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_format_toml() {
		let source = "[package]\nname=\"test\"\nversion=\"1.0.0\"";
		let result = format_toml(source, "test.toml").unwrap();
		assert!(result.contains("[package]"));
		assert!(result.contains("name"));
		assert!(result.contains("version"));
	}

	#[test]
	fn test_format_toml_with_array() {
		let source =
			"[dependencies]\nserde = {version=\"1.0\",features=[\"derive\"]}";
		let result = format_toml(source, "Cargo.toml").unwrap();
		assert!(result.contains("[dependencies]"));
		assert!(result.contains("serde"));
	}

	#[test]
	fn test_format_toml_trailing_newline() {
		let source = "[package]\nname = \"test\"";
		let result = format_toml(source, "test.toml").unwrap();
		assert!(result.ends_with('\n'));
	}

	#[test]
	fn test_format_toml_invalid_syntax() {
		let source = "[package\nname = ";
		let result = format_toml(source, "test.toml");
		assert!(result.is_err());
	}
}
