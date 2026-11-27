//! V language formatter via `v fmt` (process mode).

use crate::process::{command_error, resolve_binary, success_status};
use fama_common::Formatter;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

pub const V_PATH_ENV: &str = "FAMA_V_PATH";

const INSTALL_HINT: &str =
	"Install V (`v`, https://vlang.io) to format V files, \
	 or point FAMA_V_PATH at the binary.";

/// Formatter implementation for the V programming language via `v fmt`.
pub struct VFormatter;

impl Formatter for VFormatter {
	fn name(&self) -> &'static str {
		"v"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["v"]
	}

	fn linguist_languages(&self, ext: &str) -> &'static [&'static str] {
		match ext {
			"v" => &["V"],
			_ => &[],
		}
	}

	fn format(&self, content: &str, _path: &Path) -> Result<String, String> {
		format_v(content)
	}
}

pub fn v_binary() -> String {
	resolve_binary(V_PATH_ENV, "v")
}

pub fn format_v(source: &str) -> Result<String, String> {
	format_v_with_bin(&v_binary(), source)
}

fn format_v_with_bin(bin: &str, source: &str) -> Result<String, String> {
	let temp_dir = TempDir::new()
		.map_err(|e| format!("v formatting: failed to create temp dir: {e}"))?;
	let temp_file = temp_dir.path().join("input.v");
	fs::write(&temp_file, source)
		.map_err(|e| format!("v formatting: failed to write temp file: {e}"))?;

	let output = Command::new(bin)
		.arg("fmt")
		.arg(&temp_file)
		.output()
		.map_err(|e| command_error(bin, e, "v", INSTALL_HINT))?;

	success_status(&output, "v")?;
	String::from_utf8(output.stdout)
		.map_err(|e| format!("v fmt output was not valid UTF-8: {e}"))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_v_metadata_and_env() {
		assert_eq!(VFormatter.name(), "v");
		assert_eq!(VFormatter.extensions(), &["v"]);
		assert!(VFormatter.needs_content_check("v"));
		assert!(!VFormatter.needs_content_check("vv"));
		assert_eq!(VFormatter.linguist_languages("v"), &["V"]);
		assert!(VFormatter.linguist_languages("vv").is_empty());
		assert!(!v_binary().is_empty());
	}

	#[test]
	fn test_missing_v_reports_helpful_error() {
		let err = format_v_with_bin(
			"fama-nonexistent-v-bin",
			"fn main() { println('hi') }\n",
		)
		.unwrap_err();
		assert!(err.contains("was not found in PATH"), "{err}");
		assert!(err.contains(V_PATH_ENV), "{err}");
	}

	#[test]
	fn test_format_v_live_if_installed() {
		let bin = v_binary();
		if !Command::new(&bin)
			.arg("--version")
			.output()
			.is_ok_and(|o| o.status.success())
		{
			return;
		}
		let once = format_v("fn main(){println('hi')}\n").unwrap();
		let twice = format_v(&once).unwrap();
		assert_eq!(once, twice);
	}
}
