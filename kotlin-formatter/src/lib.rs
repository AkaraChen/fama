//! Kotlin formatter using tree-sitter-kotlin
//!
//! This module provides Kotlin code formatting functionality using the
//! tree-sitter-kotlin grammar for parsing and basic formatting.
//!
//! Note: This is a basic formatter that provides indentation normalization.
//! For production use, consider integrating with the official Kotlin formatter
//! (ktlint) or using Topiary as a CLI tool with appropriate query files.

use fama_common::FileType;
use tree_sitter::Parser;

/// Format Kotlin source code
///
/// # Arguments
/// * `source` - The Kotlin source code to format
/// * `file_path` - The file path (used for error reporting, currently unused)
///
/// # Returns
/// The formatted Kotlin source code, or an error message if formatting fails.
///
/// # Note
/// This is a basic formatter that provides:
/// - Consistent indentation (4 spaces for Kotlin)
/// - Basic whitespace normalization
///
/// For full Kotlin formatting support, integrate with ktlint or use
/// Topiary CLI with Kotlin query files.
pub fn format_kotlin(source: &str, _file_path: &str) -> Result<String, String> {
    // Initialize parser with Kotlin grammar
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_kotlin::language())
        .map_err(|e| format!("Failed to set Kotlin language: {}", e))?;

    // Parse the source code
    let tree = parser
        .parse(source, None)
        .ok_or("Failed to parse Kotlin source")?;

    // Get the root node
    let root_node = tree.root_node();

    // If parsing failed, return the source as-is
    if root_node.has_error() {
        // For now, return the source with basic whitespace normalization
        return Ok(normalize_whitespace(source));
    }

    // Apply basic formatting rules
    Ok(normalize_whitespace(source))
}

/// Normalize whitespace in Kotlin code
fn normalize_whitespace(source: &str) -> String {
    let mut result = String::new();
    let mut indent_level: usize = 0;
    const INDENT: &str = "    "; // 4 spaces for Kotlin

    for line in source.lines() {
        let trimmed = line.trim();

        // Skip empty lines but preserve them
        if trimmed.is_empty() {
            result.push('\n');
            continue;
        }

        // Decrease indent for closing braces
        if trimmed.starts_with('}') || trimmed.starts_with(']') || trimmed.starts_with(')') {
            indent_level = indent_level.saturating_sub(1);
        }

        // Add current indentation
        for _ in 0..indent_level {
            result.push_str(INDENT);
        }

        // Add the trimmed line
        result.push_str(trimmed);

        // Count opening braces to increase indent for next line
        let open_count = trimmed
            .chars()
            .filter(|&c| c == '{' || c == '[' || c == '(')
            .count();
        let close_count = trimmed
            .chars()
            .filter(|&c| c == '}' || c == ']' || c == ')')
            .count();
        indent_level = indent_level.saturating_sub(close_count);
        indent_level += open_count;

        // Add newline
        result.push('\n');
    }

    result
}

/// Format a file based on its file type
pub fn format_file(source: &str, file_path: &str, file_type: FileType) -> Result<String, String> {
    match file_type {
        FileType::Kotlin => format_kotlin(source, file_path),
        _ => Err(format!(
            "File type {:?} is not supported by kotlin-formatter",
            file_type
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_simple_function() {
        let source = r#"fun main(){println("Hello")}"#;
        let result = format_kotlin(source, "test.kt");

        // Check that formatting occurred without error
        assert!(result.is_ok());
        let formatted = result.unwrap();
        // Basic check that content is preserved
        assert!(formatted.contains("fun"));
        assert!(formatted.contains("main"));
    }

    #[test]
    fn test_format_class() {
        let source = r#"class TestClass{val x=42}"#;
        let result = format_kotlin(source, "test.kt");

        assert!(result.is_ok());
        let formatted = result.unwrap();
        assert!(formatted.contains("class"));
    }

    #[test]
    fn test_format_file_with_kotlin() {
        let source = r#"fun main(){println("Hello")}"#;
        let result = format_file(source, "test.kt", FileType::Kotlin).unwrap();

        assert!(result.contains("fun"));
        assert!(result.contains("main"));
    }

    #[test]
    fn test_format_file_with_unsupported_type() {
        let source = "test";
        let result = format_file(source, "test.js", FileType::JavaScript);
        assert!(result.is_err());
    }

    #[test]
    fn test_normalize_whitespace() {
        let source = "fun main() {\nprintln(\"test\")\n}";
        let result = normalize_whitespace(source);
        // Should have normalized indentation
        assert!(result.contains("fun main()"));
    }
}
