// php-formatter - PHP code formatter using Mago
//
// Provides PHP code formatting using the mago-formatter crate.

use fama_common::CONFIG;
use mago_formatter::{settings::FormatSettings, Formatter};
use mago_interner::ThreadedInterner;
use mago_php_version::PHPVersion;

/// Format PHP source code using Mago
///
/// # Arguments
/// * `source` - The PHP source code to format
/// * `file_path` - Path to the file (used for error reporting)
///
/// # Returns
/// * `Ok(String)` - Formatted PHP code
/// * `Err(String)` - Error message if formatting fails
pub fn format_php(source: &str, file_path: &str) -> Result<String, String> {
	let interner = ThreadedInterner::new();
	let mago_brace_style = match CONFIG.brace_style {
		fama_common::BraceStyle::SameLine => {
			mago_formatter::settings::BraceStyle::SameLine
		}
		fama_common::BraceStyle::NewLine => {
			mago_formatter::settings::BraceStyle::NextLine
		}
	};

	let settings = FormatSettings {
		print_width: CONFIG.line_width as usize,
		tab_width: CONFIG.indent_width as usize,
		use_tabs: matches!(CONFIG.indent_style, fama_common::IndentStyle::Tabs),
		end_of_line: match CONFIG.line_ending {
			fama_common::LineEnding::Lf => {
				mago_formatter::settings::EndOfLine::Lf
			}
			fama_common::LineEnding::Crlf => {
				mago_formatter::settings::EndOfLine::Crlf
			}
		},
		single_quote: matches!(
			CONFIG.quote_style,
			fama_common::QuoteStyle::Single
		),
		trailing_comma: matches!(
			CONFIG.trailing_comma,
			fama_common::TrailingComma::All
		),
		control_brace_style: mago_brace_style,
		closure_brace_style: mago_brace_style,
		function_brace_style: mago_brace_style,
		method_brace_style: mago_brace_style,
		classlike_brace_style: mago_brace_style,
		..FormatSettings::default()
	};

	let php_version = PHPVersion::new(8, 3, 0);
	let formatter = Formatter::new(&interner, php_version, settings);

	formatter
		.format_code(file_path, source)
		.map_err(|e| format!("Mago error: {}", e))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_format_simple_php() {
		let input = r#"<?php
function   foo(  $a,  $b  ){return   $a+$b;}
"#;

		let output = format_php(input, "test.php").unwrap();
		// Should be formatted with proper spacing and braces
		assert!(output.contains("function foo("));
	}

	#[test]
	fn test_format_class() {
		let input = r#"<?php
class   Foo{public   function   bar(){return   1;}}
"#;

		let output = format_php(input, "test.php").unwrap();
		assert!(output.contains("class Foo"));
	}

	#[test]
	fn test_format_with_comments() {
		let input = r#"<?php
// This is a comment
$x   =   1;  // inline comment
"#;

		let output = format_php(input, "test.php").unwrap();
		assert!(output.contains("// This is a comment"));
		assert!(output.contains("$x = 1;"));
	}
}
