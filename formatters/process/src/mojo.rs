//! Mojo formatter via `mojo format` (process mode).

use crate::process::{command_error, resolve_binary, success_status};
use fama_common::{Formatter, CONFIG};
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

pub const MOJO_PATH_ENV: &str = "FAMA_MOJO_PATH";

const INSTALL_HINT: &str =
	"Install Mojo (`mojo`, https://docs.modular.com/mojo) to format Mojo files, \
	 or point FAMA_MOJO_PATH at the binary.";

/// Formatter implementation for Mojo (`.mojo`, `.🔥`) via `mojo format`.
pub struct MojoFormatter;

impl Formatter for MojoFormatter {
	fn name(&self) -> &'static str {
		"mojo"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["mojo", "🔥"]
	}

	fn format(&self, content: &str, _path: &Path) -> Result<String, String> {
		format_mojo(content)
	}
}

pub fn mojo_binary() -> String {
	resolve_binary(MOJO_PATH_ENV, "mojo")
}

pub fn format_mojo(source: &str) -> Result<String, String> {
	format_mojo_with_bin(&mojo_binary(), source)
}

fn format_mojo_with_bin(bin: &str, source: &str) -> Result<String, String> {
	let temp_dir = TempDir::new().map_err(|e| {
		format!("mojo formatting: failed to create temp dir: {e}")
	})?;
	let temp_file = temp_dir.path().join("input.mojo");
	fs::write(&temp_file, source).map_err(|e| {
		format!("mojo formatting: failed to write temp file: {e}")
	})?;

	let line_length = CONFIG.line_width.to_string();
	let output = Command::new(bin)
		.args(["format", "--quiet", "--line-length", &line_length])
		.arg(&temp_file)
		.output()
		.map_err(|e| command_error(bin, e, "mojo", INSTALL_HINT))?;

	success_status(&output, "mojo")?;
	fs::read_to_string(&temp_file)
		.map_err(|e| format!("mojo formatting: failed to read output: {e}"))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_mojo_metadata_and_env() {
		assert_eq!(MojoFormatter.name(), "mojo");
		assert_eq!(MojoFormatter.extensions(), &["mojo", "🔥"]);
		assert!(!mojo_binary().is_empty());
	}

	#[test]
	fn test_missing_mojo_reports_helpful_error() {
		let err = format_mojo_with_bin(
			"fama-nonexistent-mojo-bin",
			"fn main():\n    var x=1+2\n",
		)
		.unwrap_err();
		assert!(err.contains("was not found in PATH"), "{err}");
		assert!(err.contains(MOJO_PATH_ENV), "{err}");
	}

	#[test]
	fn test_format_mojo_live_if_installed() {
		let bin = mojo_binary();
		if !Command::new(&bin)
			.arg("--version")
			.output()
			.is_ok_and(|o| o.status.success())
		{
			return;
		}
		let once = format_mojo("fn main():\n    var x=1+2\n").unwrap();
		let twice = format_mojo(&once).unwrap();
		assert_eq!(once, twice);
	}
}
