// formatter.rs - Format routing logic

use fama_common::{detect_file_type, FileType};
use std::fs;
use std::path::{Path, PathBuf};

/// Format a single file based on its detected type
/// Returns true if the file was changed (or would be changed in check mode)
pub fn format_file(file_path: &PathBuf, check: bool) -> anyhow::Result<bool> {
	let content = fs::read_to_string(file_path)?;
	let path_str = file_path.to_str().unwrap_or("");
	let file_type = detect_file_type(path_str);

	// C-family: use status-2 aware path to skip realloc when already formatted.
	if matches!(
		file_type,
		FileType::C
			| FileType::Cpp
			| FileType::CSharp
			| FileType::ObjectiveC
			| FileType::Java
			| FileType::Protobuf
	) {
		return match fama_clang::format_file_changed(
			&content, path_str, file_type,
		) {
			Ok(None) => Ok(false),
			Ok(Some(formatted)) => {
				if !check {
					fs::write(file_path, formatted)?;
				}
				Ok(true)
			}
			Err(e) => Err(anyhow::anyhow!("{}: {}", file_path.display(), e)),
		};
	}

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

/// Per-file outcome used by batch helpers (stats aggregation in main).
#[derive(Debug)]
pub enum FileOutcome {
	Formatted,
	Unchanged,
	Error(String),
}

/// Batch-format Rust files via multi-path rustfmt CLI (one spawn per chunk).
pub fn format_rust_batch(files: &[PathBuf], check: bool) -> Vec<(PathBuf, FileOutcome)> {
	if files.is_empty() {
		return Vec::new();
	}

	let batch = rustfmt::format_rust_paths(files, check);
	let mut out = Vec::with_capacity(files.len());

	for p in batch.changed {
		out.push((p, FileOutcome::Formatted));
	}
	for p in batch.unchanged {
		out.push((p, FileOutcome::Unchanged));
	}
	// Spawn/chunk errors are not tied 1:1 to a single path string from -l;
	// attach them as synthetic error outcomes with the message only.
	for err in batch.errors {
		// Prefer path-prefixed messages from the rustfmt crate.
		if let Some((path_part, msg)) = err.split_once(": ") {
			let pb = PathBuf::from(path_part);
			if files.iter().any(|f| f == &pb) {
				out.push((pb, FileOutcome::Error(msg.to_string())));
				continue;
			}
		}
		out.push((
			PathBuf::from("<rustfmt>"),
			FileOutcome::Error(err),
		));
	}

	out
}

/// Detect file type from a path (shared by main partitioning).
#[inline]
pub fn file_type_of(path: &Path) -> FileType {
	detect_file_type(path.to_str().unwrap_or(""))
}
