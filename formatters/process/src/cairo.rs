//! Cairo formatter via `cairo-format` (process mode).

use crate::process::{resolve_binary, run_stdin};
use fama_common::Formatter;
use std::path::Path;

pub const CAIRO_FORMAT_PATH_ENV: &str = "FAMA_CAIRO_FORMAT_PATH";

const INSTALL_HINT: &str =
	"Install `cairo-format` (Scarb / Cairo toolchain) to format Cairo files, \
	 or point FAMA_CAIRO_FORMAT_PATH at the binary.";

/// Formatter implementation for Cairo via `cairo-format`.
pub struct CairoFormatter;

impl Formatter for CairoFormatter {
	fn name(&self) -> &'static str {
		"cairo"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["cairo"]
	}

	fn format(&self, content: &str, _path: &Path) -> Result<String, String> {
		format_cairo(content)
	}
}

pub fn cairo_format_binary() -> String {
	resolve_binary(CAIRO_FORMAT_PATH_ENV, "cairo-format")
}

pub fn format_cairo(source: &str) -> Result<String, String> {
	let bin = cairo_format_binary();
	run_stdin(&bin, &["--stdin"], source, "cairo-format", INSTALL_HINT)
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::process::Command;

	#[test]
	fn test_cairo_metadata_and_env() {
		assert_eq!(CairoFormatter.name(), "cairo");
		assert_eq!(CairoFormatter.extensions(), &["cairo"]);
		assert!(!cairo_format_binary().is_empty());
	}

	#[test]
	fn test_missing_cairo_reports_helpful_error() {
		let err = run_stdin(
			"fama-nonexistent-cairo-format-bin",
			&["--stdin"],
			"fn main() {}",
			"cairo-format",
			INSTALL_HINT,
		)
		.unwrap_err();
		assert!(err.contains("was not found in PATH"), "{err}");
	}

	#[test]
	fn test_format_cairo_live_if_installed() {
		let bin = cairo_format_binary();
		if !Command::new(&bin)
			.arg("--version")
			.output()
			.is_ok_and(|o| o.status.success())
		{
			return;
		}
		let once = format_cairo("fn main(){let x=1;}\n").unwrap();
		let twice = format_cairo(&once).unwrap();
		assert_eq!(once, twice);
	}
}
