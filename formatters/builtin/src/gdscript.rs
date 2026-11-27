//! GDScript formatter via `gdck-format`.

use fama_common::{Formatter, IndentStyle, CONFIG};
use gdck_config::{FormatConfig as GdFormatConfig, IndentStyle as GdIndentStyle};
use std::path::Path;

/// Formatter implementation for GDScript via `gdck-format`.
pub struct GdscriptFormatter;

impl Formatter for GdscriptFormatter {
	fn name(&self) -> &'static str {
		"gdscript"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["gd"]
	}

	fn format(&self, content: &str, _path: &Path) -> Result<String, String> {
		format_gdscript(content)
	}
}

pub fn format_gdscript(source: &str) -> Result<String, String> {
	let mut cfg = GdFormatConfig::default();
	cfg.line_length = CONFIG.line_width;
	cfg.indent = match CONFIG.indent_style {
		IndentStyle::Tabs => GdIndentStyle::Tabs,
		IndentStyle::Spaces => GdIndentStyle::Spaces(CONFIG.indent_width),
	};

	gdck_format::format_source(source, &cfg)
		.map_err(|e| format!("GDScript formatting error: {e}"))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_format_gdscript_and_idempotence() {
		let messy = "extends Node\n\nfunc _ready()->void:\n\tvar x:int=1+2\n\tprint(x)\n";
		let once = format_gdscript(messy).unwrap();
		assert!(once.contains("var x: int = 1 + 2"), "{once}");
		let twice = format_gdscript(&once).unwrap();
		assert_eq!(once, twice);
	}

	#[test]
	fn test_format_gdscript_syntax_error() {
		assert!(format_gdscript("func broken(:").is_err());
	}
}
