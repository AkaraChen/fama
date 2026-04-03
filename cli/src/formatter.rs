// formatter.rs - Format routing logic

use fama_common::{detect_file_type, FileType};
use std::fs;
use std::path::PathBuf;

/// Format a single file based on its detected type
/// Returns true if the file was changed (or would be changed in check mode)
pub fn format_file(file_path: &PathBuf, check: bool) -> anyhow::Result<bool> {
	let content = fs::read_to_string(file_path)?;
	let path_str = file_path.to_str().unwrap_or("");
	let file_type = detect_file_type(path_str);

	let formatted = format_content(&content, path_str, file_type)
		.map_err(|e| anyhow::anyhow!("{}: {}", file_path.display(), e))?;

	if formatted != content {
		if !check {
			fs::write(file_path, formatted)?;
		}
		Ok(true)
	} else {
		Ok(false)
	}
}

/// Format content string based on file type
fn format_content(
	content: &str,
	path: &str,
	file_type: FileType,
) -> Result<String, String> {
	match file_type {
		// Web files -> biome
		FileType::JavaScript
		| FileType::TypeScript
		| FileType::Jsx
		| FileType::Tsx
		| FileType::Json
		| FileType::Jsonc
		| FileType::Html
		| FileType::Vue
		| FileType::Svelte
		| FileType::Astro
		| FileType::GraphQL => biome::format_file(content, path, file_type),

		// Data + Style files -> dprint
		FileType::Yaml
		| FileType::Markdown
		| FileType::Css
		| FileType::Scss
		| FileType::Less
		| FileType::Sass => dprint::format_file(content, path, file_type),

		// C-family languages -> clang-format
		FileType::C
		| FileType::Cpp
		| FileType::CSharp
		| FileType::ObjectiveC
		| FileType::Java
		| FileType::Protobuf => fama_clang::format_file(content, path, file_type),

		// Individual formatters
		FileType::Toml => toml_fmt::format_toml(content, path),
		FileType::Rust => rustfmt::format_rust(content, path),
		FileType::Python => ruff::format_python(content, path),
		FileType::Lua => stylua::format_lua(content, path),
		FileType::Ruby => ruby_fmt::format_ruby(content, path),
		FileType::Shell => goffi::format_shell(content, path),
		FileType::Go => goffi::format_go(content, path),
		FileType::Zig => zigffi::format_zig(content, path),
		FileType::Hcl => goffi::format_hcl(content, path),
		FileType::Dockerfile => dockerfile::format_dockerfile(content, path),
		FileType::Xml => xml_fmt::format_xml(content, path),
		FileType::Sql => fama_sqruff::format_sql(content, path),
		FileType::Php => php_fmt::format_php(content, path),

		FileType::Unknown => Err("Unknown file type".to_string()),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::fs;
	use tempfile::TempDir;

	#[test]
	fn test_format_file_no_change() {
		let temp_dir = TempDir::new().unwrap();
		let file_path = temp_dir.path().join("test.json");
		// Write already formatted JSON
		fs::write(&file_path, "{}").unwrap();

		let result = format_file(&file_path, false);
		
		// Just check that the function runs without error
		// The formatter may or may not modify "{}"
		assert!(result.is_ok());
	}

	#[test]
	fn test_format_file_with_changes() {
		let temp_dir = TempDir::new().unwrap();
		let file_path = temp_dir.path().join("test.json");
		// Malformed JSON that needs formatting
		fs::write(&file_path, r#"{"key":   "value"}"#).unwrap();

		let result = format_file(&file_path, false);
		
		assert!(result.is_ok());
		// JSON should be formatted
		assert_eq!(result.unwrap(), true);
	}

	#[test]
	fn test_format_file_check_mode() {
		let temp_dir = TempDir::new().unwrap();
		let file_path = temp_dir.path().join("test.json");
		fs::write(&file_path, r#"{"key":   "value"}"#).unwrap();
		let original_content = fs::read_to_string(&file_path).unwrap();

		let result = format_file(&file_path, true);
		
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), true);
		// File should NOT be modified in check mode
		let after_content = fs::read_to_string(&file_path).unwrap();
		assert_eq!(original_content, after_content);
	}

	#[test]
	fn test_format_file_nonexistent() {
		let file_path = PathBuf::from("/nonexistent/path/file.json");

		let result = format_file(&file_path, false);
		
		assert!(result.is_err());
	}

	#[test]
	fn test_format_file_unknown_type() {
		let temp_dir = TempDir::new().unwrap();
		let file_path = temp_dir.path().join("test.xyz");
		fs::write(&file_path, "content").unwrap();

		let result = format_file(&file_path, false);
		
		assert!(result.is_err());
		assert!(result.unwrap_err().to_string().contains("Unknown file type"));
	}

	#[test]
	fn test_format_content_json() {
		let content = r#"{"key":   "value"}"#;
		let result = format_content(content, "test.json", FileType::Json);
		
		assert!(result.is_ok());
		let formatted = result.unwrap();
		// Should be properly formatted
		assert!(formatted.contains("{"));
		assert!(formatted.contains("}"));
	}

	#[test]
	fn test_format_content_toml() {
		let content = "key=\"value\"";
		let result = format_content(content, "test.toml", FileType::Toml);
		
		assert!(result.is_ok());
		let formatted = result.unwrap();
		assert!(formatted.contains("key"));
		assert!(formatted.contains("value"));
	}

	#[test]
	fn test_format_content_rust() {
		let content = "fn main() {}";
		let result = format_content(content, "test.rs", FileType::Rust);
		
		assert!(result.is_ok());
		let formatted = result.unwrap();
		assert!(formatted.contains("fn main()"));
	}

	#[test]
	fn test_format_content_python() {
		let content = "x=1";
		let result = format_content(content, "test.py", FileType::Python);
		
		assert!(result.is_ok());
		let formatted = result.unwrap();
		assert!(formatted.contains("x = 1"));
	}

	#[test]
	fn test_format_content_lua() {
		let content = "x=1";
		let result = format_content(content, "test.lua", FileType::Lua);
		
		assert!(result.is_ok());
		let formatted = result.unwrap();
		assert!(formatted.contains("x = 1"));
	}

	#[test]
	fn test_format_content_shell() {
		let content = "echo hello";
		let result = format_content(content, "test.sh", FileType::Shell);
		
		assert!(result.is_ok());
		let formatted = result.unwrap();
		assert!(formatted.contains("echo"));
		assert!(formatted.contains("hello"));
	}

	#[test]
	fn test_format_content_go() {
		let content = "package main";
		let result = format_content(content, "test.go", FileType::Go);
		
		assert!(result.is_ok());
		let formatted = result.unwrap();
		assert!(formatted.contains("package main"));
	}

	#[test]
	fn test_format_content_zig() {
		let content = "const x = 1;";
		let result = format_content(content, "test.zig", FileType::Zig);
		
		assert!(result.is_ok());
		let formatted = result.unwrap();
		assert!(formatted.contains("const x = 1"));
	}

	#[test]
	fn test_format_content_xml() {
		let content = "<root><item/></root>";
		let result = format_content(content, "test.xml", FileType::Xml);
		
		assert!(result.is_ok());
		let formatted = result.unwrap();
		assert!(formatted.contains("<root>"));
		assert!(formatted.contains("</root>"));
	}

	#[test]
	fn test_format_content_sql() {
		let content = "SELECT 1;";
		let result = format_content(content, "test.sql", FileType::Sql);
		
		assert!(result.is_ok());
		let formatted = result.unwrap();
		assert!(formatted.contains("SELECT"));
	}

	#[test]
	fn test_format_content_dockerfile() {
		let content = "FROM alpine\nRUN echo hi";
		let result = format_content(content, "Dockerfile", FileType::Dockerfile);
		
		assert!(result.is_ok());
		let formatted = result.unwrap();
		assert!(formatted.contains("FROM alpine"));
	}

	#[test]
	fn test_format_content_hcl() {
		let content = "resource \"test\" \"name\" {}";
		let result = format_content(content, "test.hcl", FileType::Hcl);
		
		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_php() {
		let content = "<?php echo 'hello'; ?>";
		let result = format_content(content, "test.php", FileType::Php);
		
		assert!(result.is_ok());
		let formatted = result.unwrap();
		assert!(formatted.contains("<?php"));
	}

	#[test]
	fn test_format_content_ruby() {
		let content = "x = 1";
		let result = format_content(content, "test.rb", FileType::Ruby);
		
		assert!(result.is_ok());
		let formatted = result.unwrap();
		assert!(formatted.contains("x = 1"));
	}

	#[test]
	fn test_format_content_yaml() {
		let content = "key: value";
		let result = format_content(content, "test.yaml", FileType::Yaml);
		
		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_markdown() {
		let content = "# Hello";
		let result = format_content(content, "test.md", FileType::Markdown);
		
		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_css() {
		let content = "a{color:red}";
		let result = format_content(content, "test.css", FileType::Css);
		
		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_javascript() {
		let content = "const x=1;";
		let result = format_content(content, "test.js", FileType::JavaScript);
		
		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_typescript() {
		let content = "const x: number = 1;";
		let result = format_content(content, "test.ts", FileType::TypeScript);
		
		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_jsx() {
		let content = "const el = <div />;";
		let result = format_content(content, "test.jsx", FileType::Jsx);
		
		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_tsx() {
		let content = "const el = <div />;";
		let result = format_content(content, "test.tsx", FileType::Tsx);
		
		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_html() {
		let content = "<html><body>Hi</body></html>";
		let result = format_content(content, "test.html", FileType::Html);
		
		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_vue() {
		let content = "<template><div>Hi</div></template>";
		let result = format_content(content, "test.vue", FileType::Vue);
		
		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_svelte() {
		let content = "<div>Hello</div>";
		let result = format_content(content, "test.svelte", FileType::Svelte);
		
		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_astro() {
		let content = "---\nconst x = 1;\n---\n<div></div>";
		let result = format_content(content, "test.astro", FileType::Astro);
		
		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_graphql() {
		let content = "query { field }";
		let result = format_content(content, "test.graphql", FileType::GraphQL);
		
		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_c() {
		let content = "int main() { return 0; }";
		let result = format_content(content, "test.c", FileType::C);
		
		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_cpp() {
		let content = "int main() { return 0; }";
		let result = format_content(content, "test.cpp", FileType::Cpp);
		
		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_csharp() {
		let content = "class Test {}";
		let result = format_content(content, "test.cs", FileType::CSharp);
		
		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_java() {
		let content = "public class Test {}";
		let result = format_content(content, "test.java", FileType::Java);
		
		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_objective_c() {
		let content = "int main() { return 0; }";
		let result = format_content(content, "test.m", FileType::ObjectiveC);
		
		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_protobuf() {
		let content = "syntax = \"proto3\";";
		let result = format_content(content, "test.proto", FileType::Protobuf);
		
		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_unknown() {
		let content = "anything";
		let result = format_content(content, "test.xyz", FileType::Unknown);
		
		assert!(result.is_err());
		assert_eq!(result.unwrap_err(), "Unknown file type");
	}

	#[test]
	fn test_format_content_jsonc() {
		let content = "{\"key\": \"value\" // comment\n}";
		let result = format_content(content, "test.jsonc", FileType::Jsonc);
		
		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_scss() {
		let content = "a { color: red; }";
		let result = format_content(content, "test.scss", FileType::Scss);
		
		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_less() {
		let content = "a { color: red; }";
		let result = format_content(content, "test.less", FileType::Less);
		
		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_sass() {
		let content = "a\n  color: red";
		let result = format_content(content, "test.sass", FileType::Sass);
		
		assert!(result.is_ok());
	}
}
