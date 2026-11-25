//! Nix formatter via the alejandra crate (Rust-native, Unlicense).

use fama_common::FileType;

/// Format a Nix file based on its detected type
pub fn format_nix(_source: &str, _path: &str) -> Result<String, String> {
	Err("nix formatting: not implemented yet".to_string())
}

/// Format a file based on its file type
pub fn format_file(
	source: &str,
	file_path: &str,
	file_type: FileType,
) -> Result<String, String> {
	match file_type {
		FileType::Nix => format_nix(source, file_path),
		_ => Err(format!(
			"File type {:?} is not supported by the nix formatter",
			file_type
		)),
	}
}
