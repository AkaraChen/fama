// fama-common - Shared types and utilities for fama formatters
//
// Provides common file type detection and shared enums used across
// all formatter crates.

use std::path::Path;

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
    Sass,
    Html,
    Vue,
    Svelte,
    Astro,
    Yaml,
    Markdown,
    Dockerfile,
    Rust,
    Python,
    Kotlin,
    Lua,
    Shell,
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
        Some("sass") => FileType::Sass,
        Some("html") | Some("htm") => FileType::Html,
        Some("vue") => FileType::Vue,
        Some("svelte") => FileType::Svelte,
        Some("astro") => FileType::Astro,
        Some("yaml") | Some("yml") => FileType::Yaml,
        Some("md") | Some("markdown") => FileType::Markdown,
        Some("rs") => FileType::Rust,
        Some("py") => FileType::Python,
        Some("kt") | Some("kts") => FileType::Kotlin,
        Some("lua") => FileType::Lua,
        Some("sh") | Some("bash") | Some("zsh") => FileType::Shell,
        _ => {
            // Check for Dockerfile by name
            if path.file_name().and_then(|n| n.to_str()) == Some("Dockerfile") {
                FileType::Dockerfile
            } else {
                FileType::Unknown
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_javascript() {
        assert_eq!(detect_file_type("test.js"), FileType::JavaScript);
        assert_eq!(detect_file_type("test.cjs"), FileType::JavaScript);
        assert_eq!(detect_file_type("test.mjs"), FileType::JavaScript);
    }

    #[test]
    fn test_detect_typescript() {
        assert_eq!(detect_file_type("test.ts"), FileType::TypeScript);
        assert_eq!(detect_file_type("test.mts"), FileType::TypeScript);
    }

    #[test]
    fn test_detect_jsx() {
        assert_eq!(detect_file_type("test.jsx"), FileType::Jsx);
        assert_eq!(detect_file_type("test.mjsx"), FileType::Jsx);
    }

    #[test]
    fn test_detect_tsx() {
        assert_eq!(detect_file_type("test.tsx"), FileType::Tsx);
    }

    #[test]
    fn test_detect_css_variants() {
        assert_eq!(detect_file_type("test.css"), FileType::Css);
        assert_eq!(detect_file_type("test.scss"), FileType::Scss);
        assert_eq!(detect_file_type("test.less"), FileType::Less);
        assert_eq!(detect_file_type("test.sass"), FileType::Sass);
    }

    #[test]
    fn test_detect_html_variants() {
        assert_eq!(detect_file_type("test.html"), FileType::Html);
        assert_eq!(detect_file_type("test.htm"), FileType::Html);
        assert_eq!(detect_file_type("test.vue"), FileType::Vue);
        assert_eq!(detect_file_type("test.svelte"), FileType::Svelte);
        assert_eq!(detect_file_type("test.astro"), FileType::Astro);
    }

    #[test]
    fn test_detect_yaml() {
        assert_eq!(detect_file_type("test.yaml"), FileType::Yaml);
        assert_eq!(detect_file_type("test.yml"), FileType::Yaml);
    }

    #[test]
    fn test_detect_markdown() {
        assert_eq!(detect_file_type("test.md"), FileType::Markdown);
        assert_eq!(detect_file_type("test.markdown"), FileType::Markdown);
    }

    #[test]
    fn test_detect_rust() {
        assert_eq!(detect_file_type("test.rs"), FileType::Rust);
    }

    #[test]
    fn test_detect_python() {
        assert_eq!(detect_file_type("test.py"), FileType::Python);
    }

    #[test]
    fn test_detect_kotlin() {
        assert_eq!(detect_file_type("test.kt"), FileType::Kotlin);
        assert_eq!(detect_file_type("test.kts"), FileType::Kotlin);
    }

    #[test]
    fn test_detect_dockerfile() {
        assert_eq!(detect_file_type("Dockerfile"), FileType::Dockerfile);
        assert_eq!(detect_file_type("path/to/Dockerfile"), FileType::Dockerfile);
    }

    #[test]
    fn test_detect_lua() {
        assert_eq!(detect_file_type("test.lua"), FileType::Lua);
    }

    #[test]
    fn test_detect_shell() {
        assert_eq!(detect_file_type("test.sh"), FileType::Shell);
        assert_eq!(detect_file_type("test.bash"), FileType::Shell);
        assert_eq!(detect_file_type("test.zsh"), FileType::Shell);
    }

    #[test]
    fn test_detect_unknown() {
        assert_eq!(detect_file_type("unknown.xyz"), FileType::Unknown);
        assert_eq!(detect_file_type("test.unknown"), FileType::Unknown);
    }
}
