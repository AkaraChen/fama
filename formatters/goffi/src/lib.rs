//! Go-based formatters via FFI (shell via mvdan/sh, Go via go/format)

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
	fn FormatGo(source: *const c_char, source_len: size_t) -> *mut c_char;
	fn FormatGoBatch(
		sources: *const *const c_char,
		lengths: *const size_t,
		count: size_t,
	) -> *mut *mut c_char;
	fn FormatProto(source: *const c_char, source_len: size_t) -> *mut c_char;
	fn FormatProtoBatch(
		sources: *const *const c_char,
		lengths: *const size_t,
		count: size_t,
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

pub fn format_go(source: &str, _file_path: &str) -> Result<String, String> {
	let c_source =
		CString::new(source).map_err(|e| format!("Invalid source: {}", e))?;
	let c_result =
		unsafe { FormatGo(c_source.as_ptr(), source.len() as size_t) };

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

pub fn format_go_batch(sources: &[&str]) -> Vec<Result<String, String>> {
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
		FormatGoBatch(
			c_ptrs.as_ptr(),
			lengths.as_ptr(),
			sources.len() as size_t,
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

pub fn format_proto(source: &str, _file_path: &str) -> Result<String, String> {
	let c_source =
		CString::new(source).map_err(|e| format!("Invalid source: {}", e))?;
	let c_result =
		unsafe { FormatProto(c_source.as_ptr(), source.len() as size_t) };

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

pub fn format_proto_batch(sources: &[&str]) -> Vec<Result<String, String>> {
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
		FormatProtoBatch(
			c_ptrs.as_ptr(),
			lengths.as_ptr(),
			sources.len() as size_t,
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
		FileType::Go => format_go(source, file_path),
		FileType::Proto => format_proto(source, file_path),
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
	fn test_format_shell_batch() {
		let sources =
			vec!["#!/bin/bash\necho \"hello\"", "if true; then echo yes; fi"];
		let results = format_shell_batch(&sources);
		assert_eq!(results.len(), 2);
		assert!(results.iter().all(|r| r.is_ok()));
	}

	#[test]
	fn test_format_go() {
		let source =
			"package main\n\nfunc main() {\nfmt.Println(  \"hello\"  )\n}\n";
		let result = format_go(source, "test.go");
		assert!(result.is_ok());
		let formatted = result.unwrap();
		// gofmt should normalize spacing
		assert!(formatted.contains("fmt.Println(\"hello\")"));
	}

	#[test]
	fn test_format_go_already_formatted() {
		let source =
			"package main\n\nfunc main() {\n\tfmt.Println(\"hello\")\n}\n";
		let result = format_go(source, "test.go");
		assert!(result.is_ok());
	}

	#[test]
	fn test_format_go_batch() {
		let sources =
			vec!["package main\nfunc main() { }", "package foo\nvar x=1"];
		let results = format_go_batch(&sources);
		assert_eq!(results.len(), 2);
		assert!(results.iter().all(|r| r.is_ok()));
	}

	#[test]
	fn test_format_proto() {
		let source = r#"syntax="proto3";
package example;
message User{string name=1;int32 age=2;}"#;
		let result = format_proto(source, "test.proto");
		assert!(result.is_ok());
		let formatted = result.unwrap();
		assert!(formatted.contains("syntax = \"proto3\";"));
		assert!(formatted.contains("message User {"));
	}

	#[test]
	fn test_format_proto_batch() {
		let sources = vec![
			"syntax=\"proto3\";\nmessage A{string x=1;}",
			"syntax=\"proto3\";\nenum Status{OK=0;ERROR=1;}",
		];
		let results = format_proto_batch(&sources);
		assert_eq!(results.len(), 2);
		assert!(results.iter().all(|r| r.is_ok()));
	}
}
