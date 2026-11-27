//! QML formatter via Qt's `qmlformat` (process mode).

use crate::process::{command_error, resolve_binary, success_status};
use fama_common::{FormatConfig, Formatter, IndentStyle, LineEnding, CONFIG};
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

pub const QMLFORMAT_PATH_ENV: &str = "FAMA_QMLFORMAT_PATH";

const INSTALL_HINT: &str =
	"Install Qt 6 (`qmlformat`, https://doc.qt.io/qt-6/qtqml-tooling-qmlformat.html) \
	 to format QML files, or point FAMA_QMLFORMAT_PATH at the binary.";

/// Formatter implementation for QML via `qmlformat`.
pub struct QmlFormatter;

impl Formatter for QmlFormatter {
	fn name(&self) -> &'static str {
		"qml"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["qml"]
	}

	fn format(&self, content: &str, _path: &Path) -> Result<String, String> {
		format_qml(content)
	}
}

pub fn qmlformat_binary() -> String {
	resolve_binary(QMLFORMAT_PATH_ENV, "qmlformat")
}

fn build_qmlformat_args(config: &FormatConfig) -> Vec<String> {
	let mut args = vec!["--ignore-settings".to_string()];
	match config.indent_style {
		IndentStyle::Tabs => args.push("--tabs".to_string()),
		IndentStyle::Spaces => {
			args.push("--indent-width".to_string());
			args.push(config.indent_width.to_string());
		}
	}
	args.push("--newline".to_string());
	args.push(
		match config.line_ending {
			LineEnding::Lf => "unix",
			LineEnding::Crlf => "windows",
		}
		.to_string(),
	);
	args
}

pub fn format_qml(source: &str) -> Result<String, String> {
	format_qml_with_bin(&qmlformat_binary(), source)
}

fn format_qml_with_bin(bin: &str, source: &str) -> Result<String, String> {
	let temp_dir = TempDir::new().map_err(|e| {
		format!("qml formatting: failed to create temp dir: {e}")
	})?;
	let temp_file = temp_dir.path().join("input.qml");
	fs::write(&temp_file, source).map_err(|e| {
		format!("qml formatting: failed to write temp file: {e}")
	})?;

	let output = Command::new(bin)
		.args(build_qmlformat_args(&CONFIG))
		.arg(&temp_file)
		.output()
		.map_err(|e| command_error(bin, e, "qmlformat", INSTALL_HINT))?;

	success_status(&output, "qmlformat")?;
	String::from_utf8(output.stdout)
		.map_err(|e| format!("qmlformat output was not valid UTF-8: {e}"))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_qml_metadata_and_args() {
		assert_eq!(QmlFormatter.name(), "qml");
		assert_eq!(QmlFormatter.extensions(), &["qml"]);
		assert!(!qmlformat_binary().is_empty());

		let tab_args = build_qmlformat_args(&CONFIG);
		assert!(tab_args.contains(&"--ignore-settings".to_string()));
		assert!(tab_args.contains(&"--tabs".to_string()));
		assert!(tab_args.windows(2).any(|w| w == ["--newline", "unix"]));

		let space_cfg = FormatConfig {
			indent_style: IndentStyle::Spaces,
			indent_width: 2,
			line_ending: LineEnding::Crlf,
			..CONFIG
		};
		let space_args = build_qmlformat_args(&space_cfg);
		assert!(!space_args.contains(&"--tabs".to_string()));
		assert!(space_args.windows(2).any(|w| w == ["--indent-width", "2"]));
		assert!(space_args.windows(2).any(|w| w == ["--newline", "windows"]));
	}

	#[test]
	fn test_missing_qmlformat_reports_helpful_error() {
		let err = format_qml_with_bin(
			"fama-nonexistent-qmlformat-bin",
			"import QtQuick\nItem { width: 100 }\n",
		)
		.unwrap_err();
		assert!(err.contains("was not found in PATH"), "{err}");
		assert!(err.contains(QMLFORMAT_PATH_ENV), "{err}");
	}

	#[test]
	fn test_format_qml_live_if_installed() {
		let bin = qmlformat_binary();
		if !Command::new(&bin)
			.arg("--version")
			.output()
			.is_ok_and(|o| o.status.success())
		{
			return;
		}
		let once =
			format_qml("import QtQuick\nItem{width:100;height:200}\n").unwrap();
		let twice = format_qml(&once).unwrap();
		assert_eq!(once, twice);
	}
}
