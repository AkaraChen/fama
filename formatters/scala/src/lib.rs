//! Scala formatting via the scalafmt native binary (batch process mode).
//!
//! scalafmt publishes instant-start GraalVM native images on every release;
//! install it on PATH and fama batches files through a single invocation.

use fama_common::FileType;
use fama_process::ProcessFormatter;

const SCALAFMT: ProcessFormatter = ProcessFormatter {
	name: "scalafmt",
	command: "scalafmt",
	args: &[],
	mode: fama_process::ProcessMode::StdinStdout,
	write_editorconfig: false,
};

/// Format Scala source (single file)
pub fn format_scala(_source: &str, _path: &str) -> Result<String, String> {
	Err("scala formatting: not implemented yet".to_string())
}

/// Format a batch of Scala sources with one scalafmt invocation each chunk
pub fn format_batch(contents: &[&str]) -> Vec<Result<String, String>> {
	contents
		.iter()
		.map(|_| Err("scala formatting: not implemented yet".to_string()))
		.collect()
}

/// Format a file based on its file type
pub fn format_file(
	source: &str,
	file_path: &str,
	file_type: FileType,
) -> Result<String, String> {
	match file_type {
		FileType::Scala => format_scala(source, file_path),
		_ => Err(format!(
			"File type {:?} is not supported by the scala formatter",
			file_type
		)),
	}
}
