// lua-formatter - Lua code formatter using StyLua
//
// Provides Lua code formatting using the stylua crate.

use fama_common::{FormatConfig, IndentStyle, LineEnding, QuoteStyle};
use stylua_lib::{format_code, Config, IndentType, LineEndings, OutputVerification, QuoteStyle as StyluaQuoteStyle};

/// Format Lua source code using StyLua
///
/// # Arguments
/// * `source` - The Lua source code to format
/// * `_file_path` - Path to the file (unused, for future context)
///
/// # Returns
/// * `Ok(String)` - Formatted Lua code
/// * `Err(String)` - Error message if formatting fails
pub fn format_lua(source: &str, _file_path: &str) -> Result<String, String> {
    let fmt_config = FormatConfig::default();

    let indent_type = match fmt_config.indent_style {
        IndentStyle::Spaces => IndentType::Spaces,
        IndentStyle::Tabs => IndentType::Tabs,
    };

    let line_endings = match fmt_config.line_ending {
        LineEnding::Lf => LineEndings::Unix,
        LineEnding::Crlf => LineEndings::Windows,
    };

    let quote_style = match fmt_config.quote_style {
        QuoteStyle::Single => StyluaQuoteStyle::ForceSingle,
        QuoteStyle::Double => StyluaQuoteStyle::ForceDouble,
    };

    let config = Config {
        indent_type,
        indent_width: fmt_config.indent_width as usize,
        line_endings,
        column_width: fmt_config.line_width as usize,
        quote_style,
        ..Config::default()
    };

    format_code(source, config, None, OutputVerification::None)
        .map_err(|e| format!("StyLua error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_simple_lua() {
        let input = r#"
local   x   =    1
local  y=2
function   foo(  a,b  )
    return    a+b
end
"#;

        let output = format_lua(input, "test.lua").unwrap();

        // Should be formatted with proper spacing
        assert!(output.contains("local x = 1"));
        assert!(output.contains("local y = 2"));
        assert!(output.contains("function foo(a, b)"));
    }

    #[test]
    fn test_format_table() {
        let input = "local t={a=1,b=2}";
        let output = format_lua(input, "test.lua").unwrap();
        // Should format with proper spacing
        assert!(output.contains("local t = "));
    }

    #[test]
    fn test_format_with_comments() {
        let input = r#"
-- This is a comment
local   x   =   1  -- inline comment
"#;
        let output = format_lua(input, "test.lua").unwrap();
        assert!(output.contains("-- This is a comment"));
        assert!(output.contains("local x = 1"));
    }
}
