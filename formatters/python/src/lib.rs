// ruff-formatter - Python code formatter using ruff_python_formatter
//
// Provides Python code formatting using the ruff formatter library directly.

use fama_common::{FormatConfig, IndentStyle, LineEnding};
use ruff_formatter::printer::LineEnding as RuffLineEnding;
use ruff_formatter::{IndentStyle as RuffIndentStyle, IndentWidth, LineWidth};
use ruff_python_formatter::{format_module_source, PyFormatOptions};

/// Format Python source code using ruff formatter
///
/// # Arguments
/// * `source` - The Python source code to format
/// * `_file_path` - The original file path (for context)
///
/// # Returns
/// * `Ok(String)` - Formatted code
/// * `Err(String)` - Error message if formatting fails
pub fn format_python(source: &str, _file_path: &str) -> Result<String, String> {
    let config = FormatConfig::default();

    let indent_style = match config.indent_style {
        IndentStyle::Tabs => RuffIndentStyle::Tab,
        IndentStyle::Spaces => RuffIndentStyle::Space,
    };

    let line_ending = match config.line_ending {
        LineEnding::Lf => RuffLineEnding::LineFeed,
        LineEnding::Crlf => RuffLineEnding::CarriageReturnLineFeed,
    };

    let options = PyFormatOptions::default()
        .with_indent_style(indent_style)
        .with_indent_width(IndentWidth::try_from(config.indent_width).unwrap())
        .with_line_width(LineWidth::try_from(config.line_width).unwrap())
        .with_line_ending(line_ending);

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
        let source = "class Foo:\n    def __init__(self,x):\n        self.x=x\n";
        let result = format_python(source, "test.py").unwrap();
        assert!(result.contains("class Foo:"));
        assert!(result.contains("self.x = x"));
    }
}
