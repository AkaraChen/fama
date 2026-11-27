//! Go-based formatters via FFI (shell via mvdan/sh, Go via go/format)

use fama_common::{FormatConfig, Formatter, IndentStyle};
use libc::{c_char, c_uint, size_t};
use std::ffi::CStr;
use std::path::Path;
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
	fn FormatHcl(source: *const c_char, source_len: size_t) -> *mut c_char;
	fn FormatHclBatch(
		sources: *const *const c_char,
		lengths: *const size_t,
		count: size_t,
	) -> *mut *mut c_char;
	fn FormatStarlark(
		source: *const c_char,
		source_len: size_t,
		filename: *const c_char,
		filename_len: size_t,
		err_out: *mut *mut c_char,
	) -> *mut c_char;
	fn FormatCue(
		source: *const c_char,
		source_len: size_t,
		use_tabs: c_uint,
		indent_width: c_uint,
		err_out: *mut *mut c_char,
	) -> *mut c_char;
	fn FormatJsonnet(
		source: *const c_char,
		source_len: size_t,
		filename: *const c_char,
		filename_len: size_t,
		indent_width: c_uint,
		err_out: *mut *mut c_char,
	) -> *mut c_char;
	fn FormatRego(
		source: *const c_char,
		source_len: size_t,
		filename: *const c_char,
		filename_len: size_t,
		err_out: *mut *mut c_char,
	) -> *mut c_char;
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
	let c_result = unsafe {
		FormatShell(
			source.as_ptr() as *const c_char,
			source.len() as size_t,
			get_indent(),
		)
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

	let c_ptrs: Vec<*const c_char> =
		sources.iter().map(|s| s.as_ptr() as *const c_char).collect();
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
	let c_result = unsafe {
		FormatGo(source.as_ptr() as *const c_char, source.len() as size_t)
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

pub fn format_go_batch(sources: &[&str]) -> Vec<Result<String, String>> {
	if sources.is_empty() {
		return Vec::new();
	}

	let c_ptrs: Vec<*const c_char> =
		sources.iter().map(|s| s.as_ptr() as *const c_char).collect();
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

pub fn format_hcl(source: &str, _file_path: &str) -> Result<String, String> {
	let c_result = unsafe {
		FormatHcl(source.as_ptr() as *const c_char, source.len() as size_t)
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

pub fn format_hcl_batch(sources: &[&str]) -> Vec<Result<String, String>> {
	if sources.is_empty() {
		return Vec::new();
	}

	let c_ptrs: Vec<*const c_char> =
		sources.iter().map(|s| s.as_ptr() as *const c_char).collect();
	let lengths: Vec<size_t> =
		sources.iter().map(|s| s.len() as size_t).collect();

	let c_results = unsafe {
		FormatHclBatch(
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

/// Convert a `(c_result, err_ptr)` pair returned by a Go FFI function into `Result<String, String>`
/// and free whichever C string was allocated.
unsafe fn take_c_result(
	c_result: *mut c_char,
	c_err: *mut c_char,
) -> Result<String, String> {
	if !c_err.is_null() {
		let msg = unsafe { CStr::from_ptr(c_err) }
			.to_str()
			.unwrap_or("Invalid UTF-8 in formatter error")
			.to_string();
		unsafe { FreeString(c_err) };
		if !c_result.is_null() {
			unsafe { FreeString(c_result) };
		}
		return Err(msg);
	}
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

pub fn format_starlark(
	source: &str,
	file_path: &str,
) -> Result<String, String> {
	let name = if file_path.is_empty() {
		"BUILD.bazel"
	} else {
		file_path
	};
	let mut c_err: *mut c_char = std::ptr::null_mut();
	let c_result = unsafe {
		FormatStarlark(
			source.as_ptr() as *const c_char,
			source.len() as size_t,
			name.as_ptr() as *const c_char,
			name.len() as size_t,
			&mut c_err,
		)
	};
	unsafe { take_c_result(c_result, c_err) }
}

pub fn format_cue(source: &str, _file_path: &str) -> Result<String, String> {
	let config = FormatConfig::default();
	let use_tabs = match config.indent_style {
		IndentStyle::Tabs => 1,
		IndentStyle::Spaces => 0,
	};
	let mut c_err: *mut c_char = std::ptr::null_mut();
	let c_result = unsafe {
		FormatCue(
			source.as_ptr() as *const c_char,
			source.len() as size_t,
			use_tabs,
			config.indent_width as c_uint,
			&mut c_err,
		)
	};
	unsafe { take_c_result(c_result, c_err) }
}

pub fn format_jsonnet(
	source: &str,
	file_path: &str,
) -> Result<String, String> {
	let config = FormatConfig::default();
	let name = if file_path.is_empty() {
		"input.jsonnet"
	} else {
		file_path
	};
	let mut c_err: *mut c_char = std::ptr::null_mut();
	let c_result = unsafe {
		FormatJsonnet(
			source.as_ptr() as *const c_char,
			source.len() as size_t,
			name.as_ptr() as *const c_char,
			name.len() as size_t,
			config.indent_width as c_uint,
			&mut c_err,
		)
	};
	unsafe { take_c_result(c_result, c_err) }
}

pub fn format_rego(source: &str, file_path: &str) -> Result<String, String> {
	let name = if file_path.is_empty() {
		"policy.rego"
	} else {
		file_path
	};
	let mut c_err: *mut c_char = std::ptr::null_mut();
	let c_result = unsafe {
		FormatRego(
			source.as_ptr() as *const c_char,
			source.len() as size_t,
			name.as_ptr() as *const c_char,
			name.len() as size_t,
			&mut c_err,
		)
	};
	unsafe { take_c_result(c_result, c_err) }
}

/// Formatter implementation for Shell, Go, HCL/Terraform, Starlark/Bazel, CUE,
/// Jsonnet, and Rego via Go FFI.
pub struct GoFfiFormatter;

/// Static formatter instance registered by `fama`.
pub static FORMATTER: GoFfiFormatter = GoFfiFormatter;

const STARLARK_FILENAMES: &[&str] = &[
	"BUILD",
	"BUILD.bazel",
	"WORKSPACE",
	"WORKSPACE.bazel",
	"MODULE.bazel",
];

impl Formatter for GoFfiFormatter {
	fn name(&self) -> &'static str {
		"goffi"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&[
			"sh",
			"bash",
			"zsh",
			"go",
			"hcl",
			"tf",
			"tfvars",
			"bzl",
			"star",
			"cue",
			"jsonnet",
			"libsonnet",
			"rego",
		]
	}

	fn matches_filename(&self, name: &str) -> bool {
		STARLARK_FILENAMES.contains(&name)
	}

	fn format(&self, content: &str, path: &Path) -> Result<String, String> {
		let path_str = path.to_str().unwrap_or("");
		match path.extension().and_then(|e| e.to_str()) {
			Some("sh" | "bash" | "zsh") => format_shell(content, path_str),
			Some("go") => format_go(content, path_str),
			Some("hcl" | "tf" | "tfvars") => format_hcl(content, path_str),
			Some("bzl" | "star") => format_starlark(content, path_str),
			Some("cue") => format_cue(content, path_str),
			Some("jsonnet" | "libsonnet") => format_jsonnet(content, path_str),
			Some("rego") => format_rego(content, path_str),
			_ if path
				.file_name()
				.and_then(|n| n.to_str())
				.is_some_and(|n| self.matches_filename(n)) =>
			{
				format_starlark(content, path_str)
			}
			_ => Err(format!("Unsupported goffi file: {}", path.display())),
		}
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
	fn test_format_hcl() {
		let source = r#"resource "aws_instance"   "example" {
ami           = "ami-12345"
    instance_type="t2.micro"
}"#;
		let result = format_hcl(source, "test.hcl");
		assert!(result.is_ok());
		let formatted = result.unwrap();
		// HCL formatter should normalize spacing
		assert!(formatted.contains("instance_type = "));
	}

	#[test]
	fn test_format_hcl_batch() {
		let sources = vec![r#"foo   =    "bar""#, r#"baz={x=1}"#];
		let results = format_hcl_batch(&sources);
		assert_eq!(results.len(), 2);
		assert!(results.iter().all(|r| r.is_ok()));
	}

	#[test]
	fn test_format_starlark_and_bazel_filenames() {
		for name in ["BUILD", "BUILD.bazel", "WORKSPACE", "MODULE.bazel"] {
			assert!(GoFfiFormatter.matches_filename(name), "{name}");
		}

		let messy = "rust_binary(\nname=\"app\",\nsrcs=[\"main.rs\",\"lib.rs\"],\n)\n";
		let once = GoFfiFormatter
			.format(messy, Path::new("BUILD.bazel"))
			.unwrap();
		assert!(once.contains("name = \"app\""), "{once}");
		let twice = GoFfiFormatter
			.format(&once, Path::new("BUILD.bazel"))
			.unwrap();
		assert_eq!(once, twice);

		let bzl = "def   hello(  x,y=1 ):\n    return  x+y\n";
		let formatted_bzl = GoFfiFormatter
			.format(bzl, Path::new("defs.bzl"))
			.unwrap();
		assert!(formatted_bzl.contains("def hello(x, y = 1):"), "{formatted_bzl}");

		assert!(GoFfiFormatter
			.format("def broken(:", Path::new("bad.star"))
			.is_err());
	}

	#[test]
	fn test_format_cue() {
		let messy = "package foo\n\na:  {\nb:1,\nc:  \"hi\"\n}\n";
		let once = GoFfiFormatter.format(messy, Path::new("cfg.cue")).unwrap();
		assert!(once.contains("a: {"), "{once}");
		assert!(once.contains("\tb: 1"), "{once}");
		let twice = GoFfiFormatter.format(&once, Path::new("cfg.cue")).unwrap();
		assert_eq!(once, twice);

		assert!(GoFfiFormatter.format("a: {{{", Path::new("bad.cue")).is_err());
	}

	#[test]
	fn test_format_jsonnet() {
		let messy = "local   x=1;  {a:x,b:[1,2,3]}";
		let once = GoFfiFormatter
			.format(messy, Path::new("main.jsonnet"))
			.unwrap();
		assert!(once.contains("local x = 1;"), "{once}");
		let twice = GoFfiFormatter
			.format(&once, Path::new("main.jsonnet"))
			.unwrap();
		assert_eq!(once, twice);

		assert!(GoFfiFormatter
			.format("local x = ;", Path::new("bad.libsonnet"))
			.is_err());
	}

	#[test]
	fn test_format_rego() {
		let v1 = "package example\ndefault allow:=false\nallow if {input.admin==true}\n";
		let once = GoFfiFormatter
			.format(v1, Path::new("policy.rego"))
			.unwrap();
		assert!(once.contains("default allow := false"), "{once}");
		let twice = GoFfiFormatter
			.format(&once, Path::new("policy.rego"))
			.unwrap();
		assert_eq!(once, twice);

		// Rego v0 fallback
		let v0 = "package legacy\nallow { input.ok == true }\n";
		let formatted_v0 = GoFfiFormatter
			.format(v0, Path::new("legacy.rego"))
			.unwrap();
		assert!(formatted_v0.contains("allow {"), "{formatted_v0}");

		assert!(GoFfiFormatter
			.format("package {{{", Path::new("bad.rego"))
			.is_err());
	}
}
