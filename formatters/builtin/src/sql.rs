// sql.rs - SQL code formatter using sqruff

use ahash::AHashMap;
use fama_common::{Formatter, IndentStyle, CONFIG};
use sqruff_lib::core::config::{FluffConfig, Value};
use sqruff_lib::core::linter::core::Linter;
use std::path::Path;

/// Formatter implementation for SQL via `sqruff`.
pub struct SqlFormatter;

impl Formatter for SqlFormatter {
	fn name(&self) -> &'static str {
		"sql"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["sql"]
	}

	fn format(&self, content: &str, path: &Path) -> Result<String, String> {
		format_sql(content, path.to_str().unwrap_or(""))
	}
}

thread_local! {
	static LINTER: Linter = Linter::new(create_config(), None, None, false);
}

pub fn format_sql(source: &str, _file_path: &str) -> Result<String, String> {
	Ok(LINTER.with(|linter| {
		let linted_file = linter.lint_string(source, None, true);
		linted_file.fix_string()
	}))
}

fn create_config() -> FluffConfig {
	let mut configs = AHashMap::new();

	let mut core = AHashMap::new();
	core.insert(
		"max_line_length".to_string(),
		Value::Int(CONFIG.line_width as i32),
	);
	configs.insert("sqruff".to_string(), Value::Map(core));

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

	let mut rules = AHashMap::new();
	rules.insert(
		"capitalisation_policy".to_string(),
		Value::String("upper".into()),
	);
	configs.insert("rules".to_string(), Value::Map(rules));

	FluffConfig::new(configs, None, None)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_format_simple_sql() {
		let input = "SELECT   id,   name   FROM   users   WHERE   id   =   1";
		let output = format_sql(input, "test.sql").unwrap();
		assert!(output.contains("SELECT"));
		assert!(output.contains("FROM"));
		assert!(output.contains("WHERE"));
	}

	#[test]
	fn test_format_with_newlines() {
		let input = r#"SELECT id,name FROM users WHERE id=1"#;
		let output = format_sql(input, "test.sql").unwrap();
		assert!(output.contains("SELECT"));
		assert!(output.contains("FROM"));
	}
}
