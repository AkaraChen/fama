//! Shell formatter using mvdan/sh via Go FFI
//!
//! This module provides Shell script formatting functionality via the
//! mvdan/sh Go library, accessed through a C shared library (CGO).

use fama_common::{FileType, FormatConfig, IndentStyle};
use libc::{c_char, c_uint, size_t};
use std::ffi::{CStr, CString};
use std::slice;

// FFI declarations for the Go shared library
extern "C" {
    /// Format a shell script with config
    ///
    /// # Arguments
    /// * `source` - The shell script source code
    /// * `source_len` - Length of the source code
    /// * `indent` - Indent: 0 for tabs, >0 for number of spaces
    ///
    /// # Returns
    /// A newly allocated C string with the formatted code (must be freed with FreeString)
    fn FormatShellWithConfig(
        source: *const c_char,
        source_len: size_t,
        indent: c_uint,
    ) -> *mut c_char;

    /// Free a string allocated by FormatShell
    fn FreeString(str: *mut c_char);

    /// Format multiple shell scripts in batch with config
    fn FormatShellBatchWithConfig(
        sources: *const *const c_char,
        lengths: *const size_t,
        count: size_t,
        indent: c_uint,
    ) -> *mut *mut c_char;

    /// Free an array of strings allocated by FormatShellBatch
    fn FreeStringArray(arr: *mut *mut c_char, count: size_t);
}

/// Format Shell source code
///
/// # Arguments
/// * `source` - The Shell source code to format
/// * `file_path` - The file path (used for error reporting, currently unused)
///
/// # Returns
/// The formatted Shell source code, or an error message if formatting fails.
pub fn format_shell(source: &str, _file_path: &str) -> Result<String, String> {
    let config = FormatConfig::default();

    // shfmt uses 0 for tabs, >0 for number of spaces
    let indent: c_uint = match config.indent_style {
        IndentStyle::Tabs => 0,
        IndentStyle::Spaces => config.indent_width as c_uint,
    };

    // Convert Rust string to C string
    let c_source = match CString::new(source) {
        Ok(s) => s,
        Err(e) => return Err(format!("Failed to convert source to C string: {}", e)),
    };

    // Call the Go formatting function with config
    let c_result =
        unsafe { FormatShellWithConfig(c_source.as_ptr(), source.len() as size_t, indent) };

    if c_result.is_null() {
        return Err("Go formatter returned null".to_string());
    }

    // Convert the result back to Rust string
    let result = unsafe { CStr::from_ptr(c_result) }
        .to_str()
        .map(|s| s.to_string())
        .map_err(|e| format!("Failed to convert result from C string: {}", e));

    // Free the C string
    unsafe { FreeString(c_result) };

    result
}

/// Format multiple shell scripts in batch for efficiency
///
/// # Arguments
/// * `sources` - Slice of shell script source codes
///
/// # Returns
/// Vector of formatted source codes
pub fn format_shell_batch(sources: &[&str]) -> Vec<Result<String, String>> {
    if sources.is_empty() {
        return Vec::new();
    }

    let config = FormatConfig::default();

    // shfmt uses 0 for tabs, >0 for number of spaces
    let indent: c_uint = match config.indent_style {
        IndentStyle::Tabs => 0,
        IndentStyle::Spaces => config.indent_width as c_uint,
    };

    // Convert each source to C string
    let c_sources: Vec<CString> = sources
        .iter()
        .map(|s| CString::new(*s))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|_| vec![]);

    if c_sources.len() != sources.len() {
        // If conversion failed, return all errors
        return sources
            .iter()
            .map(|_| Err("Failed to convert source to C string".to_string()))
            .collect();
    }

    // Create pointers array
    let c_ptrs: Vec<*const c_char> = c_sources.iter().map(|s| s.as_ptr()).collect();
    let lengths: Vec<size_t> = sources.iter().map(|s| s.len() as size_t).collect();

    // Call batch formatting with config
    let c_results = unsafe {
        FormatShellBatchWithConfig(
            c_ptrs.as_ptr(),
            lengths.as_ptr(),
            sources.len() as size_t,
            indent,
        )
    };

    if c_results.is_null() {
        return sources
            .iter()
            .map(|_| Err("Go formatter returned null".to_string()))
            .collect();
    }

    // Convert results back to Rust strings
    let results_slice = unsafe { slice::from_raw_parts(c_results, sources.len()) };
    let mut results = Vec::with_capacity(sources.len());

    for &c_str in results_slice {
        if c_str.is_null() {
            results.push(Err("Go formatter returned null string".to_string()));
        } else {
            let result = unsafe { CStr::from_ptr(c_str) }
                .to_str()
                .map(|s| s.to_string())
                .map_err(|e| format!("Failed to convert result from C string: {}", e));
            results.push(result);
        }
    }

    // Free the C string array
    unsafe { FreeStringArray(c_results, sources.len() as size_t) };

    results
}

/// Format a file based on its file type
pub fn format_file(source: &str, file_path: &str, file_type: FileType) -> Result<String, String> {
    match file_type {
        FileType::Shell => format_shell(source, file_path),
        _ => Err(format!(
            "File type {:?} is not supported by sh-formatter",
            file_type
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_simple_script() {
        let source = r#"#!/bin/bash
echo "hello"  "#;

        let result = format_shell(source, "test.sh");
        assert!(result.is_ok());
        let formatted = result.unwrap();
        // Should be formatted with proper spacing
        assert!(formatted.contains("echo"));
    }

    #[test]
    fn test_format_file_with_shell() {
        let source = r#"#!/bin/bash
if true; then echo "yes";fi"#;

        let result = format_file(source, "test.sh", FileType::Shell);
        assert!(result.is_ok());
    }

    #[test]
    fn test_format_file_with_unsupported_type() {
        let source = "test";
        let result = format_file(source, "test.js", FileType::JavaScript);
        assert!(result.is_err());
    }

    #[test]
    fn test_format_batch() {
        let sources = vec![
            r#"#!/bin/bash
echo "hello""#,
            r#"if true; then echo "yes"; fi"#,
        ];

        let results = format_shell_batch(&sources);
        assert_eq!(results.len(), 2);
        assert!(results[0].is_ok());
        assert!(results[1].is_ok());
    }
}
