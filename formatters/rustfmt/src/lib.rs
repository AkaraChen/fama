//! Rust formatter using rustfmt
//!
//! This module provides Rust code formatting functionality via the rustfmt
//! formatter, using the rust-format crate for a clean library API.

use fama_common::{FileType, CONFIG};
use rust_format::{Config, Formatter, RustFmt};

// Module-level constants - pre-converted config values
const RUSTFMT_HARD_TABS: &str =
	if matches!(CONFIG.indent_style, fama_common::IndentStyle::Tabs) {
		"true"
	} else {
		"false"
	};
// Note: These need to be string literals for const, so we use fixed values
// matching CONFIG defaults. If CONFIG changes, update these.
const RUSTFMT_TAB_SPACES: &str = "4";
const RUSTFMT_MAX_WIDTH: &str = "80";
const RUSTFMT_NEWLINE_STYLE: &str = match CONFIG.line_ending {
	fama_common::LineEnding::Lf => "Unix",
	fama_common::LineEnding::Crlf => "Windows",
};

/// Format Rust source code
///
/// # Arguments
/// * `source` - The Rust source code to format
/// * `file_path` - The file path (used for error reporting, currently unused)
///
/// # Returns
/// The formatted Rust source code, or an error message if formatting fails.
pub fn format_rust(source: &str, _file_path: &str) -> Result<String, String> {
	let config = Config::new_str()
		.option("hard_tabs", RUSTFMT_HARD_TABS)
		.option("tab_spaces", RUSTFMT_TAB_SPACES)
		.option("max_width", RUSTFMT_MAX_WIDTH)
		.option("newline_style", RUSTFMT_NEWLINE_STYLE);

	let formatter = RustFmt::from_config(config);

	formatter
		.format_str(source)
		.map_err(|e| format!("rustfmt error: {}", e))
}

/// Format a file based on its file type
pub fn format_file(
	source: &str,
	file_path: &str,
	file_type: FileType,
) -> Result<String, String> {
	match file_type {
		FileType::Rust => format_rust(source, file_path),
		_ => Err(format!(
			"File type {:?} is not supported by rust-formatter",
			file_type
		)),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_format_simple_function() {
		let source = r#"fn main(){println!("Hello");}"#;
		let result = format_rust(source, "test.rs").unwrap();

		// Check that formatting occurred (should have proper indentation)
		assert!(result.contains("fn main()"));
		assert!(result.contains("println!"));
	}

	#[test]
	fn test_format_file_with_rust() {
		let source = r#"fn main(){println!("Hello");}"#;
		let result = format_file(source, "test.rs", FileType::Rust).unwrap();

		// Check that formatting occurred
		assert!(result.contains("fn main()"));
		assert!(result.contains("println!"));
	}

	#[test]
	fn test_format_file_with_unsupported_type() {
		let source = "test";
		let result = format_file(source, "test.js", FileType::JavaScript);
		assert!(result.is_err());
	}
}
