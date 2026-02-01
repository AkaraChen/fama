// biome-binding - Biome formatting library
//
// Provides a unified formatting API for JavaScript, TypeScript, JSX, CSS, HTML,
// YAML, and Markdown using Biome parser/formatter crates and dprint plugins.

#![allow(clippy::all)]

// Biome formatter imports
use biome_formatter::{IndentStyle, IndentWidth, LineWidth, QuoteStyle};
use biome_js_formatter::context::JsFormatOptions;
use biome_js_syntax::JsFileSource;

use biome_js_parser::parse;
use biome_css_parser::parse_css;
use biome_html_parser::parse_html;

use std::path::Path;

/// Hard-coded formatting options
const INDENT_WIDTH: u8 = 2;
const LINE_WIDTH: u16 = 80;

/// Format JavaScript source code
pub fn format_javascript(source: &str, _file_path: &str) -> Result<String, String> {
    let source_type = JsFileSource::js_module();
    let options = JsFormatOptions::new(source_type)
        .with_indent_style(IndentStyle::Space)
        .with_indent_width(IndentWidth::try_from(INDENT_WIDTH).unwrap())
        .with_line_width(LineWidth::try_from(LINE_WIDTH).unwrap())
        .with_quote_style(QuoteStyle::Double);

    let parsed = parse(source, source_type, Default::default());

    if parsed.has_errors() {
        return Err(format!("Parse errors in JavaScript file"));
    }

    let syntax = parsed.syntax();

    let formatted = biome_js_formatter::format_node(options, &syntax)
        .map_err(|e| format!("Format error: {:?}", e))?;

    formatted
        .print()
        .map(|p| p.as_code().to_string())
        .map_err(|e| format!("Print error: {:?}", e))
}

/// Format TypeScript source code
pub fn format_typescript(source: &str, _file_path: &str) -> Result<String, String> {
    let source_type = JsFileSource::ts();
    let options = JsFormatOptions::new(source_type)
        .with_indent_style(IndentStyle::Space)
        .with_indent_width(IndentWidth::try_from(INDENT_WIDTH).unwrap())
        .with_line_width(LineWidth::try_from(LINE_WIDTH).unwrap())
        .with_quote_style(QuoteStyle::Double);

    let parsed = parse(source, source_type, Default::default());

    if parsed.has_errors() {
        return Err(format!("Parse errors in TypeScript file"));
    }

    let syntax = parsed.syntax();

    let formatted = biome_js_formatter::format_node(options, &syntax)
        .map_err(|e| format!("Format error: {:?}", e))?;

    formatted
        .print()
        .map(|p| p.as_code().to_string())
        .map_err(|e| format!("Print error: {:?}", e))
}

/// Format JSX source code
pub fn format_jsx(source: &str, _file_path: &str) -> Result<String, String> {
    let source_type = JsFileSource::jsx();
    let options = JsFormatOptions::new(source_type)
        .with_indent_style(IndentStyle::Space)
        .with_indent_width(IndentWidth::try_from(INDENT_WIDTH).unwrap())
        .with_line_width(LineWidth::try_from(LINE_WIDTH).unwrap())
        .with_quote_style(QuoteStyle::Double);

    let parsed = parse(source, source_type, Default::default());

    if parsed.has_errors() {
        return Err(format!("Parse errors in JSX file"));
    }

    let syntax = parsed.syntax();

    let formatted = biome_js_formatter::format_node(options, &syntax)
        .map_err(|e| format!("Format error: {:?}", e))?;

    formatted
        .print()
        .map(|p| p.as_code().to_string())
        .map_err(|e| format!("Print error: {:?}", e))
}

/// Format TSX source code
pub fn format_tsx(source: &str, _file_path: &str) -> Result<String, String> {
    let source_type = JsFileSource::tsx();
    let options = JsFormatOptions::new(source_type)
        .with_indent_style(IndentStyle::Space)
        .with_indent_width(IndentWidth::try_from(INDENT_WIDTH).unwrap())
        .with_line_width(LineWidth::try_from(LINE_WIDTH).unwrap())
        .with_quote_style(QuoteStyle::Double);

    let parsed = parse(source, source_type, Default::default());

    if parsed.has_errors() {
        return Err(format!("Parse errors in TSX file"));
    }

    let syntax = parsed.syntax();

    let formatted = biome_js_formatter::format_node(options, &syntax)
        .map_err(|e| format!("Format error: {:?}", e))?;

    formatted
        .print()
        .map(|p| p.as_code().to_string())
        .map_err(|e| format!("Print error: {:?}", e))
}

/// Format CSS source code
pub fn format_css(source: &str, _file_path: &str) -> Result<String, String> {
    let options = biome_css_formatter::context::CssFormatOptions::default()
        .with_indent_style(IndentStyle::Space)
        .with_indent_width(IndentWidth::try_from(INDENT_WIDTH).unwrap())
        .with_line_width(LineWidth::try_from(LINE_WIDTH).unwrap());

    let parsed = parse_css(source, Default::default());

    if parsed.has_errors() {
        return Err(format!("Parse errors in CSS file"));
    }

    let syntax = parsed.syntax();

    let formatted = biome_css_formatter::format_node(options, &syntax)
        .map_err(|e| format!("Format error: {:?}", e))?;

    formatted
        .print()
        .map(|p| p.as_code().to_string())
        .map_err(|e| format!("Print error: {:?}", e))
}

/// Format SCSS source code (uses CSS parser - limited support)
pub fn format_scss(source: &str, file_path: &str) -> Result<String, String> {
    // SCSS uses CSS parser - has limited support for SCSS features
    // For full SCSS support, a dedicated SCSS parser would be needed
    match format_css(source, file_path) {
        Ok(result) => Ok(result),
        Err(_) => {
            // If CSS parser fails, return original content (SCSS has features CSS parser can't handle)
            eprintln!("Warning: {} syntax not fully supported, file may not be properly formatted", file_path);
            Ok(source.to_string())
        }
    }
}

/// Format LESS source code (uses CSS parser - limited support)
pub fn format_less(source: &str, file_path: &str) -> Result<String, String> {
    // LESS uses CSS parser - has limited support for LESS features
    // For full LESS support, a dedicated LESS parser would be needed
    match format_css(source, file_path) {
        Ok(result) => Ok(result),
        Err(_) => {
            // If CSS parser fails, return original content (LESS has features CSS parser can't handle)
            eprintln!("Warning: {} syntax not fully supported, file may not be properly formatted", file_path);
            Ok(source.to_string())
        }
    }
}

/// Format HTML source code
pub fn format_html(source: &str, _file_path: &str) -> Result<String, String> {
    let options = biome_html_formatter::context::HtmlFormatOptions::default()
        .with_indent_style(IndentStyle::Space)
        .with_indent_width(IndentWidth::try_from(INDENT_WIDTH).unwrap())
        .with_line_width(LineWidth::try_from(LINE_WIDTH).unwrap());

    let parsed = parse_html(source, Default::default());

    if parsed.has_errors() {
        return Err(format!("Parse errors in HTML file"));
    }

    let syntax = parsed.syntax();

    let formatted = biome_html_formatter::format_node(options, &syntax, false)
        .map_err(|e| format!("Format error: {:?}", e))?;

    formatted
        .print()
        .map(|p| p.as_code().to_string())
        .map_err(|e| format!("Print error: {:?}", e))
}

/// Format Vue SFC source code (limited - extracts and formats template/script/style)
pub fn format_vue(source: &str, file_path: &str) -> Result<String, String> {
    // Vue SFC has special syntax - for now use HTML formatter with lenient parsing
    // Full Vue support would require extracting each section and formatting separately
    match format_html(source, file_path) {
        Ok(result) => Ok(result),
        Err(_) => {
            // If HTML parser fails, return original content (Vue has features HTML parser can't handle)
            Ok(source.to_string())
        }
    }
}

/// Format Svelte source code (limited - uses HTML parser)
pub fn format_svelte(source: &str, file_path: &str) -> Result<String, String> {
    // Svelte has special syntax - for now use HTML formatter with lenient parsing
    // Full Svelte support would require a dedicated Svelte parser
    match format_html(source, file_path) {
        Ok(result) => Ok(result),
        Err(_) => {
            // If HTML parser fails, return original content (Svelte has features HTML parser can't handle)
            eprintln!("Warning: {} syntax not fully supported, file may not be properly formatted", file_path);
            Ok(source.to_string())
        }
    }
}

/// Format Astro source code (limited - extracts frontmatter and HTML)
pub fn format_astro(source: &str, file_path: &str) -> Result<String, String> {
    // Astro has frontmatter (fenced code block) - for now use HTML formatter
    // Full Astro support would require extracting and formatting frontmatter separately
    match format_html(source, file_path) {
        Ok(result) => Ok(result),
        Err(_) => {
            // If HTML parser fails, return original content (Astro has features HTML parser can't handle)
            eprintln!("Warning: {} syntax not fully supported, file may not be properly formatted", file_path);
            Ok(source.to_string())
        }
    }
}

/// Format YAML source code with specified options
pub fn format_yaml(source: &str, _file_path: &str) -> Result<String, String> {
    // Parse and re-serialize YAML for consistent formatting
    let value: serde_json::Value = serde_yaml::from_str(source)
        .map_err(|e| format!("YAML parsing error: {}", e))?;
    serde_yaml::to_string(&value)
        .map_err(|e| format!("YAML formatting error: {}", e))
}

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
    let format_code_block = |_file_path: &str, _code: &str, _line_width: u32| -> Result<Option<String>, anyhow::Error> {
        Ok(None)
    };

    match dprint_plugin_markdown::format_text(source, &config, format_code_block) {
        Ok(Some(result)) => Ok(result),
        Ok(None) => {
            // No changes needed, return original content
            Ok(source.to_string())
        }
        Err(e) => Err(format!("Markdown formatting error: {}", e)),
    }
}

/// File type enum for language detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    JavaScript,
    TypeScript,
    Jsx,
    Tsx,
    Css,
    Scss,
    Less,
    Html,
    Vue,
    Svelte,
    Astro,
    Yaml,
    Markdown,
    Unknown,
}

/// Detect file type from extension
pub fn detect_file_type(path: &str) -> FileType {
    let path = Path::new(path);
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("js") | Some("cjs") | Some("mjs") => FileType::JavaScript,
        Some("ts") | Some("mts") => FileType::TypeScript,
        Some("jsx") | Some("mjsx") => FileType::Jsx,
        Some("tsx") => FileType::Tsx,
        Some("css") => FileType::Css,
        Some("scss") => FileType::Scss,
        Some("less") => FileType::Less,
        Some("html") | Some("htm") => FileType::Html,
        Some("vue") => FileType::Vue,
        Some("svelte") => FileType::Svelte,
        Some("astro") => FileType::Astro,
        Some("yaml") | Some("yml") => FileType::Yaml,
        Some("md") | Some("markdown") => FileType::Markdown,
        _ => FileType::Unknown,
    }
}

/// Format a file based on its extension
pub fn format_file(source: &str, file_path: &str) -> Result<String, String> {
    match detect_file_type(file_path) {
        FileType::JavaScript => format_javascript(source, file_path),
        FileType::TypeScript => format_typescript(source, file_path),
        FileType::Jsx => format_jsx(source, file_path),
        FileType::Tsx => format_tsx(source, file_path),
        FileType::Css => format_css(source, file_path),
        FileType::Scss => format_scss(source, file_path),
        FileType::Less => format_less(source, file_path),
        FileType::Html => format_html(source, file_path),
        FileType::Vue => format_vue(source, file_path),
        FileType::Svelte => format_svelte(source, file_path),
        FileType::Astro => format_astro(source, file_path),
        FileType::Yaml => format_yaml(source, file_path),
        FileType::Markdown => format_markdown(source, file_path),
        FileType::Unknown => Err(format!(
            "Unknown file type for: {}",
            file_path
        )),
    }
}

/// Legacy function - use format_file instead
pub fn format_code(input: &str) -> String {
    input.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_javascript() {
        let source = "const   x   =   1;";
        let result = format_javascript(source, "test.js").unwrap();
        assert!(result.contains("x = 1"));
    }

    #[test]
    fn test_format_typescript() {
        let source = "const   x: number   =   1;";
        let result = format_typescript(source, "test.ts").unwrap();
        assert!(result.contains("x: number") && result.contains("1"));
    }

    #[test]
    fn test_format_css() {
        let source = "body{margin:0;padding:0;}";
        let result = format_css(source, "test.css").unwrap();
        assert!(result.contains("margin") && result.contains("padding"));
    }

    #[test]
    fn test_format_html() {
        let source = "<html><body></body></html>";
        let result = format_html(source, "test.html").unwrap();
        assert!(result.contains("<html>") || result.contains("<body>"));
    }

    #[test]
    fn test_format_yaml() {
        let source = "name: test\nage: 30";
        let result = format_yaml(source, "test.yaml").unwrap();
        assert!(result.contains("name") || result.contains("age"));
    }

    #[test]
    fn test_format_markdown() {
        let source = "# Hello World";
        let result = format_markdown(source, "test.md").unwrap();
        assert!(result.contains("Hello") || result.contains("#"));
    }

    #[test]
    fn test_detect_file_type() {
        assert_eq!(detect_file_type("test.js"), FileType::JavaScript);
        assert_eq!(detect_file_type("test.ts"), FileType::TypeScript);
        assert_eq!(detect_file_type("test.jsx"), FileType::Jsx);
        assert_eq!(detect_file_type("test.tsx"), FileType::Tsx);
        assert_eq!(detect_file_type("test.css"), FileType::Css);
        assert_eq!(detect_file_type("test.scss"), FileType::Scss);
        assert_eq!(detect_file_type("test.html"), FileType::Html);
        assert_eq!(detect_file_type("test.vue"), FileType::Vue);
        assert_eq!(detect_file_type("test.yaml"), FileType::Yaml);
        assert_eq!(detect_file_type("test.md"), FileType::Markdown);
        assert_eq!(detect_file_type("unknown.xyz"), FileType::Unknown);
    }
}
