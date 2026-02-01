// dprint-formatter - Dprint plugin formatting library
//
// Provides formatting API for Markdown, YAML, CSS, SCSS, LESS, SASS,
// and Dockerfile using dprint plugins and Malva.

#![allow(clippy::all)]

use fama_common::FileType;

/// Format Markdown source code with specified options
pub fn format_markdown(source: &str, _file_path: &str) -> Result<String, String> {
    use dprint_core::configuration::NewLineKind;
    use dprint_plugin_markdown::configuration::*;

    let config = Configuration {
        line_width: 80,
        new_line_kind: NewLineKind::LineFeed,
        text_wrap: TextWrap::Maintain,
        emphasis_kind: EmphasisKind::Underscores,
        strong_kind: StrongKind::Asterisks,
        unordered_list_kind: UnorderedListKind::Dashes,
        ignore_directive: "dprint-ignore".to_string(),
        ignore_file_directive: "dprint-ignore-file".to_string(),
        ignore_start_directive: "dprint-ignore-start".to_string(),
        ignore_end_directive: "dprint-ignore-end".to_string(),
    };

    // Create a closure that returns Ok(None) to not format code blocks
    let format_code_block = |_file_path: &str,
                             _code: &str,
                             _line_width: u32|
     -> Result<Option<String>, anyhow::Error> { Ok(None) };

    match dprint_plugin_markdown::format_text(source, &config, format_code_block) {
        Ok(Some(result)) => Ok(result),
        Ok(None) => {
            // No changes needed, return original content
            Ok(source.to_string())
        }
        Err(e) => Err(format!("Markdown formatting error: {}", e)),
    }
}

/// Format YAML source code with specified options
pub fn format_yaml(source: &str, _file_path: &str) -> Result<String, String> {
    use pretty_yaml::config::{FormatOptions, LanguageOptions, LayoutOptions, LineBreak};

    let config = FormatOptions {
        layout: LayoutOptions {
            print_width: 80,
            indent_width: 2,
            line_break: LineBreak::Lf,
        },
        language: LanguageOptions::default(),
    };

    pretty_yaml::format_text(source, &config).map_err(|e| format!("YAML formatting error: {}", e))
}

/// Format CSS source code using Malva formatter
pub fn format_css(source: &str, _file_path: &str) -> Result<String, String> {
    use malva::{config::FormatOptions, format_text, Syntax};

    let options = FormatOptions::default();
    format_text(source, Syntax::Css, &options).map_err(|e| format!("CSS formatting error: {}", e))
}

/// Format SCSS source code using Malva formatter
pub fn format_scss(source: &str, _file_path: &str) -> Result<String, String> {
    use malva::{config::FormatOptions, format_text, Syntax};

    let options = FormatOptions::default();
    format_text(source, Syntax::Scss, &options).map_err(|e| format!("SCSS formatting error: {}", e))
}

/// Format LESS source code using Malva formatter
pub fn format_less(source: &str, _file_path: &str) -> Result<String, String> {
    use malva::{config::FormatOptions, format_text, Syntax};

    let options = FormatOptions::default();
    format_text(source, Syntax::Less, &options).map_err(|e| format!("LESS formatting error: {}", e))
}

/// Format SASS source code using Malva formatter
pub fn format_sass(source: &str, _file_path: &str) -> Result<String, String> {
    use malva::{config::FormatOptions, format_text, Syntax};

    let options = FormatOptions::default();
    format_text(source, Syntax::Sass, &options).map_err(|e| format!("SASS formatting error: {}", e))
}

/// Format Dockerfile source code
/// Note: Currently returns source unchanged due to dprint-core version conflicts
/// Dockerfile plugin uses dprint-core 0.65 which is incompatible with dprint-core 0.67 used by Markdown
pub fn format_dockerfile(source: &str, _file_path: &str) -> Result<String, String> {
    // For now, return the source unchanged
    // TODO: Resolve dprint-core version conflict or use alternative Dockerfile formatter
    Ok(source.to_string())
}

/// Format a file based on its file type
pub fn format_file(source: &str, file_path: &str, file_type: FileType) -> Result<String, String> {
    match file_type {
        FileType::Markdown => format_markdown(source, file_path),
        FileType::Yaml => format_yaml(source, file_path),
        FileType::Css => format_css(source, file_path),
        FileType::Scss => format_scss(source, file_path),
        FileType::Less => format_less(source, file_path),
        FileType::Sass => format_sass(source, file_path),
        FileType::Dockerfile => format_dockerfile(source, file_path),
        _ => Err(format!(
            "File type {:?} is not supported by dprint-formatter",
            file_type
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_markdown() {
        let source = "# Hello World";
        let result = format_markdown(source, "test.md").unwrap();
        assert!(result.contains("Hello") || result.contains("#"));
    }

    #[test]
    fn test_format_yaml() {
        let source = "name: test\nage: 30";
        let result = format_yaml(source, "test.yaml").unwrap();
        assert!(result.contains("name") || result.contains("age"));
    }

    #[test]
    fn test_format_css() {
        let source = "body{margin:0;padding:0;}";
        let result = format_css(source, "test.css").unwrap();
        // CSS formatting with Malva should format the code
        assert!(result.contains("margin") && result.contains("padding"));
    }

    #[test]
    fn test_format_scss() {
        let source = ".foo{margin:0;}";
        let result = format_scss(source, "test.scss").unwrap();
        assert!(result.contains("margin"));
    }

    #[test]
    fn test_format_less() {
        let source = ".foo{margin:0;}";
        let result = format_less(source, "test.less").unwrap();
        assert!(result.contains("margin"));
    }

    #[test]
    fn test_format_sass() {
        let source = ".foo\n  margin: 0";
        let result = format_sass(source, "test.sass").unwrap();
        assert!(result.contains("margin"));
    }

    #[test]
    fn test_format_dockerfile() {
        let source = "FROM ubuntu";
        let result = format_dockerfile(source, "Dockerfile").unwrap();
        // Dockerfile formatting returns source unchanged (placeholder)
        assert_eq!(result, source);
    }

    #[test]
    fn test_format_file_with_markdown() {
        let source = "# Hello World";
        let result = format_file(source, "test.md", FileType::Markdown).unwrap();
        assert!(result.contains("Hello") || result.contains("#"));
    }

    #[test]
    fn test_format_file_with_unsupported_type() {
        let source = "test";
        let result = format_file(source, "test.js", FileType::JavaScript);
        assert!(result.is_err());
    }
}
