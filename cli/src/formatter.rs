// formatter.rs - Format routing logic

use fama_common::{detect_file_type, FileType};
use std::fs;
use std::path::PathBuf;

/// Format a single file based on its detected type
pub fn format_file(file_path: &PathBuf) -> anyhow::Result<bool> {
	let content = fs::read_to_string(file_path)?;
	let path_str = file_path.to_str().unwrap_or("");
	let file_type = detect_file_type(path_str);

	let formatted = format_content(&content, path_str, file_type)
		.map_err(|e| anyhow::anyhow!("{}: {}", file_path.display(), e))?;

	if formatted != content {
		fs::write(file_path, formatted)?;
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
		| FileType::Html
		| FileType::Vue
		| FileType::Svelte
		| FileType::Astro => biome::format_file(content, path, file_type),

		// Data + Style files -> dprint
		FileType::Yaml
		| FileType::Markdown
		| FileType::Css
		| FileType::Scss
		| FileType::Less
		| FileType::Sass => dprint::format_file(content, path, file_type),

		// Individual formatters
		FileType::Rust => rustfmt::format_rust(content, path),
		FileType::Python => ruff::format_python(content, path),
		FileType::Lua => stylua::format_lua(content, path),
		FileType::Shell => shfmt::format_shell(content, path),
		FileType::Dockerfile => dockerfile::format_dockerfile(content, path),

		FileType::Unknown => Err("Unknown file type".to_string()),
	}
}
