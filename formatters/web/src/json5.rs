//! JSON5 formatter via `dprint-plugin-json`.

use dprint_core_json::configuration::NewLineKind;
use dprint_plugin_json::configuration::{
	Configuration, ConfigurationBuilder, TrailingCommaKind,
};
use fama_common::{Formatter, IndentStyle, LineEnding, TrailingComma, CONFIG};
use std::path::Path;
use std::sync::OnceLock;

/// Formatter implementation for JSON5 via `dprint-plugin-json`.
pub struct Json5Formatter;

impl Formatter for Json5Formatter {
	fn name(&self) -> &'static str {
		"json5"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["json5"]
	}

	fn format(&self, content: &str, path: &Path) -> Result<String, String> {
		format_json5(content, path)
	}
}

fn json5_config() -> &'static Configuration {
	static CFG: OnceLock<Configuration> = OnceLock::new();
	CFG.get_or_init(|| {
		let mut builder = ConfigurationBuilder::new();
		builder
			.line_width(CONFIG.line_width as u32)
			.use_tabs(matches!(CONFIG.indent_style, IndentStyle::Tabs))
			.indent_width(CONFIG.indent_width)
			.new_line_kind(match CONFIG.line_ending {
				LineEnding::Lf => NewLineKind::LineFeed,
				LineEnding::Crlf => NewLineKind::CarriageReturnLineFeed,
			})
			.trailing_commas(match CONFIG.trailing_comma {
				TrailingComma::All => TrailingCommaKind::Always,
				TrailingComma::None => TrailingCommaKind::Never,
			});
		builder.build()
	})
}

pub fn format_json5(source: &str, path: &Path) -> Result<String, String> {
	let target = if path.as_os_str().is_empty() {
		Path::new("config.json5")
	} else {
		path
	};
	match dprint_plugin_json::format_text(target, source, json5_config()) {
		Ok(Some(formatted)) => Ok(formatted),
		Ok(None) => Ok(source.to_string()),
		Err(e) => Err(format!("JSON5 formatting error: {e}")),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_format_json5_and_idempotence() {
		let messy = "{\n  // comment\n  unquotedKey: 'single',\n  hex: 0x10,\n  arr: [1,2,],\n}\n";
		let once = format_json5(messy, Path::new("cfg.json5")).unwrap();
		assert!(once.contains("\"unquotedKey\": \"single\""), "{once}");
		assert!(once.contains("\t// comment"), "{once}");
		let twice = format_json5(&once, Path::new("cfg.json5")).unwrap();
		assert_eq!(once, twice);
	}

	#[test]
	fn test_format_json5_invalid() {
		assert!(format_json5("{ broken: ", Path::new("bad.json5")).is_err());
	}
}
