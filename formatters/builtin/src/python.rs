// python.rs - Python code formatter using ruff_python_formatter

use fama_common::{Formatter, CONFIG};
use ruff_formatter::printer::LineEnding as RuffLineEnding;
use ruff_formatter::{IndentStyle as RuffIndentStyle, IndentWidth, LineWidth};
use ruff_python_formatter::{
	format_module_source, PyFormatOptions, QuoteStyle as RuffQuoteStyle,
};
use std::path::Path;

/// Formatter implementation for Python via Ruff.
pub struct PythonFormatter;

impl Formatter for PythonFormatter {
	fn name(&self) -> &'static str {
		"python"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["py"]
	}

	fn format(&self, content: &str, path: &Path) -> Result<String, String> {
		format_python(content, path.to_str().unwrap_or(""))
	}
}

const RUFF_INDENT_STYLE: RuffIndentStyle = match CONFIG.indent_style {
	fama_common::IndentStyle::Tabs => RuffIndentStyle::Tab,
	fama_common::IndentStyle::Spaces => RuffIndentStyle::Space,
};
const RUFF_INDENT_WIDTH: u8 = CONFIG.indent_width;
const RUFF_LINE_WIDTH: u16 = CONFIG.line_width;
const RUFF_LINE_ENDING: RuffLineEnding = match CONFIG.line_ending {
	fama_common::LineEnding::Lf => RuffLineEnding::LineFeed,
	fama_common::LineEnding::Crlf => RuffLineEnding::CarriageReturnLineFeed,
};
const RUFF_QUOTE_STYLE: RuffQuoteStyle = match CONFIG.quote_style {
	fama_common::QuoteStyle::Single => RuffQuoteStyle::Single,
	fama_common::QuoteStyle::Double => RuffQuoteStyle::Double,
};

pub fn format_python(source: &str, _file_path: &str) -> Result<String, String> {
	let options = PyFormatOptions::default()
		.with_indent_style(RUFF_INDENT_STYLE)
		.with_indent_width(IndentWidth::try_from(RUFF_INDENT_WIDTH).unwrap())
		.with_line_width(LineWidth::try_from(RUFF_LINE_WIDTH).unwrap())
		.with_line_ending(RUFF_LINE_ENDING)
		.with_quote_style(RUFF_QUOTE_STYLE);

	format_module_source(source, options)
		.map(|printed| printed.into_code())
		.map_err(|e| format!("Python formatting error: {}", e))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_format_python_basic() {
		let source = "x=1+2\ny=3\n";
		let result = format_python(source, "test.py").unwrap();
		assert!(result.contains("x = 1 + 2"));
		assert!(result.contains("y = 3"));
	}

	#[test]
	fn test_format_python_function() {
		let source = "def foo(x,y):\n    return x+y\n";
		let result = format_python(source, "test.py").unwrap();
		assert!(result.contains("def foo(x, y):"));
		assert!(result.contains("return x + y"));
	}

	#[test]
	fn test_format_python_class() {
		let source =
			"class Foo:\n    def __init__(self,x):\n        self.x=x\n";
		let result = format_python(source, "test.py").unwrap();
		assert!(result.contains("class Foo:"));
		assert!(result.contains("self.x = x"));
	}
}
