//! Meson build file formatter via `meson format` (process mode).

use crate::process::{resolve_binary, run_stdin};
use fama_common::Formatter;
use std::path::Path;

pub const MESON_PATH_ENV: &str = "FAMA_MESON_PATH";

const MESON_FILENAMES: &[&str] =
	&["meson.build", "meson.options", "meson_options.txt"];

const INSTALL_HINT: &str =
	"Install Meson >= 1.5 (`meson`, https://mesonbuild.com) to format Meson files, \
	 or point FAMA_MESON_PATH at the binary.";

/// Formatter implementation for Meson files (`meson.build`, `meson.options`,
/// `meson_options.txt`) via `meson format`.
pub struct MesonFormatter;

impl Formatter for MesonFormatter {
	fn name(&self) -> &'static str {
		"meson"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&[]
	}

	fn matches_filename(&self, name: &str) -> bool {
		MESON_FILENAMES.contains(&name)
	}

	fn format(&self, content: &str, _path: &Path) -> Result<String, String> {
		format_meson(content)
	}
}

pub fn meson_binary() -> String {
	resolve_binary(MESON_PATH_ENV, "meson")
}

pub fn format_meson(source: &str) -> Result<String, String> {
	let bin = meson_binary();
	run_stdin(&bin, &["format", "-"], source, "meson", INSTALL_HINT)
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::process::Command;

	#[test]
	fn test_meson_metadata_and_filenames() {
		assert_eq!(MesonFormatter.name(), "meson");
		assert!(MesonFormatter.extensions().is_empty());
		for name in ["meson.build", "meson.options", "meson_options.txt"] {
			assert!(MesonFormatter.matches_filename(name), "{name}");
		}
		assert!(!MesonFormatter.matches_filename("CMakeLists.txt"));
		assert!(!MesonFormatter.matches_filename("notes.txt"));
		assert!(!meson_binary().is_empty());
	}

	#[test]
	fn test_missing_meson_reports_helpful_error() {
		let err = run_stdin(
			"fama-nonexistent-meson-bin",
			&["format", "-"],
			"project('demo','c')\n",
			"meson",
			INSTALL_HINT,
		)
		.unwrap_err();
		assert!(err.contains("was not found in PATH"), "{err}");
		assert!(err.contains(MESON_PATH_ENV), "{err}");
	}

	#[test]
	fn test_format_meson_live_if_installed() {
		let bin = meson_binary();
		if !Command::new(&bin)
			.args(["format", "--help"])
			.output()
			.is_ok_and(|o| o.status.success())
		{
			return;
		}
		let once = format_meson("project('demo','c')\n").unwrap();
		let twice = format_meson(&once).unwrap();
		assert_eq!(once, twice);
	}
}
