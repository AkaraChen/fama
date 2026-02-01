//! Shell formatter using mvdan/sh via Go FFI

use fama_common::{FileType, FormatConfig, IndentStyle};
use libc::{c_char, c_uint, size_t};
use std::ffi::{CStr, CString};
use std::slice;

extern "C" {
	fn FormatShell(
		source: *const c_char,
		source_len: size_t,
		indent: c_uint,
	) -> *mut c_char;
	fn FormatShellBatch(
		sources: *const *const c_char,
		lengths: *const size_t,
		count: size_t,
		indent: c_uint,
	) -> *mut *mut c_char;
	fn FreeString(str: *mut c_char);
	fn FreeStringArray(arr: *mut *mut c_char, count: size_t);
}

fn get_indent() -> c_uint {
	let config = FormatConfig::default();
	match config.indent_style {
		IndentStyle::Tabs => 0,
		IndentStyle::Spaces => config.indent_width as c_uint,
	}
}

pub fn format_shell(source: &str, _file_path: &str) -> Result<String, String> {
	let c_source =
		CString::new(source).map_err(|e| format!("Invalid source: {}", e))?;
	let c_result = unsafe {
		FormatShell(c_source.as_ptr(), source.len() as size_t, get_indent())
	};

	if c_result.is_null() {
		return Err("Formatter returned null".to_string());
	}

	let result = unsafe { CStr::from_ptr(c_result) }
		.to_str()
		.map(|s| s.to_string())
		.map_err(|e| format!("Invalid UTF-8: {}", e));

	unsafe { FreeString(c_result) };
	result
}

pub fn format_shell_batch(sources: &[&str]) -> Vec<Result<String, String>> {
	if sources.is_empty() {
		return Vec::new();
	}

	let c_sources: Vec<CString> =
		match sources.iter().map(|s| CString::new(*s)).collect() {
			Ok(v) => v,
			Err(_) => {
				return sources
					.iter()
					.map(|_| Err("Invalid source".to_string()))
					.collect()
			}
		};

	let c_ptrs: Vec<*const c_char> =
		c_sources.iter().map(|s| s.as_ptr()).collect();
	let lengths: Vec<size_t> =
		sources.iter().map(|s| s.len() as size_t).collect();

	let c_results = unsafe {
		FormatShellBatch(
			c_ptrs.as_ptr(),
			lengths.as_ptr(),
			sources.len() as size_t,
			get_indent(),
		)
	};

	if c_results.is_null() {
		return sources
			.iter()
			.map(|_| Err("Formatter returned null".to_string()))
			.collect();
	}

	let results_slice =
		unsafe { slice::from_raw_parts(c_results, sources.len()) };
	let results: Vec<Result<String, String>> = results_slice
		.iter()
		.map(|&c_str| {
			if c_str.is_null() {
				Err("Null result".to_string())
			} else {
				unsafe { CStr::from_ptr(c_str) }
					.to_str()
					.map(|s| s.to_string())
					.map_err(|e| format!("Invalid UTF-8: {}", e))
			}
		})
		.collect();

	unsafe { FreeStringArray(c_results, sources.len() as size_t) };
	results
}

pub fn format_file(
	source: &str,
	file_path: &str,
	file_type: FileType,
) -> Result<String, String> {
	match file_type {
		FileType::Shell => format_shell(source, file_path),
		_ => Err(format!("File type {:?} not supported", file_type)),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_format_shell() {
		let source = "#!/bin/bash\necho \"hello\"  ";
		let result = format_shell(source, "test.sh");
		assert!(result.is_ok());
	}

	#[test]
	fn test_format_batch() {
		let sources =
			vec!["#!/bin/bash\necho \"hello\"", "if true; then echo yes; fi"];
		let results = format_shell_batch(&sources);
		assert_eq!(results.len(), 2);
		assert!(results.iter().all(|r| r.is_ok()));
	}
}
