//! KDL document formatter via the `kdl` crate.

use fama_common::{Formatter, IndentStyle, CONFIG};
use kdl::{FormatConfig, KdlDocument};
use std::path::Path;

/// Formatter implementation for KDL documents via `kdl`.
pub struct KdlFormatter;

impl Formatter for KdlFormatter {
	fn name(&self) -> &'static str {
		"kdl"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["kdl"]
	}

	fn format(&self, content: &str, _path: &Path) -> Result<String, String> {
		format_kdl(content)
	}
}

pub fn format_kdl(source: &str) -> Result<String, String> {
	let mut doc: KdlDocument = source
		.parse()
		.map_err(|e| format!("KDL parse error: {e}"))?;

	let indent = match CONFIG.indent_style {
		IndentStyle::Tabs => "\t".to_string(),
		IndentStyle::Spaces => " ".repeat(CONFIG.indent_width as usize),
	};
	let cfg = FormatConfig::builder().indent(&indent).build();
	doc.autoformat_config(&cfg);

	let mut formatted = doc.to_string();
	if !formatted.ends_with('\n') {
		formatted.push('\n');
	}
	Ok(formatted)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_format_kdl_and_idempotence() {
		let messy = "node   123   prop=\"val\" {\nchild   42\n}\n";
		let once = format_kdl(messy).unwrap();
		assert!(once.contains("\tchild 42"), "{once}");
		let twice = format_kdl(&once).unwrap();
		assert_eq!(once, twice);
	}

	#[test]
	fn test_format_kdl_invalid() {
		assert!(format_kdl("node { unclosed").is_err());
	}
}
