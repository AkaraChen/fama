//! Zig formatter via FFI (using Zig's std.zig.Ast.Render)

use fama_common::Formatter;
use libc::{c_char, size_t};
use std::ffi::{CStr, CString};
use std::path::Path;

#[repr(C)]
struct FormatResult {
	data: *mut c_char,
	len: size_t,
	error_msg: *const c_char,
}

extern "C" {
	fn zig_fmt(source: *const c_char, source_len: size_t) -> FormatResult;
	fn zig_fmt_free(result: *mut FormatResult);
	fn zig_fmt_version() -> *const c_char;
}

/// Get the version of the Zig formatter
pub fn version() -> &'static str {
	unsafe {
		let ptr = zig_fmt_version();
		if ptr.is_null() {
			"unknown"
		} else {
			CStr::from_ptr(ptr).to_str().unwrap_or("unknown")
		}
	}
}

/// Format Zig source code
pub fn format_zig(source: &str, _file_path: &str) -> Result<String, String> {
	let c_source =
		CString::new(source).map_err(|e| format!("Invalid source: {}", e))?;

	let mut result =
		unsafe { zig_fmt(c_source.as_ptr(), source.len() as size_t) };

	if result.data.is_null() {
		let error = if result.error_msg.is_null() {
			"Unknown error".to_string()
		} else {
			unsafe { CStr::from_ptr(result.error_msg) }
				.to_str()
				.unwrap_or("Unknown error")
				.to_string()
		};
		return Err(error);
	}

	let formatted = unsafe { CStr::from_ptr(result.data) }
		.to_str()
		.map(|s| s.to_string())
		.map_err(|e| format!("Invalid UTF-8: {}", e));

	unsafe { zig_fmt_free(&mut result) };
	formatted
}

/// Formatter implementation for Zig via `zig fmt` FFI.
pub struct ZigFormatter;

/// Static formatter instance registered by `fama`.
pub static FORMATTER: ZigFormatter = ZigFormatter;

impl Formatter for ZigFormatter {
	fn name(&self) -> &'static str {
		"zig"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["zig"]
	}

	fn format(&self, content: &str, path: &Path) -> Result<String, String> {
		format_zig(content, path.to_str().unwrap_or(""))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_version() {
		let v = version();
		assert!(!v.is_empty());
	}

	#[test]
	fn test_format_zig_simple() {
		let source = "const x=1;";
		let result = format_zig(source, "test.zig");
		assert!(result.is_ok());
		let formatted = result.unwrap();
		assert!(formatted.contains("const x = 1;"));
	}

	#[test]
	fn test_format_zig_function() {
		let source = "fn foo()void{return;}";
		let result = format_zig(source, "test.zig");
		assert!(result.is_ok());
		let formatted = result.unwrap();
		assert!(formatted.contains("fn foo() void"));
	}

	#[test]
	fn test_format_zig_already_formatted() {
		let source = "const x = 1;\n";
		let result = format_zig(source, "test.zig");
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), source);
	}

	#[test]
	fn test_format_zig_invalid_syntax() {
		let source = "const x = ;"; // invalid syntax
		let result = format_zig(source, "test.zig");
		assert!(result.is_err());
	}
}
