// lua.rs - Lua code formatter using StyLua

use fama_common::{Formatter, CONFIG};
use std::path::Path;
use stylua_lib::{
	format_code, Config, IndentType, LineEndings, OutputVerification,
	QuoteStyle as StyluaQuoteStyle,
};

/// Formatter implementation for Lua via StyLua.
pub struct LuaFormatter;

impl Formatter for LuaFormatter {
	fn name(&self) -> &'static str {
		"lua"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["lua"]
	}

	fn format(&self, content: &str, path: &Path) -> Result<String, String> {
		format_lua(content, path.to_str().unwrap_or(""))
	}
}

const STYLUA_INDENT_TYPE: IndentType = match CONFIG.indent_style {
	fama_common::IndentStyle::Spaces => IndentType::Spaces,
	fama_common::IndentStyle::Tabs => IndentType::Tabs,
};
const STYLUA_INDENT_WIDTH: usize = CONFIG.indent_width as usize;
const STYLUA_LINE_ENDINGS: LineEndings = match CONFIG.line_ending {
	fama_common::LineEnding::Lf => LineEndings::Unix,
	fama_common::LineEnding::Crlf => LineEndings::Windows,
};
const STYLUA_COLUMN_WIDTH: usize = CONFIG.line_width as usize;
const STYLUA_QUOTE_STYLE: StyluaQuoteStyle = match CONFIG.quote_style {
	fama_common::QuoteStyle::Single => StyluaQuoteStyle::ForceSingle,
	fama_common::QuoteStyle::Double => StyluaQuoteStyle::ForceDouble,
};

pub fn format_lua(source: &str, _file_path: &str) -> Result<String, String> {
	let config = Config {
		indent_type: STYLUA_INDENT_TYPE,
		indent_width: STYLUA_INDENT_WIDTH,
		line_endings: STYLUA_LINE_ENDINGS,
		column_width: STYLUA_COLUMN_WIDTH,
		quote_style: STYLUA_QUOTE_STYLE,
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

		assert!(output.contains("local x = 1"));
		assert!(output.contains("local y = 2"));
		assert!(output.contains("function foo(a, b)"));
	}

	#[test]
	fn test_format_table() {
		let input = "local t={a=1,b=2}";
		let output = format_lua(input, "test.lua").unwrap();
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
