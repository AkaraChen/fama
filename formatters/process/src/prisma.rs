//! Prisma Schema formatter via `prisma-fmt` (process mode).

use crate::process::{resolve_binary, run_stdin};
use fama_common::Formatter;
use std::path::Path;

pub const PRISMA_FMT_PATH_ENV: &str = "FAMA_PRISMA_FMT_PATH";

const INSTALL_HINT: &str =
	"Install `prisma-fmt` to format Prisma schema files, \
	 or point FAMA_PRISMA_FMT_PATH at the binary.";

/// Formatter implementation for Prisma schemas via `prisma-fmt`.
pub struct PrismaFormatter;

impl Formatter for PrismaFormatter {
	fn name(&self) -> &'static str {
		"prisma"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["prisma"]
	}

	fn format(&self, content: &str, _path: &Path) -> Result<String, String> {
		format_prisma(content)
	}
}

pub fn prisma_fmt_binary() -> String {
	resolve_binary(PRISMA_FMT_PATH_ENV, "prisma-fmt")
}

pub fn format_prisma(source: &str) -> Result<String, String> {
	let bin = prisma_fmt_binary();
	run_stdin(&bin, &["format"], source, "prisma-fmt", INSTALL_HINT)
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::process::Command;

	#[test]
	fn test_prisma_metadata_and_env() {
		assert_eq!(PrismaFormatter.name(), "prisma");
		assert_eq!(PrismaFormatter.extensions(), &["prisma"]);
		assert!(!prisma_fmt_binary().is_empty());
	}

	#[test]
	fn test_missing_prisma_reports_helpful_error() {
		let err = run_stdin(
			"fama-nonexistent-prisma-fmt-bin",
			&["format"],
			"model User { id Int @id }",
			"prisma-fmt",
			INSTALL_HINT,
		)
		.unwrap_err();
		assert!(err.contains("was not found in PATH"), "{err}");
	}

	#[test]
	fn test_format_prisma_live_if_installed() {
		let bin = prisma_fmt_binary();
		if !Command::new(&bin)
			.arg("--version")
			.output()
			.is_ok_and(|o| o.status.success())
		{
			return;
		}
		let once =
			format_prisma("model User {\nid Int @id\n}\n").unwrap();
		let twice = format_prisma(&once).unwrap();
		assert_eq!(once, twice);
	}
}
