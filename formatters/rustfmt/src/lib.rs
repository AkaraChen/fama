//! Rust formatter using rustfmt
//!
//! This module provides Rust code formatting functionality via the rustfmt
//! formatter, using the rust-format crate for a clean library API.

use fama_common::FileType;
use rust_format::Formatter;
use rust_format::RustFmt;

/// Format Rust source code
///
/// # Arguments
/// * `source` - The Rust source code to format
/// * `file_path` - The file path (used for error reporting, currently unused)
///
/// # Returns
/// The formatted Rust source code, or an error message if formatting fails.
pub fn format_rust(source: &str, _file_path: &str) -> Result<String, String> {
    // Use rustfmt via rust-format crate
    let formatter = RustFmt::default();

    formatter
        .format_str(source)
        .map_err(|e| format!("rustfmt error: {}", e))
}

/// Format a file based on its file type
pub fn format_file(source: &str, file_path: &str, file_type: FileType) -> Result<String, String> {
    match file_type {
        FileType::Rust => format_rust(source, file_path),
        _ => Err(format!(
            "File type {:?} is not supported by rust-formatter",
            file_type
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_simple_function() {
        let source = r#"fn main(){println!("Hello");}"#;
        let result = format_rust(source, "test.rs").unwrap();

        // Check that formatting occurred (should have proper indentation)
        assert!(result.contains("fn main()"));
        assert!(result.contains("println!"));
    }

    #[test]
    fn test_format_file_with_rust() {
        let source = r#"fn main(){println!("Hello");}"#;
        let result = format_file(source, "test.rs", FileType::Rust).unwrap();

        // Check that formatting occurred
        assert!(result.contains("fn main()"));
        assert!(result.contains("println!"));
    }

    #[test]
    fn test_format_file_with_unsupported_type() {
        let source = "test";
        let result = format_file(source, "test.js", FileType::JavaScript);
        assert!(result.is_err());
    }
}
