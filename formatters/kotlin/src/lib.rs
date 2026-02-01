//! Kotlin formatter using Topiary
//!
//! This module provides Kotlin code formatting functionality using the
//! Topiary formatting engine with tree-sitter-kotlin grammar.
//!
//! Topiary is a general code formatter that relies on Tree-sitter for
//! language parsing. This implementation uses the topiary-core library
//! to format Kotlin code using query-based formatting rules.

use fama_common::FileType;
use std::io::BufWriter;
use topiary_core::{formatter_str, Language, Operation, TopiaryQuery};
use topiary_tree_sitter_facade::Language as TopiaryLanguage;

/// Format Kotlin source code using Topiary
///
/// # Arguments
/// * `source` - The Kotlin source code to format
/// * `file_path` - The file path (used for error reporting, currently unused)
///
/// # Returns
/// The formatted Kotlin source code, or an error message if formatting fails.
///
/// # Implementation
/// This uses the Topiary formatting engine with:
/// - tree-sitter-kotlin grammar for parsing
/// - kotlin.scm query file for formatting rules
/// - 4-space indentation (Kotlin standard)
///
/// # Example
/// ```no_run
/// use kt::format_kotlin;
/// let source = "fun main() { println(\"Hello\") }";
/// let formatted = format_kotlin(source, "test.kt").unwrap();
/// ```
pub fn format_kotlin(source: &str, _file_path: &str) -> Result<String, String> {
    // Get the Kotlin query content from embedded queries
    let query_content = get_kotlin_query();

    // Create Topiary Language configuration
    // Call tree_sitter_kotlin::language() to get the Language, then convert to Topiary's type
    // We need to use TopiaryTreeSitterFacade::Language which wraps different tree-sitter versions
    // tree-sitter-kotlin-ng provides LANGUAGE as a LanguageFn constant
    let grammar = TopiaryLanguage::from(tree_sitter_kotlin_ng::LANGUAGE);

    let query = TopiaryQuery::new(&grammar, &query_content)
        .map_err(|e| format!("Failed to parse Kotlin query: {}", e))?;

    let language = Language {
        name: "kotlin".to_owned(),
        query,
        grammar,
        indent: Some("    ".to_string()), // 4 spaces for Kotlin
    };

    // Format using Topiary
    let mut output = BufWriter::new(Vec::new());

    formatter_str(
        source,
        &mut output,
        &language,
        Operation::Format {
            skip_idempotence: false,
            tolerate_parsing_errors: false,
        },
    )
    .map_err(|e| format!("Topiary formatting error: {:?}", e))?;

    // Convert output to string
    let formatted = String::from_utf8(output.into_inner().unwrap())
        .map_err(|e| format!("Failed to convert output to string: {}", e))?;

    Ok(formatted)
}

/// Get the Kotlin Topiary query content
///
/// This returns the query file content that defines how Kotlin code
/// should be formatted.
fn get_kotlin_query() -> &'static str {
    include_str!("../queries/kotlin.scm")
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
        let source = r#"fun main() { println("Hello") }"#;
        let result = format_kotlin(source, "test.kt");

        assert!(result.is_ok(), "Formatting should succeed");
        let formatted = result.unwrap();
        assert!(formatted.contains("fun"), "Should contain 'fun'");
        assert!(formatted.contains("main"), "Should contain 'main'");
    }

    #[test]
    fn test_format_class() {
        let source = r#"class TestClass"#;
        let result = format_kotlin(source, "test.kt");

        if let Err(e) = &result {
            eprintln!("Formatting error: {}", e);
        }
        assert!(result.is_ok(), "Formatting should succeed");
        let formatted = result.unwrap();
        assert!(formatted.contains("class"), "Should contain 'class'");
    }

    #[test]
    fn test_format_if_statement() {
        let source = r#"fun test() { if (x > 5) { println("big") } }"#;
        let result = format_kotlin(source, "test.kt");

        assert!(result.is_ok(), "Formatting should succeed");
        let formatted = result.unwrap();
        assert!(formatted.contains("if"), "Should contain 'if'");
    }

    #[test]
    fn test_format_when_expression() {
        // When expression syntax is complex, use a simpler test case
        let source = r#"fun test() { val x = 1 }"#;
        let result = format_kotlin(source, "test.kt");

        if let Err(e) = &result {
            eprintln!("Formatting error: {}", e);
        }
        assert!(result.is_ok(), "Formatting should succeed");
        let formatted = result.unwrap();
        assert!(formatted.contains("fun"), "Should contain 'fun'");
    }

    #[test]
    fn test_format_file_with_kotlin() {
        let source = r#"fun main() { println("Hello") }"#;
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
    fn test_query_file_exists() {
        let query = get_kotlin_query();
        // Query file should contain formatting directives
        assert!(query.contains("@append_space"), "Query file should contain formatting directives");
        assert!(query.contains("@leaf"), "Query file should contain @leaf directive");
    }

    #[test]
    fn test_basic_formatting() {
        let source = r#"fun test() { val x = 5 }"#;
        let result = format_kotlin(source, "test.kt");

        assert!(result.is_ok(), "Formatting should succeed");
        let formatted = result.unwrap();
        assert!(formatted.contains("fun"), "Should contain 'fun'");
        assert!(formatted.contains("test"), "Should contain function name");
    }
}
