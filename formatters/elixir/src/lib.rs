//! Elixir formatting via `mix format` (the built-in, canonical formatter),
//! through the process batch mode.

use fama_common::FileType;
use fama_process::ProcessFormatter;

const MIX: ProcessFormatter = ProcessFormatter {
	name: "mix",
	command: "mix",
	args: &["format", "-"],
	mode: fama_process::ProcessMode::StdinStdout,
	write_editorconfig: false,
};

/// Format Elixir source (single file)
pub fn format_elixir(_source: &str, _path: &str) -> Result<String, String> {
	Err("elixir formatting: not implemented yet".to_string())
}

/// Format a batch of Elixir sources
pub fn format_batch(contents: &[&str]) -> Vec<Result<String, String>> {
	contents
		.iter()
		.map(|_| Err("elixir formatting: not implemented yet".to_string()))
		.collect()
}

/// Format a file based on its file type
pub fn format_file(
	source: &str,
	file_path: &str,
	file_type: FileType,
) -> Result<String, String> {
	match file_type {
		FileType::Elixir => format_elixir(source, file_path),
		_ => Err(format!(
			"File type {:?} is not supported by the elixir formatter",
			file_type
		)),
	}
}
