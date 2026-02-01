// lua-formatter - Lua code formatter using StyLua
//
// Provides Lua code formatting using the stylua crate.
// Uses 2-space indentation (Lua convention).

use stylua_lib::{format_code, Config, IndentType, LineEndings, OutputVerification};

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
    // Configure StyLua with Lua conventions
    let config = Config {
        indent_type: IndentType::Spaces,
        indent_width: 2,
        line_endings: LineEndings::Unix,
        ..Config::default()
    };

    // Format the code
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
