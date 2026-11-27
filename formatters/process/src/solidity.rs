//! Solidity formatter via `forge fmt` (process mode).

use crate::process::{resolve_binary, run_stdin};
use fama_common::Formatter;
use std::path::Path;

pub const FORGE_PATH_ENV: &str = "FAMA_FORGE_PATH";

const INSTALL_HINT: &str =
	"Install Foundry (`forge`, https://getfoundry.sh) to format Solidity files, \
	 or point FAMA_FORGE_PATH at the binary.";

/// Formatter implementation for Solidity via `forge fmt`.
pub struct SolidityFormatter;

impl Formatter for SolidityFormatter {
	fn name(&self) -> &'static str {
		"solidity"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["sol"]
	}

	fn format(&self, content: &str, _path: &Path) -> Result<String, String> {
		format_solidity(content)
	}
}

pub fn forge_binary() -> String {
	resolve_binary(FORGE_PATH_ENV, "forge")
}

pub fn format_solidity(source: &str) -> Result<String, String> {
	let bin = forge_binary();
	run_stdin(&bin, &["fmt", "--raw", "-"], source, "forge", INSTALL_HINT)
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::process::Command;

	#[test]
	fn test_solidity_metadata_and_env() {
		assert_eq!(SolidityFormatter.name(), "solidity");
		assert_eq!(SolidityFormatter.extensions(), &["sol"]);
		assert!(!forge_binary().is_empty());
	}

	#[test]
	fn test_missing_forge_reports_helpful_error() {
		let err = run_stdin(
			"fama-nonexistent-forge-bin",
			&["fmt", "--raw", "-"],
			"contract A {}",
			"forge",
			INSTALL_HINT,
		)
		.unwrap_err();
		assert!(err.contains("was not found in PATH"), "{err}");
	}

	#[test]
	fn test_format_solidity_live_if_installed() {
		let bin = forge_binary();
		if !Command::new(&bin)
			.arg("--version")
			.output()
			.is_ok_and(|o| o.status.success())
		{
			return;
		}
		let input = "pragma solidity ^0.8.0;\ncontract A{uint256 x;}\n";
		let once = format_solidity(input).unwrap();
		let twice = format_solidity(&once).unwrap();
		assert_eq!(once, twice);
	}
}
