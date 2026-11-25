//! CMake formatter via the cmakefmt-rust crate (Rust-native cmake-format
//! successor).

use fama_common::FileType;

/// Format a CMake file based on its detected type
pub fn format_cmake(_source: &str, _path: &str) -> Result<String, String> {
	Err("cmake formatting: not implemented yet".to_string())
}

/// Format a file based on its file type
pub fn format_file(
	source: &str,
	file_path: &str,
	file_type: FileType,
) -> Result<String, String> {
	match file_type {
		FileType::CMake => format_cmake(source, file_path),
		_ => Err(format!(
			"File type {:?} is not supported by the cmake formatter",
			file_type
		)),
	}
}
