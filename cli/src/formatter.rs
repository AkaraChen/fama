// formatter.rs - Format routing logic

use crate::registry::{registry, ResolveOutcome};
use fama_common::Formatter;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Maximum bytes read when disambiguating shared or content-guarded extensions.
const DISAMBIGUATION_PREFIX_BYTES: usize = 4096;

/// Number of files formatted per batch process invocation. Larger batches
/// amortize process startup further but hold more content in memory.
const BATCH_SIZE: usize = 64;

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

/// Read at most `max_bytes` from `path` for extension disambiguation.
fn read_prefix(path: &Path, max_bytes: usize) -> std::io::Result<String> {
	let file = fs::File::open(path)?;
	let mut buf = Vec::with_capacity(max_bytes.min(4096));
	file.take(max_bytes as u64).read_to_end(&mut buf)?;
	Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn route_resolved<'a>(
	id: usize,
	fmt: &'static dyn Formatter,
	file: &'a PathBuf,
	batch_groups: &mut HashMap<usize, (&'static dyn Formatter, Vec<&'a PathBuf>)>,
	other_files: &mut Vec<(&'a PathBuf, Option<&'static dyn Formatter>)>,
) {
	if fmt.supports_batch() {
		batch_groups
			.entry(id)
			.or_insert_with(|| (fmt, Vec::new()))
			.1
			.push(file);
	} else {
		other_files.push((file, Some(fmt)));
	}
}

/// Format all files, parallelizing per file and batching batchable formatters
/// concurrently, with large single files scheduled first (LPT) to minimize
/// tail latency.
pub fn format_files(
	files: &[PathBuf],
	check: bool,
	debug: bool,
) -> FormatStats {
	let reg = registry();
	let mut batch_groups: HashMap<usize, (&'static dyn Formatter, Vec<&PathBuf>)> =
		HashMap::new();
	let mut other_files: Vec<(&PathBuf, Option<&'static dyn Formatter>)> =
		Vec::new();
	let mut pre_stats = FormatStats::default();

	for file in files {
		match reg.resolve_path(file) {
			ResolveOutcome::Unique(id, fmt) => {
				route_resolved(
					id,
					fmt,
					file,
					&mut batch_groups,
					&mut other_files,
				);
			}
			ResolveOutcome::NeedsContent => {
				match read_prefix(file, DISAMBIGUATION_PREFIX_BYTES) {
					Ok(prefix) => {
						if let Some((id, fmt)) =
							reg.resolve_with_content(file, &prefix)
						{
							route_resolved(
								id,
								fmt,
								file,
								&mut batch_groups,
								&mut other_files,
							);
						}
						// If no candidate matched (fail-closed), skip gracefully.
					}
					Err(e) => {
						record(
							&mut pre_stats,
							file,
							Err(format!("{}: {}", file.display(), e)),
							debug,
						);
					}
				}
			}
			ResolveOutcome::None => {
				other_files.push((file, None));
			}
		}
	}

	// Longest-processing-time-first ordering for single-file formatters so large
	// files (e.g. megabyte C/TS bundles) start immediately instead of trailing.
	let mut sized_others: Vec<(&PathBuf, Option<&'static dyn Formatter>, u64)> =
		if other_files.len() > 1 {
			let mut v: Vec<(&PathBuf, Option<&'static dyn Formatter>, u64)> =
				other_files
					.into_par_iter()
					.map(|(file, fmt)| {
						let len =
							fs::metadata(file).map(|m| m.len()).unwrap_or(0);
						(file, fmt, len)
					})
					.collect();
			v.sort_unstable_by_key(|a| std::cmp::Reverse(a.2));
			v
		} else {
			other_files
				.into_iter()
				.map(|(file, fmt)| (file, fmt, 0))
				.collect()
		};

	let (batched_stats, other_stats) = rayon::join(
		|| {
			let groups: Vec<(&'static dyn Formatter, Vec<&PathBuf>)> =
				batch_groups.into_values().collect();
			let workers = rayon::current_num_threads().max(1);
			groups
				.into_par_iter()
				.map(|(fmt, group)| {
					let chunk_size =
						group.len().div_ceil(workers).clamp(1, BATCH_SIZE);
					group
						.par_chunks(chunk_size)
						.map(|chunk| format_batch_chunk(fmt, chunk, check, debug))
						.reduce(FormatStats::default, FormatStats::merge)
				})
				.reduce(FormatStats::default, FormatStats::merge)
		},
		|| {
			sized_others
				.par_iter()
				.fold(FormatStats::default, |mut stats, (file, fmt, _)| {
					let result = format_file_with_formatter(file, *fmt, check)
						.map_err(|e| e.to_string());
					record(&mut stats, file, result, debug);
					stats
				})
				.reduce(FormatStats::default, FormatStats::merge)
		},
	);

	let _ = &mut sized_others;
	pre_stats.merge(batched_stats).merge(other_stats)
}

/// Format a chunk of same-language files with a single batch invocation
fn format_batch_chunk(
	fmt: &dyn Formatter,
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
		kept.iter().zip(&contents).zip(fmt.format_batch(&refs))
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

/// Format a single file using an already-resolved `Formatter`
fn format_file_with_formatter(
	file_path: &Path,
	formatter: Option<&'static dyn Formatter>,
	check: bool,
) -> anyhow::Result<bool> {
	let content = fs::read_to_string(file_path)?;

	let formatted = match formatter {
		Some(fmt) => fmt
			.format(&content, file_path)
			.map_err(|e| anyhow::anyhow!("{}: {}", file_path.display(), e))?,
		None => anyhow::bail!("{}: Unknown file type", file_path.display()),
	};

	if formatted != content {
		if !check {
			fs::write(file_path, formatted)?;
		}
		Ok(true)
	} else {
		Ok(false)
	}
}

/// Format a single file based on registry lookup
/// Returns true if the file was changed (or would be changed in check mode)
#[cfg(test)]
pub fn format_file(file_path: &Path, check: bool) -> anyhow::Result<bool> {
	let formatter = match registry().resolve_path(file_path) {
		ResolveOutcome::Unique(_, fmt) => Some(fmt),
		ResolveOutcome::NeedsContent => {
			let prefix = read_prefix(file_path, DISAMBIGUATION_PREFIX_BYTES)
				.unwrap_or_default();
			registry()
				.resolve_with_content(file_path, &prefix)
				.map(|(_, fmt)| fmt)
		}
		ResolveOutcome::None => None,
	};
	format_file_with_formatter(file_path, formatter, check)
}

/// Format content string via registry lookup
#[cfg(test)]
fn format_content(content: &str, path: &str) -> Result<String, String> {
	let path = Path::new(path);
	match registry().resolve_with_content(path, content) {
		Some((_, fmt)) => fmt.format(content, path),
		None => Err("Unknown file type".to_string()),
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
		fs::write(&file_path, "{}").unwrap();

		let result = format_file(&file_path, false);

		assert!(result.is_ok());
	}

	#[test]
	fn test_format_file_with_changes() {
		let temp_dir = TempDir::new().unwrap();
		let file_path = temp_dir.path().join("test.json");
		fs::write(&file_path, r#"{"key":   "value"}"#).unwrap();

		let result = format_file(&file_path, false);

		assert!(result.is_ok());
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
		let result = format_content(content, "test.json");

		assert!(result.is_ok());
		let formatted = result.unwrap();
		assert!(formatted.contains("{"));
		assert!(formatted.contains("}"));
	}

	#[test]
	fn test_format_content_toml() {
		let content = "key=\"value\"";
		let result = format_content(content, "test.toml");

		assert!(result.is_ok());
		let formatted = result.unwrap();
		assert!(formatted.contains("key"));
		assert!(formatted.contains("value"));
	}

	#[test]
	fn test_format_content_rust() {
		let content = "fn main() {}";
		let result = format_content(content, "test.rs");

		assert!(result.is_ok());
		let formatted = result.unwrap();
		assert!(formatted.contains("fn main()"));
	}

	#[test]
	fn test_format_content_python() {
		let content = "x=1";
		let result = format_content(content, "test.py");

		assert!(result.is_ok());
		let formatted = result.unwrap();
		assert!(formatted.contains("x = 1"));
	}

	#[test]
	fn test_format_content_lua() {
		let content = "x=1";
		let result = format_content(content, "test.lua");

		assert!(result.is_ok());
		let formatted = result.unwrap();
		assert!(formatted.contains("x = 1"));
	}

	#[test]
	fn test_format_content_shell() {
		let content = "echo hello";
		let result = format_content(content, "test.sh");

		assert!(result.is_ok());
		let formatted = result.unwrap();
		assert!(formatted.contains("echo"));
		assert!(formatted.contains("hello"));
	}

	#[test]
	fn test_format_content_go() {
		let content = "package main";
		let result = format_content(content, "test.go");

		assert!(result.is_ok());
		let formatted = result.unwrap();
		assert!(formatted.contains("package main"));
	}

	#[test]
	fn test_format_content_zig() {
		let content = "const x = 1;";
		let result = format_content(content, "test.zig");

		assert!(result.is_ok());
		let formatted = result.unwrap();
		assert!(formatted.contains("const x = 1"));
	}

	#[test]
	fn test_format_content_xml() {
		let content = "<root><item/></root>";
		let result = format_content(content, "test.xml");

		assert!(result.is_ok());
		let formatted = result.unwrap();
		assert!(formatted.contains("<root>"));
		assert!(formatted.contains("</root>"));
	}

	#[test]
	fn test_format_content_sql() {
		let content = "SELECT 1;";
		let result = format_content(content, "test.sql");

		assert!(result.is_ok());
		let formatted = result.unwrap();
		assert!(formatted.contains("SELECT"));
	}

	#[test]
	fn test_format_content_dockerfile() {
		let content = "FROM alpine\nRUN echo hi";
		let result = format_content(content, "Dockerfile");

		assert!(result.is_ok());
		let formatted = result.unwrap();
		assert!(formatted.contains("FROM alpine"));
	}

	#[test]
	fn test_format_content_hcl() {
		let content = "resource \"test\" \"name\" {}";
		let result = format_content(content, "test.hcl");

		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_php() {
		let content = "<?php echo 'hello'; ?>";
		let result = format_content(content, "test.php");

		assert!(result.is_ok());
		let formatted = result.unwrap();
		assert!(formatted.contains("<?php"));
	}

	#[test]
	fn test_format_content_ruby() {
		let content = "x = 1";
		let result = format_content(content, "test.rb");

		assert!(result.is_ok());
		let formatted = result.unwrap();
		assert!(formatted.contains("x = 1"));
	}

	#[test]
	fn test_format_content_dart() {
		let content = "void main(){print('hello');}";
		let result = format_content(content, "test.dart");

		assert!(result.is_ok(), "dart formatting failed: {:?}", result);
		let formatted = result.unwrap();
		assert!(formatted.contains("print('hello')"), "Output: {formatted}");
	}

	#[test]
	fn test_format_content_swift() {
		let content = "func f()->Int{return 42}";
		let result = format_content(content, "test.swift");

		assert!(result.is_ok(), "swift formatting failed: {:?}", result);
		let formatted = result.unwrap();
		assert!(formatted.contains("return 42"), "Output: {formatted}");
	}

	#[test]
	fn test_format_content_yaml() {
		let content = "key: value";
		let result = format_content(content, "test.yaml");

		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_markdown() {
		let content = "# Hello";
		let result = format_content(content, "test.md");

		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_mdx() {
		let content = "+++\ntitle=\"Hello\"\n+++\n\nimport Foo from './foo'\n\n#   Hello <Badge   variant='ok'>new</Badge>\n\n<Callout   type=\"info\">\n\n-   item {1+2}\n\n</Callout>\n";
		let formatted = format_content(content, "doc.mdx").unwrap();
		assert!(formatted.contains("title = \"Hello\""), "{formatted}");
		assert!(formatted.contains("import Foo from \"./foo\";"), "{formatted}");
		assert!(
			formatted.contains("# Hello <Badge variant=\"ok\">new</Badge>"),
			"{formatted}"
		);
		assert!(formatted.contains("- item {1 + 2}"), "{formatted}");
		let second = format_content(&formatted, "doc.mdx").unwrap();
		assert_eq!(formatted, second);
	}

	#[test]
	fn test_format_content_css() {
		let content = "a{color:red}";
		let result = format_content(content, "test.css");

		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_javascript() {
		let content = "const x=1;";
		let result = format_content(content, "test.js");

		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_typescript() {
		let content = "const x: number = 1;";
		let result = format_content(content, "test.ts");

		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_jsx() {
		let content = "const el = <div />;";
		let result = format_content(content, "test.jsx");

		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_tsx() {
		let content = "const el = <div />;";
		let result = format_content(content, "test.tsx");

		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_html() {
		let content = "<html><body>Hi</body></html>";
		let result = format_content(content, "test.html");

		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_vue() {
		let content = "<template><div>Hi</div></template>";
		let result = format_content(content, "test.vue");

		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_svelte() {
		let content = "<div>Hello</div>";
		let result = format_content(content, "test.svelte");

		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_astro() {
		let content = "---\nconst x = 1;\n---\n<div></div>";
		let result = format_content(content, "test.astro");

		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_graphql() {
		let content = "query { field }";
		let result = format_content(content, "test.graphql");

		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_c() {
		let content = "int main() { return 0; }";
		let result = format_content(content, "test.c");

		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_cpp() {
		let content = "int main() { return 0; }";
		let result = format_content(content, "test.cpp");

		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_csharp() {
		let content = "class Test {}";
		let result = format_content(content, "test.cs");

		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_java() {
		let content = "public class Test {}";
		let result = format_content(content, "test.java");

		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_objective_c() {
		let content =
			"#import <Foundation/Foundation.h>\n@interface Foo : NSObject\n@end\n";
		let result = format_content(content, "test.m");

		assert!(result.is_ok(), "Objective-C failed: {:?}", result);
	}

	#[test]
	fn test_format_content_verilog_and_fail_closed_disambiguation() {
		let verilog =
			"module top(input wire a,output wire b);assign b=a;endmodule\n";
		let formatted = format_content(verilog, "top.v").unwrap();
		assert!(formatted.contains("assign b = a;"), "{formatted}");

		// Coq/Rocq, V, and inconclusive .v fail closed
		assert!(format_content(
			"Require Import Arith.\nProof. reflexivity. Qed.\n",
			"proof.v"
		)
		.is_err());
		assert!(format_content(
			"module main\nfn main() {\n\tprintln('hi')\n}\n",
			"main.v"
		)
		.is_err());
		assert!(format_content("// inconclusive\n", "ambig.v").is_err());

		// Modeline overrides
		assert!(format_content(
			"// vim: ft=verilog\nmodule m;endmodule\n",
			"hint.v"
		)
		.is_ok());
		assert!(format_content(
			"// vim: ft=coq\nmodule top(input wire a);endmodule\n",
			"override.v"
		)
		.is_err());

		// MATLAB and inconclusive .m fail closed; modeline overrides
		assert!(format_content(
			"% MATLAB\nfunction y = f(x)\n  y = x + 1;\nend\n",
			"calc.m"
		)
		.is_err());
		assert!(format_content("int main() { return 0; }\n", "plain.m").is_err());
		assert!(format_content(
			"// vim: ft=objc\nint main(){return 0;}\n",
			"hint.m"
		)
		.is_ok());
	}

	#[test]
	fn test_format_files_skips_unmatched_ambiguous_files_without_error() {
		let temp_dir = TempDir::new().unwrap();

		let verilog_path = temp_dir.path().join("top.v");
		let coq_path = temp_dir.path().join("proof.v");
		let ambig_v_path = temp_dir.path().join("ambig.v");
		let modeline_v_path = temp_dir.path().join("modeline.v");
		let objc_path = temp_dir.path().join("App.m");
		let matlab_path = temp_dir.path().join("calc.m");
		let ambig_m_path = temp_dir.path().join("plain.m");
		let modeline_m_path = temp_dir.path().join("modeline.m");

		let coq_src = "Require Import Arith.\nTheorem t : 1=1. Proof. reflexivity. Qed.\n";
		let ambig_v_src = "// inconclusive\nx=1;\n";
		let matlab_src = "% MATLAB\nfunction y=f(x)\ny=x+1;\nend\n";
		let ambig_m_src = "int main(){return 0;}\n";

		fs::write(
			&verilog_path,
			"module top(input wire a,output wire b);assign b=a;endmodule\n",
		)
		.unwrap();
		fs::write(&coq_path, coq_src).unwrap();
		fs::write(&ambig_v_path, ambig_v_src).unwrap();
		fs::write(&modeline_v_path, "// vim: ft=verilog\nmodule m;assign a=b;endmodule\n")
			.unwrap();
		fs::write(
			&objc_path,
			"#import <Foundation/Foundation.h>\n@interface App:NSObject\n@end\n",
		)
		.unwrap();
		fs::write(&matlab_path, matlab_src).unwrap();
		fs::write(&ambig_m_path, ambig_m_src).unwrap();
		fs::write(&modeline_m_path, "// vim: ft=objc\nint main(){return 0;}\n")
			.unwrap();

		let files = vec![
			verilog_path.clone(),
			coq_path.clone(),
			ambig_v_path.clone(),
			modeline_v_path.clone(),
			objc_path.clone(),
			matlab_path.clone(),
			ambig_m_path.clone(),
			modeline_m_path.clone(),
		];

		let stats = format_files(&files, false, false);
		assert!(stats.errors.is_empty(), "unexpected errors: {:?}", stats.errors);
		assert_eq!(stats.formatted + stats.unchanged, 4);

		// Accepted files were formatted
		assert!(fs::read_to_string(&verilog_path)
			.unwrap()
			.contains("assign b = a;"));
		assert!(fs::read_to_string(&modeline_v_path)
			.unwrap()
			.contains("assign a = b;"));
		assert!(fs::read_to_string(&objc_path)
			.unwrap()
			.contains("@interface App : NSObject"));
		assert!(fs::read_to_string(&modeline_m_path)
			.unwrap()
			.contains("return 0;"));

		// Declined / inconclusive files were skipped untouched
		assert_eq!(fs::read_to_string(&coq_path).unwrap(), coq_src);
		assert_eq!(fs::read_to_string(&ambig_v_path).unwrap(), ambig_v_src);
		assert_eq!(fs::read_to_string(&matlab_path).unwrap(), matlab_src);
		assert_eq!(fs::read_to_string(&ambig_m_path).unwrap(), ambig_m_src);
	}

	#[test]
	fn test_format_content_protobuf() {
		let content = "syntax = \"proto3\";";
		let result = format_content(content, "test.proto");

		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_unknown() {
		let content = "anything";
		let result = format_content(content, "test.xyz");

		assert!(result.is_err());
		assert_eq!(result.unwrap_err(), "Unknown file type");
	}

	#[test]
	fn test_format_content_jsonc() {
		let content = "{\"key\": \"value\" // comment\n}";
		let result = format_content(content, "test.jsonc");

		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_scss() {
		let content = "a { color: red; }";
		let result = format_content(content, "test.scss");

		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_less() {
		let content = "a { color: red; }";
		let result = format_content(content, "test.less");

		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_sass() {
		let content = "a\n  color: red";
		let result = format_content(content, "test.sass");

		assert!(result.is_ok());
	}

	#[test]
	fn test_format_content_jupyter_python_and_markdown() {
		let notebook = r##"{
  "cells": [
    {
      "cell_type": "markdown",
      "metadata": {},
      "source": [
        "#   Title\n",
        "\n",
        "*  item"
      ]
    },
    {
      "cell_type": "code",
      "execution_count": null,
      "metadata": {},
      "outputs": [],
      "source": [
        "x=1+2\n",
        "def f( a , b ): return a+b"
      ]
    }
  ],
  "metadata": {
    "kernelspec": {
      "display_name": "Python 3",
      "language": "python",
      "name": "python3"
    },
    "language_info": {
      "name": "python"
    }
  },
  "nbformat": 4,
  "nbformat_minor": 5
}
"##;
		let formatted = format_content(notebook, "notebook.ipynb").unwrap();
		assert!(formatted.contains("x = 1 + 2"), "{formatted}");
		assert!(formatted.contains("def f(a, b):"), "{formatted}");
	}
}
