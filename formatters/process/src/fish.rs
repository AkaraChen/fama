//! Fish shell formatter via `fish_indent` (process mode).

use crate::process::{resolve_binary, run_stdin};
use fama_common::Formatter;
use std::path::Path;

pub const FISH_INDENT_PATH_ENV: &str = "FAMA_FISH_INDENT_PATH";

const INSTALL_HINT: &str =
	"Install Fish shell (`fish_indent`, https://fishshell.com) to format Fish scripts, \
	 or point FAMA_FISH_INDENT_PATH at the binary.";

/// Formatter implementation for Fish shell scripts via `fish_indent`.
pub struct FishFormatter;

impl Formatter for FishFormatter {
	fn name(&self) -> &'static str {
		"fish"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["fish"]
	}

	fn format(&self, content: &str, _path: &Path) -> Result<String, String> {
		format_fish(content)
	}
}

pub fn fish_indent_binary() -> String {
	resolve_binary(FISH_INDENT_PATH_ENV, "fish_indent")
}

pub fn format_fish(source: &str) -> Result<String, String> {
	let bin = fish_indent_binary();
	run_stdin(&bin, &[], source, "fish_indent", INSTALL_HINT)
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::process::Command;

	#[test]
	fn test_fish_metadata_and_env() {
		assert_eq!(FishFormatter.name(), "fish");
		assert_eq!(FishFormatter.extensions(), &["fish"]);
		assert!(!fish_indent_binary().is_empty());
	}

	#[test]
	fn test_missing_fish_indent_reports_helpful_error() {
		let err = run_stdin(
			"fama-nonexistent-fish-indent-bin",
			&[],
			"if test $x = 1;echo hi;end\n",
			"fish_indent",
			INSTALL_HINT,
		)
		.unwrap_err();
		assert!(err.contains("was not found in PATH"), "{err}");
		assert!(err.contains(FISH_INDENT_PATH_ENV), "{err}");
	}

	#[test]
	fn test_format_fish_live_if_installed() {
		let bin = fish_indent_binary();
		if !Command::new(&bin)
			.arg("--version")
			.output()
			.is_ok_and(|o| o.status.success())
		{
			return;
		}
		let once = format_fish("if test $x = 1;echo hi;end\n").unwrap();
		let twice = format_fish(&once).unwrap();
		assert_eq!(once, twice);
	}
}
