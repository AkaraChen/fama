//! Typst formatter via `typstyle-core`.

use fama_common::{Formatter, LineEnding as FamaLineEnding, CONFIG};
use std::path::Path;
use typstyle_core::{Config, LineEnding, Typstyle};

/// Formatter implementation for Typst via `typstyle-core`.
pub struct TypstFormatter;

impl Formatter for TypstFormatter {
	fn name(&self) -> &'static str {
		"typst"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["typ"]
	}

	fn format(&self, content: &str, _path: &Path) -> Result<String, String> {
		format_typst(content)
	}
}

pub fn format_typst(source: &str) -> Result<String, String> {
	let line_ending = match CONFIG.line_ending {
		FamaLineEnding::Lf => LineEnding::Lf,
		FamaLineEnding::Crlf => LineEnding::Crlf,
	};
	let config = Config {
		tab_spaces: CONFIG.indent_width as usize,
		max_width: CONFIG.line_width as usize,
		line_ending,
		..Config::default()
	};

	Typstyle::new(config)
		.format_text(source)
		.render()
		.map_err(|e| format!("Typst formatting error: {e}"))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_format_typst_and_idempotence() {
		let messy = "#let  add( x , y )={x+y}\n= Heading\nHello   world!\n";
		let once = format_typst(messy).unwrap();
		assert!(once.contains("#let add(x, y)"), "{once}");
		let twice = format_typst(&once).unwrap();
		assert_eq!(once, twice);
	}

	#[test]
	fn test_format_typst_syntax_error() {
		assert!(format_typst("#let x = (").is_err());
	}
}
