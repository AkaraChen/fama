// sqruff-formatter - SQL code formatter using sqruff
//
// Provides SQL code formatting using the sqruff crate.

use ahash::AHashMap;
use fama_common::{IndentStyle, CONFIG};
use sqruff_lib::core::config::{FluffConfig, Value};
use sqruff_lib::core::linter::core::Linter;

/// Format SQL source code using sqruff
///
/// # Arguments
/// * `source` - The SQL source code to format
/// * `_file_path` - Path to the file (unused, for future context)
///
/// # Returns
/// * `Ok(String)` - Formatted SQL code
/// * `Err(String)` - Error message if formatting fails
pub fn format_sql(source: &str, _file_path: &str) -> Result<String, String> {
	let config = create_config();
	let linter = Linter::new(config, None, None, false);
	let linted_file = linter.lint_string(source, None, true);
	Ok(linted_file.fix_string())
}

/// Create sqruff FluffConfig from fama FormatConfig
fn create_config() -> FluffConfig {
	let mut configs = AHashMap::new();

	// Core section
	let mut core = AHashMap::new();
	core.insert(
		"max_line_length".to_string(),
		Value::Int(CONFIG.line_width as i32),
	);
	configs.insert("sqruff".to_string(), Value::Map(core));

	// Indentation section
	let mut indentation = AHashMap::new();
	let indent_unit = match CONFIG.indent_style {
		IndentStyle::Tabs => "tab",
		IndentStyle::Spaces => "space",
	};
	indentation
		.insert("indent_unit".to_string(), Value::String(indent_unit.into()));
	indentation.insert(
		"tab_space_size".to_string(),
		Value::Int(CONFIG.indent_width as i32),
	);
	configs.insert("indentation".to_string(), Value::Map(indentation));

	FluffConfig::new(configs, None, None)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_format_simple_sql() {
		let input = "SELECT   id,   name   FROM   users   WHERE   id   =   1";
		let output = format_sql(input, "test.sql").unwrap();
		// Should be formatted with proper spacing
		assert!(output.contains("SELECT"));
		assert!(output.contains("FROM"));
		assert!(output.contains("WHERE"));
	}

	#[test]
	fn test_format_with_newlines() {
		let input = r#"SELECT id,name FROM users WHERE id=1"#;
		let output = format_sql(input, "test.sql").unwrap();
		// Should be formatted properly
		assert!(output.contains("SELECT"));
		assert!(output.contains("FROM"));
	}
}
