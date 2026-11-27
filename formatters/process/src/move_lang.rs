//! Move smart-contract formatter via `movefmt` (process mode).

use crate::process::{command_error, resolve_binary, success_status};
use fama_common::Formatter;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

pub const MOVEFMT_PATH_ENV: &str = "FAMA_MOVEFMT_PATH";

const INSTALL_HINT: &str =
	"Install `movefmt` to format Move files, \
	 or point FAMA_MOVEFMT_PATH at the binary.";

/// Formatter implementation for Move via `movefmt`.
pub struct MoveFormatter;

impl Formatter for MoveFormatter {
	fn name(&self) -> &'static str {
		"move"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["move"]
	}

	fn format(&self, content: &str, _path: &Path) -> Result<String, String> {
		format_move(content)
	}
}

pub fn movefmt_binary() -> String {
	resolve_binary(MOVEFMT_PATH_ENV, "movefmt")
}

pub fn format_move(source: &str) -> Result<String, String> {
	format_move_with_bin(&movefmt_binary(), source)
}

fn format_move_with_bin(bin: &str, source: &str) -> Result<String, String> {
	let temp_dir = TempDir::new()
		.map_err(|e| format!("move formatting: failed to create temp dir: {e}"))?;
	let temp_file = temp_dir.path().join("input.move");
	fs::write(&temp_file, source)
		.map_err(|e| format!("move formatting: failed to write temp file: {e}"))?;

	let output = Command::new(bin)
		.args(["--emit", "stdout"])
		.arg(&temp_file)
		.output()
		.map_err(|e| command_error(bin, e, "movefmt", INSTALL_HINT))?;

	success_status(&output, "movefmt")?;
	String::from_utf8(output.stdout)
		.map_err(|e| format!("movefmt output was not valid UTF-8: {e}"))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_move_metadata_and_env() {
		assert_eq!(MoveFormatter.name(), "move");
		assert_eq!(MoveFormatter.extensions(), &["move"]);
		assert!(!movefmt_binary().is_empty());
	}

	#[test]
	fn test_missing_movefmt_reports_helpful_error() {
		let err = format_move_with_bin(
			"fama-nonexistent-movefmt-bin",
			"module 0x1::m {}",
		)
		.unwrap_err();
		assert!(err.contains("was not found in PATH"), "{err}");
	}

	#[test]
	fn test_format_move_live_if_installed() {
		let bin = movefmt_binary();
		if !Command::new(&bin)
			.arg("--version")
			.output()
			.is_ok_and(|o| o.status.success())
		{
			return;
		}
		let once = format_move("module 0x1::m{public fun f(){}}\n").unwrap();
		let twice = format_move(&once).unwrap();
		assert_eq!(once, twice);
	}
}
