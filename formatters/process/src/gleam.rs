//! Gleam formatter via `gleam format` (process mode).

use crate::process::{resolve_binary, run_stdin};
use fama_common::Formatter;
use std::path::Path;

pub const GLEAM_PATH_ENV: &str = "FAMA_GLEAM_PATH";

const INSTALL_HINT: &str =
	"Install Gleam (`gleam`, https://gleam.run) to format Gleam files, \
	 or point FAMA_GLEAM_PATH at the binary.";

/// Formatter implementation for Gleam via `gleam format`.
pub struct GleamFormatter;

impl Formatter for GleamFormatter {
	fn name(&self) -> &'static str {
		"gleam"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["gleam"]
	}

	fn format(&self, content: &str, _path: &Path) -> Result<String, String> {
		format_gleam(content)
	}
}

pub fn gleam_binary() -> String {
	resolve_binary(GLEAM_PATH_ENV, "gleam")
}

pub fn format_gleam(source: &str) -> Result<String, String> {
	let bin = gleam_binary();
	run_stdin(&bin, &["format", "--stdin"], source, "gleam", INSTALL_HINT)
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::process::Command;

	#[test]
	fn test_gleam_metadata_and_env() {
		assert_eq!(GleamFormatter.name(), "gleam");
		assert_eq!(GleamFormatter.extensions(), &["gleam"]);
		assert!(!gleam_binary().is_empty());
	}

	#[test]
	fn test_missing_gleam_reports_helpful_error() {
		let err = run_stdin(
			"fama-nonexistent-gleam-bin",
			&["format", "--stdin"],
			"pub fn main() { 1 }",
			"gleam",
			INSTALL_HINT,
		)
		.unwrap_err();
		assert!(err.contains("was not found in PATH"), "{err}");
	}

	#[test]
	fn test_format_gleam_live_if_installed() {
		let bin = gleam_binary();
		if !Command::new(&bin)
			.arg("--version")
			.output()
			.is_ok_and(|o| o.status.success())
		{
			return;
		}
		let once = format_gleam("pub fn main(){1}\n").unwrap();
		let twice = format_gleam(&once).unwrap();
		assert_eq!(once, twice);
	}
}
