// toml-fmt - TOML formatting library using Taplo

use fama_common::{FormatConfig, IndentStyle, LineEnding};

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

	let fmt_config = FormatConfig::default();

	let indent_string = match fmt_config.indent_style {
		IndentStyle::Tabs => "\t".to_string(),
		IndentStyle::Spaces => " ".repeat(fmt_config.indent_width as usize),
	};

	let options = Options {
		column_width: fmt_config.line_width as usize,
		indent_string,
		crlf: matches!(fmt_config.line_ending, LineEnding::Crlf),
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
		let source = "[dependencies]\nserde = {version=\"1.0\",features=[\"derive\"]}";
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
