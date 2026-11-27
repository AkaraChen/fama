//! Crystal formatter via `crystal tool format` (process mode).

use crate::process::{resolve_binary, run_stdin};
use fama_common::Formatter;
use std::path::Path;

pub const CRYSTAL_PATH_ENV: &str = "FAMA_CRYSTAL_PATH";

const INSTALL_HINT: &str =
	"Install Crystal (`crystal`, https://crystal-lang.org) to format Crystal files, \
	 or point FAMA_CRYSTAL_PATH at the binary.";

/// Formatter implementation for Crystal via `crystal tool format`.
pub struct CrystalFormatter;

impl Formatter for CrystalFormatter {
	fn name(&self) -> &'static str {
		"crystal"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["cr"]
	}

	fn format(&self, content: &str, _path: &Path) -> Result<String, String> {
		format_crystal(content)
	}
}

pub fn crystal_binary() -> String {
	resolve_binary(CRYSTAL_PATH_ENV, "crystal")
}

pub fn format_crystal(source: &str) -> Result<String, String> {
	let bin = crystal_binary();
	run_stdin(
		&bin,
		&["tool", "format", "-"],
		source,
		"crystal",
		INSTALL_HINT,
	)
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::process::Command;

	#[test]
	fn test_crystal_metadata_and_env() {
		assert_eq!(CrystalFormatter.name(), "crystal");
		assert_eq!(CrystalFormatter.extensions(), &["cr"]);
		assert!(!crystal_binary().is_empty());
	}

	#[test]
	fn test_missing_crystal_reports_helpful_error() {
		let err = run_stdin(
			"fama-nonexistent-crystal-bin",
			&["tool", "format", "-"],
			"puts 1+2\n",
			"crystal",
			INSTALL_HINT,
		)
		.unwrap_err();
		assert!(err.contains("was not found in PATH"), "{err}");
		assert!(err.contains(CRYSTAL_PATH_ENV), "{err}");
	}

	#[test]
	fn test_format_crystal_live_if_installed() {
		let bin = crystal_binary();
		if !Command::new(&bin)
			.arg("--version")
			.output()
			.is_ok_and(|o| o.status.success())
		{
			return;
		}
		let once = format_crystal("def greet(name)\nputs name\nend\n").unwrap();
		let twice = format_crystal(&once).unwrap();
		assert_eq!(once, twice);
	}
}
