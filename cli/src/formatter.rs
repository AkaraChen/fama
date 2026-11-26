// formatter.rs - Format routing logic

use fama_common::{detect_file_type, FileType};
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Number of Rust files formatted per rustfmt process invocation. Larger
/// batches amortize the process spawn further but hold more content in memory.
const RUST_BATCH_SIZE: usize = 64;

/// Statistics collected during formatting
#[derive(Default)]
pub struct FormatStats {
	pub formatted: usize,
	pub unchanged: usize,
	pub errors: Vec<String>,
	pub formatted_files: Vec<PathBuf>,
}

impl FormatStats {
	/// Merge two FormatStats instances (used in parallel reduce)
	pub fn merge(mut self, other: FormatStats) -> FormatStats {
		self.formatted += other.formatted;
		self.unchanged += other.unchanged;
		self.errors.extend(other.errors);
		self.formatted_files.extend(other.formatted_files);
		self
	}
}

/// Record the outcome of formatting one file into the stats
fn record(
	stats: &mut FormatStats,
	file: &Path,
	result: Result<bool, String>,
	debug: bool,
) {
	match result {
		Ok(true) => {
			if debug {
				// Green for formatted files
				eprintln!(
					"{}",
					crate::color::Color::Green
						.paint(&file.display().to_string())
				);
			}
			stats.formatted += 1;
			stats.formatted_files.push(file.to_path_buf());
		}
		Ok(false) => {
			if debug {
				eprintln!("{}", file.display());
			}
			stats.unchanged += 1;
		}
		Err(e) => {
			if debug {
				eprintln!(
					"{}",
					crate::color::Color::Red.paint(&file.display().to_string())
				);
			}
			stats.errors.push(e);
		}
	}
}

/// A batch formatter: takes the chunk's sources, returns one result per input
type BatchFn = fn(&[&str]) -> Vec<Result<String, String>>;

/// Batch backends: languages whose formatter crates expose
/// `format_batch(&[&str]) -> Vec<Result<String, String>>` and are therefore
/// formatted in chunks with one formatter invocation per chunk.
fn batch_backend(file_type: FileType) -> Option<BatchFn> {
	match file_type {
		FileType::Rust => Some(rustfmt::format_batch),
		FileType::Scala => Some(scala_fmt::format_batch),
		FileType::R => Some(air_fmt::format_batch),
		FileType::Elixir => Some(elixir_fmt::format_batch),
		_ => None,
	}
}

/// Format all files, parallelizing per file and batching the batchable
/// languages (see [`batch_backend`]) concurrently, with large single files
/// scheduled first (LPT) to minimize tail latency.
pub fn format_files(
	files: &[PathBuf],
	check: bool,
	debug: bool,
) -> FormatStats {
	let mut batch_groups: HashMap<FileType, Vec<&PathBuf>> = HashMap::new();
	let mut other_files: Vec<(&PathBuf, FileType)> = Vec::new();
	for file in files {
		let file_type = detect_file_type(file.to_str().unwrap_or(""));
		if batch_backend(file_type).is_some() {
			batch_groups.entry(file_type).or_default().push(file);
		} else {
			other_files.push((file, file_type));
		}
	}

	// Longest-processing-time-first ordering for single-file formatters so large
	// files (e.g. megabyte C/TS bundles) start immediately instead of trailing.
	let mut sized_others: Vec<(&PathBuf, FileType, u64)> = if other_files.len() > 1
	{
		let mut v: Vec<(&PathBuf, FileType, u64)> = other_files
			.into_par_iter()
			.map(|(file, ft)| {
				let len = fs::metadata(file).map(|m| m.len()).unwrap_or(0);
				(file, ft, len)
			})
			.collect();
		v.sort_unstable_by(|a, b| b.2.cmp(&a.2));
		v
	} else {
		other_files
			.into_iter()
			.map(|(file, ft)| (file, ft, 0))
			.collect()
	};

	let (batched_stats, other_stats) = rayon::join(
		|| {
			let groups: Vec<(BatchFn, Vec<&PathBuf>)> = batch_groups
				.into_iter()
				.filter_map(|(ft, group)| {
					batch_backend(ft).map(|backend| (backend, group))
				})
				.collect();
			let workers = rayon::current_num_threads().max(1);
			groups
				.into_par_iter()
				.map(|(backend, group)| {
					let chunk_size =
						group.len().div_ceil(workers).clamp(1, RUST_BATCH_SIZE);
					group
						.par_chunks(chunk_size)
						.map(|chunk| {
							format_batch_chunk(backend, chunk, check, debug)
						})
						.reduce(FormatStats::default, FormatStats::merge)
				})
				.reduce(FormatStats::default, FormatStats::merge)
		},
		|| {
			sized_others
				.par_iter()
				.fold(FormatStats::default, |mut stats, (file, file_type, _)| {
					let result = format_file_with_type(file, *file_type, check)
						.map_err(|e| e.to_string());
					record(&mut stats, file, result, debug);
					stats
				})
				.reduce(FormatStats::default, FormatStats::merge)
		},
	);

	let _ = &mut sized_others;
	batched_stats.merge(other_stats)
}

/// Format a chunk of same-language files with a single batch invocation
fn format_batch_chunk(
	backend: BatchFn,
	files: &[&PathBuf],
	check: bool,
	debug: bool,
) -> FormatStats {
	let mut stats = FormatStats::default();
	let mut contents: Vec<String> = Vec::with_capacity(files.len());
	let mut kept: Vec<&PathBuf> = Vec::with_capacity(files.len());

	for file in files {
		match fs::read_to_string(file) {
			Ok(content) => {
				contents.push(content);
				kept.push(file);
			}
			Err(e) => record(&mut stats, file, Err(e.to_string()), debug),
		}
	}

	if contents.is_empty() {
		return stats;
	}

	let refs: Vec<&str> = contents.iter().map(String::as_str).collect();
	for ((file, original), formatted) in
		kept.iter().zip(&contents).zip(backend(&refs))
	{
		let result = formatted.and_then(|formatted| {
			let changed = formatted != *original;
			if changed && !check {
				fs::write(file, formatted)
					.map_err(|e| format!("{}: {}", file.display(), e))?;
			}
			Ok(changed)
		});
		record(&mut stats, file, result, debug);
	}

	stats
}

/// Format a single file using an already-detected `FileType`
fn format_file_with_type(
	file_path: &PathBuf,
	file_type: FileType,
	check: bool,
) -> anyhow::Result<bool> {
	let content = fs::read_to_string(file_path)?;
	let path_str = file_path.to_str().unwrap_or("");

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

/// Format a single file based on its detected type
/// Returns true if the file was changed (or would be changed in check mode)
#[cfg(test)]
pub fn format_file(file_path: &PathBuf, check: bool) -> anyhow::Result<bool> {
	let path_str = file_path.to_str().unwrap_or("");
	let file_type = detect_file_type(path_str);
	format_file_with_type(file_path, file_type, check)
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
		// Kotlin goes through the persistent ktfmt-worker when one is
		// available (FAMA_KTFMT_WORKER or `ktfmt-worker` on PATH); the client
		// falls back to the process-based ktfmt path otherwise.
		FileType::Kotlin => fama_kotlin_worker::format_kotlin(content, path),
		FileType::CMake => cmake_fmt::format_cmake(content, path),
		FileType::Nix => nix_fmt::format_nix(content, path),
		FileType::Scala => scala_fmt::format_scala(content, path),
		FileType::R => air_fmt::format_r(content, path),
		FileType::Elixir => elixir_fmt::format_elixir(content, path),
		FileType::Wgsl => wgsl_fmt::format_wgsl(content, path),

		// Dart -> dart_style compiled to WASM, hosted in-process on wasmtime
		FileType::Dart => fama_dart::format_file(content, path, file_type),

		// Swift -> swift-format compiled to WASM, hosted in-process on wasmtime
		FileType::Swift => fama_swift::format_file(content, path, file_type),

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
		assert!(result.unwrap());
	}

	#[test]
	fn test_format_file_check_mode() {
		let temp_dir = TempDir::new().unwrap();
		let file_path = temp_dir.path().join("test.json");
		fs::write(&file_path, r#"{"key":   "value"}"#).unwrap();
		let original_content = fs::read_to_string(&file_path).unwrap();

		let result = format_file(&file_path, true);

		assert!(result.is_ok());
		assert!(result.unwrap());
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
		assert!(result
			.unwrap_err()
			.to_string()
			.contains("Unknown file type"));
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
		let result =
			format_content(content, "Dockerfile", FileType::Dockerfile);

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
	fn test_format_content_dart() {
		let content = "void main(){print('hello');}";
		let result = format_content(content, "test.dart", FileType::Dart);

		assert!(result.is_ok(), "dart formatting failed: {:?}", result);
		let formatted = result.unwrap();
		assert!(formatted.contains("print('hello')"), "Output: {formatted}");
	}

	#[test]
	fn test_format_content_swift() {
		let content = "func f()->Int{return 42}";
		let result = format_content(content, "test.swift", FileType::Swift);

		assert!(result.is_ok(), "swift formatting failed: {:?}", result);
		let formatted = result.unwrap();
		assert!(formatted.contains("return 42"), "Output: {formatted}");
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
