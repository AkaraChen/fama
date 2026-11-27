//! Azure Bicep formatter via `bicep format` (process mode).

use crate::process::{command_error, resolve_binary, success_status};
use fama_common::{FormatConfig, Formatter, IndentStyle, LineEnding, CONFIG};
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

pub const BICEP_PATH_ENV: &str = "FAMA_BICEP_PATH";

const INSTALL_HINT: &str =
	"Install Azure Bicep CLI (`bicep`, https://learn.microsoft.com/azure/azure-resource-manager/bicep/install) \
	 to format Bicep files, or point FAMA_BICEP_PATH at the binary.";

/// Formatter implementation for Azure Bicep (`.bicep`, `.bicepparam`) via
/// `bicep format`.
pub struct BicepFormatter;

impl Formatter for BicepFormatter {
	fn name(&self) -> &'static str {
		"bicep"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["bicep", "bicepparam"]
	}

	fn format(&self, content: &str, path: &Path) -> Result<String, String> {
		format_bicep(content, path)
	}
}

pub fn bicep_binary() -> String {
	resolve_binary(BICEP_PATH_ENV, "bicep")
}

fn build_bicep_args(config: &FormatConfig) -> Vec<String> {
	let mut args = vec!["--stdout".to_string(), "--indent-kind".to_string()];
	match config.indent_style {
		IndentStyle::Tabs => args.push("Tab".to_string()),
		IndentStyle::Spaces => {
			args.push("Space".to_string());
			args.push("--indent-size".to_string());
			args.push(config.indent_width.to_string());
		}
	}
	args.push("--newline-kind".to_string());
	args.push(
		match config.line_ending {
			LineEnding::Lf => "LF",
			LineEnding::Crlf => "CRLF",
		}
		.to_string(),
	);
	args.push("--insert-final-newline".to_string());
	args
}

fn temp_filename(path: &Path) -> &'static str {
	if path.extension().and_then(|e| e.to_str()) == Some("bicepparam") {
		"input.bicepparam"
	} else {
		"input.bicep"
	}
}

pub fn format_bicep(source: &str, path: &Path) -> Result<String, String> {
	format_bicep_with_bin(&bicep_binary(), source, path)
}

fn format_bicep_with_bin(
	bin: &str,
	source: &str,
	path: &Path,
) -> Result<String, String> {
	let temp_dir = TempDir::new().map_err(|e| {
		format!("bicep formatting: failed to create temp dir: {e}")
	})?;
	let temp_file = temp_dir.path().join(temp_filename(path));
	fs::write(&temp_file, source).map_err(|e| {
		format!("bicep formatting: failed to write temp file: {e}")
	})?;

	let output = Command::new(bin)
		.arg("format")
		.arg(&temp_file)
		.args(build_bicep_args(&CONFIG))
		.output()
		.map_err(|e| command_error(bin, e, "bicep", INSTALL_HINT))?;

	success_status(&output, "bicep")?;
	String::from_utf8(output.stdout)
		.map_err(|e| format!("bicep output was not valid UTF-8: {e}"))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_bicep_metadata_and_args() {
		assert_eq!(BicepFormatter.name(), "bicep");
		assert_eq!(BicepFormatter.extensions(), &["bicep", "bicepparam"]);
		assert!(!bicep_binary().is_empty());
		assert_eq!(temp_filename(Path::new("main.bicep")), "input.bicep");
		assert_eq!(
			temp_filename(Path::new("prod.bicepparam")),
			"input.bicepparam"
		);

		let tab_args = build_bicep_args(&CONFIG);
		assert!(tab_args.contains(&"--stdout".to_string()));
		assert!(tab_args.windows(2).any(|w| w == ["--indent-kind", "Tab"]));
		assert!(!tab_args.contains(&"--indent-size".to_string()));
		assert!(tab_args
			.windows(2)
			.any(|w| w == ["--newline-kind", "LF"]));
		assert!(tab_args.contains(&"--insert-final-newline".to_string()));

		let space_cfg = FormatConfig {
			indent_style: IndentStyle::Spaces,
			indent_width: 2,
			line_ending: LineEnding::Crlf,
			..CONFIG
		};
		let space_args = build_bicep_args(&space_cfg);
		assert!(space_args
			.windows(2)
			.any(|w| w == ["--indent-kind", "Space"]));
		assert!(space_args.windows(2).any(|w| w == ["--indent-size", "2"]));
		assert!(space_args
			.windows(2)
			.any(|w| w == ["--newline-kind", "CRLF"]));
	}

	#[test]
	fn test_missing_bicep_reports_helpful_error() {
		let err = format_bicep_with_bin(
			"fama-nonexistent-bicep-bin",
			"param location string = 'eastus'\n",
			Path::new("main.bicep"),
		)
		.unwrap_err();
		assert!(err.contains("was not found in PATH"), "{err}");
		assert!(err.contains(BICEP_PATH_ENV), "{err}");
	}

	#[test]
	fn test_format_bicep_live_if_installed() {
		let bin = bicep_binary();
		if !Command::new(&bin)
			.arg("--version")
			.output()
			.is_ok_and(|o| o.status.success())
		{
			return;
		}
		let path = Path::new("main.bicep");
		let once = format_bicep("param   location   string = 'eastus'\n", path)
			.unwrap();
		let twice = format_bicep(&once, path).unwrap();
		assert_eq!(once, twice);
	}
}
