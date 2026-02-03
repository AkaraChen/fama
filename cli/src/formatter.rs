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

		// Individual formatters
		FileType::Toml => toml_fmt::format_toml(content, path),
		FileType::Rust => rustfmt::format_rust(content, path),
		FileType::Python => ruff::format_python(content, path),
		FileType::Lua => stylua::format_lua(content, path),
		FileType::Shell => goffi::format_shell(content, path),
		FileType::Go => goffi::format_go(content, path),
		FileType::Dockerfile => dockerfile::format_dockerfile(content, path),
		FileType::Xml => xml_fmt::format_xml(content, path),
		FileType::Sql => fama_sqruff::format_sql(content, path),
		FileType::Php => php_fmt::format_php(content, path),

		FileType::Unknown => Err("Unknown file type".to_string()),
	}
}
