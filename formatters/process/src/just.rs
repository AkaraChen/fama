//! Justfile formatter via `just --fmt` (process mode).

use crate::process::{command_error, resolve_binary, success_status};
use fama_common::Formatter;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

pub const JUST_PATH_ENV: &str = "FAMA_JUST_PATH";

const JUST_FILENAMES: &[&str] = &["justfile", "Justfile", ".justfile"];

const INSTALL_HINT: &str =
	"Install `just` (https://just.systems) to format justfiles, \
	 or point FAMA_JUST_PATH at the binary.";

/// Formatter implementation for `just` command runner files via `just --fmt`.
pub struct JustFormatter;

impl Formatter for JustFormatter {
	fn name(&self) -> &'static str {
		"just"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["just"]
	}

	fn matches_filename(&self, name: &str) -> bool {
		JUST_FILENAMES.contains(&name)
	}

	fn format(&self, content: &str, _path: &Path) -> Result<String, String> {
		format_just(content)
	}
}

pub fn just_binary() -> String {
	resolve_binary(JUST_PATH_ENV, "just")
}

pub fn format_just(source: &str) -> Result<String, String> {
	format_just_with_bin(&just_binary(), source)
}

fn format_just_with_bin(bin: &str, source: &str) -> Result<String, String> {
	let temp_dir = TempDir::new()
		.map_err(|e| format!("just formatting: failed to create temp dir: {e}"))?;
	let temp_file = temp_dir.path().join("justfile");
	fs::write(&temp_file, source)
		.map_err(|e| format!("just formatting: failed to write temp file: {e}"))?;

	let output = Command::new(bin)
		.args(["--fmt", "--unstable", "--justfile"])
		.arg(&temp_file)
		.output()
		.map_err(|e| command_error(bin, e, "just", INSTALL_HINT))?;

	success_status(&output, "just")?;
	fs::read_to_string(&temp_file)
		.map_err(|e| format!("just formatting: failed to read output: {e}"))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_just_metadata_and_filenames() {
		assert_eq!(JustFormatter.name(), "just");
		assert_eq!(JustFormatter.extensions(), &["just"]);
		for name in ["justfile", "Justfile", ".justfile"] {
			assert!(JustFormatter.matches_filename(name), "{name}");
		}
		assert!(!JustFormatter.matches_filename("Makefile"));
		assert!(!just_binary().is_empty());
	}

	#[test]
	fn test_missing_just_reports_helpful_error() {
		let err =
			format_just_with_bin("fama-nonexistent-just-bin", "build:\n\techo hi\n")
				.unwrap_err();
		assert!(err.contains("was not found in PATH"), "{err}");
	}

	#[test]
	fn test_format_just_live_if_installed() {
		let bin = just_binary();
		if !Command::new(&bin)
			.arg("--version")
			.output()
			.is_ok_and(|o| o.status.success())
		{
			return;
		}
		let once = format_just("build:\n    echo hi\n").unwrap();
		let twice = format_just(&once).unwrap();
		assert_eq!(once, twice);
	}
}
