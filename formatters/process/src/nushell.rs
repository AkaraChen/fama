//! Nushell formatter via `nufmt` (process mode).

use crate::process::{resolve_binary, run_stdin};
use fama_common::Formatter;
use std::path::Path;

pub const NUFMT_PATH_ENV: &str = "FAMA_NUFMT_PATH";

const INSTALL_HINT: &str =
	"Install `nufmt` (https://github.com/nushell/nufmt) to format Nushell files, \
	 or point FAMA_NUFMT_PATH at the binary.";

/// Formatter implementation for Nushell via `nufmt`.
pub struct NushellFormatter;

impl Formatter for NushellFormatter {
	fn name(&self) -> &'static str {
		"nushell"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["nu"]
	}

	fn format(&self, content: &str, _path: &Path) -> Result<String, String> {
		format_nushell(content)
	}
}

pub fn nufmt_binary() -> String {
	resolve_binary(NUFMT_PATH_ENV, "nufmt")
}

pub fn format_nushell(source: &str) -> Result<String, String> {
	let bin = nufmt_binary();
	run_stdin(&bin, &["--stdin"], source, "nufmt", INSTALL_HINT)
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::process::Command;

	#[test]
	fn test_nushell_metadata_and_env() {
		assert_eq!(NushellFormatter.name(), "nushell");
		assert_eq!(NushellFormatter.extensions(), &["nu"]);
		assert!(!nufmt_binary().is_empty());
	}

	#[test]
	fn test_missing_nufmt_reports_helpful_error() {
		let err = run_stdin(
			"fama-nonexistent-nufmt-bin",
			&["--stdin"],
			"def greet [] { 'hi' }",
			"nufmt",
			INSTALL_HINT,
		)
		.unwrap_err();
		assert!(err.contains("was not found in PATH"), "{err}");
	}

	#[test]
	fn test_format_nushell_live_if_installed() {
		let bin = nufmt_binary();
		if !Command::new(&bin)
			.arg("--version")
			.output()
			.is_ok_and(|o| o.status.success())
		{
			return;
		}
		let once = format_nushell("def greet [] {'hi'}\n").unwrap();
		let twice = format_nushell(&once).unwrap();
		assert_eq!(once, twice);
	}
}
