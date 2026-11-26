//! Rust formatter using rustfmt
//!
//! This module provides Rust code formatting functionality via the rustfmt
//! formatter, using the rust-format crate for a clean library API, plus a
//! batched mode that formats many files with a single rustfmt invocation.

use fama_common::{FileType, CONFIG};
use rust_format::{Config, Formatter, RustFmt};
use std::io::Write;
use std::process::Command;

// Module-level constants - pre-converted config values
const RUSTFMT_HARD_TABS: &str =
	if matches!(CONFIG.indent_style, fama_common::IndentStyle::Tabs) {
		"true"
	} else {
		"false"
	};
// Note: These need to be string literals for const, so we use fixed values
// matching CONFIG defaults. If CONFIG changes, update these.
const RUSTFMT_TAB_SPACES: &str = "4";
const RUSTFMT_MAX_WIDTH: &str = "80";
const RUSTFMT_NEWLINE_STYLE: &str = match CONFIG.line_ending {
	fama_common::LineEnding::Lf => "Unix",
	fama_common::LineEnding::Crlf => "Windows",
};

/// The `--config` options shared by every rustfmt invocation, matching the
/// options that [`format_rust`] passes via rust-format.
fn config_options() -> String {
	format!(
		"hard_tabs={},tab_spaces={},max_width={},newline_style={}",
		RUSTFMT_HARD_TABS, RUSTFMT_TAB_SPACES, RUSTFMT_MAX_WIDTH, RUSTFMT_NEWLINE_STYLE
	)
}

/// The rustfmt binary to invoke, honoring `RUSTFMT` like rust-format does
fn rustfmt_binary() -> std::ffi::OsString {
	std::env::var_os("RUSTFMT")
		.unwrap_or_else(|| std::ffi::OsString::from("rustfmt"))
}

/// Format Rust source code
///
/// # Arguments
/// * `source` - The Rust source code to format
/// * `file_path` - The file path (used for error reporting, currently unused)
///
/// # Returns
/// The formatted Rust source code, or an error message if formatting fails.
pub fn format_rust(source: &str, _file_path: &str) -> Result<String, String> {
	let config = Config::new_str()
		.option("hard_tabs", RUSTFMT_HARD_TABS)
		.option("tab_spaces", RUSTFMT_TAB_SPACES)
		.option("max_width", RUSTFMT_MAX_WIDTH)
		.option("newline_style", RUSTFMT_NEWLINE_STYLE);

	let formatter = RustFmt::from_config(config);

	formatter
		.format_str(source)
		.map_err(|e| format!("rustfmt error: {}", e))
}

/// Format a file based on its file type
pub fn format_file(
	source: &str,
	file_path: &str,
	file_type: FileType,
) -> Result<String, String> {
	match file_type {
		FileType::Rust => format_rust(source, file_path),
		_ => Err(format!(
			"File type {:?} is not supported by rust-formatter",
			file_type
		)),
	}
}

/// Format multiple Rust sources with a single rustfmt invocation.
///
/// Each rustfmt process spawn costs far more than the formatting itself for
/// typical files, so batching amortizes it across the whole chunk. Sources are
/// written to a temporary directory, formatted in place by one process, and
/// read back; the returned results line up 1:1 with the inputs.
///
/// If the batch invocation fails (any file with invalid syntax fails the whole
/// batch), the chunk falls back to per-file formatting so errors stay isolated
/// to the files that actually own them. If rustfmt cannot be spawned at all,
/// every input gets the same error.
///
/// # Arguments
/// * `contents` - The Rust sources to format
///
/// # Returns
/// One result per input, in input order
pub fn format_batch(contents: &[&str]) -> Vec<Result<String, String>> {
	if contents.is_empty() {
		return Vec::new();
	}
	if contents.len() == 1 {
		return vec![format_rust(contents[0], "input.rs")];
	}

	match format_batch_inner(contents) {
		Ok(results) => results,
		// Batch failure (e.g. a syntax error in one file aborted the batch):
		// bisect in parallel so healthy halves format in one spawn each instead
		// of falling back to N serial spawns.
		Err(BatchFailure::NeedsIsolate) => {
			let mid = contents.len() / 2;
			let (left, right) = std::thread::scope(|s| {
				let h = s.spawn(|| format_batch(&contents[..mid]));
				let right = format_batch(&contents[mid..]);
				let left = h.join().expect("rustfmt bisect thread panicked");
				(left, right)
			});
			let mut combined = left;
			combined.extend(right);
			combined
		}
		Err(BatchFailure::ToolMissing(err)) => {
			(0..contents.len()).map(|_| Err(err.clone())).collect()
		}
	}
}

enum BatchFailure {
	NeedsIsolate,
	ToolMissing(String),
}

/// Write sources to temp files and format them all with one rustfmt process
fn format_batch_inner(
	contents: &[&str],
) -> Result<Vec<Result<String, String>>, BatchFailure> {
	let temp_dir = tempfile::tempdir().map_err(|_| BatchFailure::NeedsIsolate)?;

	let mut paths = Vec::with_capacity(contents.len());
	for (index, content) in contents.iter().enumerate() {
		let path = temp_dir.path().join(format!("input_{}.rs", index));
		fs_write(&path, content).map_err(|_| BatchFailure::NeedsIsolate)?;
		paths.push(path);
	}

	// Same arguments rust-format passes (--edition 2021 + --config), with the
	// default in-place emit mode that multi-file invocation requires.
	let output = Command::new(rustfmt_binary())
		.arg("--edition")
		.arg("2021")
		.arg("--config")
		.arg(config_options())
		.args(&paths)
		.output()
		.map_err(|e| {
			if e.kind() == std::io::ErrorKind::NotFound {
				BatchFailure::ToolMissing(format!("Failed to run rustfmt: {e}"))
			} else {
				BatchFailure::NeedsIsolate
			}
		})?;

	if !output.status.success() {
		return Err(BatchFailure::NeedsIsolate);
	}

	Ok(paths.iter().map(|path| fs_read(path)).collect())
}

fn fs_write(path: &std::path::Path, content: &str) -> Result<(), String> {
	std::fs::File::create(path)
		.and_then(|mut file| file.write_all(content.as_bytes()))
		.map_err(|e| format!("Failed to write {}: {}", path.display(), e))
}

fn fs_read(path: &std::path::Path) -> Result<String, String> {
	std::fs::read_to_string(path)
		.map_err(|e| format!("Failed to read rustfmt output {}: {}", path.display(), e))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_format_simple_function() {
		let source = r#"fn main(){println!("Hello");}"#;
		let result = format_rust(source, "test.rs").unwrap();

		// Check that formatting occurred (should have proper indentation)
		assert!(result.contains("fn main()"));
		assert!(result.contains("println!"));
	}

	#[test]
	fn test_format_batch_matches_per_file() {
		let sources = [
			"fn a(){let x=1;}",
			"fn b(){}",
			"mod c { pub fn d(){} }",
		];
		let batched = format_batch(&sources);

		assert_eq!(batched.len(), sources.len());
		for (source, result) in sources.iter().zip(&batched) {
			let formatted = result
				.as_ref()
				.unwrap_or_else(|e| panic!("batch failed: {e}"));
			let expected =
				format_rust(source, "input.rs").unwrap_or_else(|e| panic!("{e}"));
			assert_eq!(
				formatted, &expected,
				"batch output differs from per-file output"
			);
		}
	}

	#[test]
	fn test_format_batch_single_and_empty() {
		assert!(format_batch(&[]).is_empty());

		let single = format_batch(&["fn a(){let x=1;}"]);
		assert_eq!(single.len(), 1);
		assert!(single[0].as_ref().unwrap().contains("fn a()"));
	}

	#[test]
	fn test_format_batch_isolates_invalid_source() {
		// rustfmt fails the whole batch on a parse error; the fallback must
		// isolate the error to the offending file only
		let sources = [
			"fn a(){let x=1;}",
			"this is not rust at all {{{",
			"fn b(){}",
		];
		let batched = format_batch(&sources);

		assert_eq!(batched.len(), sources.len());
		assert!(batched[0].is_ok(), "valid file a must succeed");
		assert!(batched[1].is_err(), "invalid file must fail");
		assert!(batched[2].is_ok(), "valid file b must succeed");
	}

	#[test]
	fn test_format_batch_preserves_order() {
		let names = ["zero", "one", "two", "three"];
		let sources: Vec<String> = names
			.iter()
			.map(|name| format!("fn {name}(){{}}"))
			.collect();
		let refs: Vec<&str> = sources.iter().map(String::as_str).collect();
		let batched = format_batch(&refs);

		assert_eq!(batched.len(), names.len());
		for (name, result) in names.iter().zip(&batched) {
			let formatted = result.as_ref().unwrap();
			assert!(
				formatted.contains(&format!("fn {name}()")),
				"order mismatch: expected {name}, got: {formatted}"
			);
		}
	}

	#[test]
	fn test_format_file_with_rust() {
		let source = r#"fn main(){println!("Hello");}"#;
		let result = format_file(source, "test.rs", FileType::Rust).unwrap();

		// Check that formatting occurred
		assert!(result.contains("fn main()"));
		assert!(result.contains("println!"));
	}

	#[test]
	fn test_format_file_with_unsupported_type() {
		let source = "test";
		let result = format_file(source, "test.js", FileType::JavaScript);
		assert!(result.is_err());
	}
}
