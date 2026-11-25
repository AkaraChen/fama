//! R formatting via Air (posit-dev/air), the Rust-native R formatter,
//! through the process batch mode.

use fama_common::FileType;
use fama_process::ProcessFormatter;

const AIR: ProcessFormatter = ProcessFormatter {
	name: "air",
	command: "air",
	args: &[],
	mode: fama_process::ProcessMode::StdinStdout,
	write_editorconfig: false,
};

/// Format R source (single file)
pub fn format_r(_source: &str, _path: &str) -> Result<String, String> {
	Err("r formatting: not implemented yet".to_string())
}

/// Format a batch of R sources
pub fn format_batch(contents: &[&str]) -> Vec<Result<String, String>> {
	contents
		.iter()
		.map(|_| Err("r formatting: not implemented yet".to_string()))
		.collect()
}

/// Format a file based on its file type
pub fn format_file(
	source: &str,
	file_path: &str,
	file_type: FileType,
) -> Result<String, String> {
	match file_type {
		FileType::R => format_r(source, file_path),
		_ => Err(format!(
			"File type {:?} is not supported by the r formatter",
			file_type
		)),
	}
}
