//! Rust formatter using rustfmt
//!
//! Single-file path uses the rust-format crate (stdin). Batch path invokes the
//! rustfmt CLI once per chunk with multiple file paths to amortize process spawn.
//! Chunks run in parallel via rayon; path matching avoids canonicalize.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use fama_common::{FileType, CONFIG};
use rayon::prelude::*;
use rust_format::{Config, Formatter, RustFmt};
use rustc_hash::FxHashSet;

// Module-level constants - pre-converted config values
const RUSTFMT_HARD_TABS: &str =
	if matches!(CONFIG.indent_style, fama_common::IndentStyle::Tabs) {
		"true"
	} else {
		"false"
	};
// Note: These need to be string literals for const, so we use fixed values
// matching CONFIG defaults. If CONFIG changes, update these.
const RUSTFMT_TAB_SPACES: &str = "4";
const RUSTFMT_MAX_WIDTH: &str = "80";
const RUSTFMT_NEWLINE_STYLE: &str = match CONFIG.line_ending {
	fama_common::LineEnding::Lf => "Unix",
	fama_common::LineEnding::Crlf => "Windows",
};

/// Files per rustfmt CLI invocation (amortize spawn; stay under ARG_MAX).
const RUSTFMT_BATCH_CHUNK: usize = 64;

/// Cached formatter; CONFIG is process-static so one instance is enough.
static RUST_FMT: OnceLock<RustFmt> = OnceLock::new();

/// Cached `--config key=val,...` string for the multi-path CLI path.
static RUSTFMT_CONFIG_STR: OnceLock<String> = OnceLock::new();

/// Cached absolute CWD for cheap absolute-join path equality (no canonicalize).
static CWD: OnceLock<PathBuf> = OnceLock::new();

fn get_formatter() -> &'static RustFmt {
	RUST_FMT.get_or_init(|| {
		let config = Config::new_str()
			.option("hard_tabs", RUSTFMT_HARD_TABS)
			.option("tab_spaces", RUSTFMT_TAB_SPACES)
			.option("max_width", RUSTFMT_MAX_WIDTH)
			.option("newline_style", RUSTFMT_NEWLINE_STYLE);
		RustFmt::from_config(config)
	})
}

fn rustfmt_config_str() -> &'static str {
	RUSTFMT_CONFIG_STR.get_or_init(|| {
		format!(
			"hard_tabs={},tab_spaces={},max_width={},newline_style={}",
			RUSTFMT_HARD_TABS,
			RUSTFMT_TAB_SPACES,
			RUSTFMT_MAX_WIDTH,
			RUSTFMT_NEWLINE_STYLE
		)
	})
}

fn cwd() -> &'static Path {
	CWD.get_or_init(|| {
		std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
	})
	.as_path()
}

fn rustfmt_bin() -> PathBuf {
	std::env::var_os("RUSTFMT")
		.map(PathBuf::from)
		.unwrap_or_else(|| PathBuf::from("rustfmt"))
}

/// Format Rust source code (single string via rustfmt stdin).
///
/// # Arguments
/// * `source` - The Rust source code to format
/// * `file_path` - The file path (used for error reporting, currently unused)
///
/// # Returns
/// The formatted Rust source code, or an error message if formatting fails.
pub fn format_rust(source: &str, _file_path: &str) -> Result<String, String> {
	get_formatter()
		.format_str(source)
		.map_err(|e| format!("rustfmt error: {}", e))
}

/// Result of batch-formatting Rust files via multi-path rustfmt CLI.
#[derive(Debug, Default)]
pub struct RustBatchResult {
	/// Paths that were reformatted (or would be in check mode).
	pub changed: Vec<PathBuf>,
	/// Paths that were already correctly formatted.
	pub unchanged: Vec<PathBuf>,
	/// Spawn / hard-failure errors.
	pub errors: Vec<String>,
}

/// Batch-format Rust files by spawning rustfmt once per chunk with multiple paths.
///
/// Chunks of [`RUSTFMT_BATCH_CHUNK`] are processed in parallel via rayon.
///
/// - `check == false`: rustfmt rewrites dirty files in place (`-l` lists them).
/// - `check == true`: rustfmt `--check -l` only reports files that need formatting.
///
/// Note: path mode may also touch child modules discovered via `mod` (rustfmt default).
/// Bench/corpus files are independent; single-file `format_rust` still uses stdin.
pub fn format_rust_paths(paths: &[PathBuf], check: bool) -> RustBatchResult {
	if paths.is_empty() {
		return RustBatchResult::default();
	}

	let bin = rustfmt_bin();
	let chunks: Vec<&[PathBuf]> = paths.chunks(RUSTFMT_BATCH_CHUNK).collect();

	// Parallelize independent rustfmt CLI spawns across chunks.
	let chunk_results: Vec<RustBatchResult> = chunks
		.into_par_iter()
		.map(|chunk| match run_rustfmt_chunk(&bin, chunk, check) {
			Ok(r) => r,
			Err(e) => {
				let mut r = RustBatchResult::default();
				for p in chunk {
					r.errors.push(format!("{}: {}", p.display(), e));
				}
				r
			}
		})
		.collect();

	let mut result = RustBatchResult::default();
	for chunk_result in chunk_results {
		result.changed.extend(chunk_result.changed);
		result.unchanged.extend(chunk_result.unchanged);
		result.errors.extend(chunk_result.errors);
	}
	result
}

fn run_rustfmt_chunk(
	bin: &Path,
	chunk: &[PathBuf],
	check: bool,
) -> Result<RustBatchResult, String> {
	let mut cmd = Command::new(bin);
	cmd.stdin(Stdio::null())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.arg("--edition")
		.arg("2021")
		.arg("--config")
		.arg(rustfmt_config_str())
		.arg("-l"); // --files-with-diff

	if check {
		cmd.arg("--check");
	}

	for p in chunk {
		cmd.arg(p);
	}

	let output = cmd
		.output()
		.map_err(|e| format!("failed to spawn rustfmt: {}", e))?;

	let stdout = String::from_utf8_lossy(&output.stdout);
	let stderr = String::from_utf8_lossy(&output.stderr);
	let stderr_trim = stderr.trim();

	// Paths printed by -l (files that differ / were rewritten).
	let mut changed_set: FxHashSet<PathBuf> =
		FxHashSet::with_capacity_and_hasher(chunk.len(), Default::default());
	for line in stdout.lines() {
		let line = line.trim();
		if line.is_empty() {
			continue;
		}
		changed_set.insert(normalize_path(Path::new(line)));
	}

	// Paths mentioned in rustfmt/rustc diagnostics (syntax/parse failures).
	// Must not be classified as Unchanged when -l omits them.
	let error_set = error_paths_from_stderr(&stderr);

	let mut result = RustBatchResult::default();
	for p in chunk {
		let key = normalize_path(p);
		if changed_set.contains(&key) {
			result.changed.push(p.clone());
		} else if error_set.contains(&key) {
			result.errors.push(format!(
				"{}: {}",
				p.display(),
				summarize_stderr_for_path(&stderr, p)
			));
		} else {
			result.unchanged.push(p.clone());
		}
	}

	// Non-zero exit with no -l hits and no parsed path diagnostics, but
	// non-empty stderr (e.g. parse failure format we did not map): treat as
	// hard failure for every path still marked unchanged — never false-green.
	if !output.status.success() && !stderr_trim.is_empty() {
		let only_check_diffs = check
			&& output.status.code() == Some(1)
			&& !result.changed.is_empty()
			&& result.errors.is_empty()
			&& error_set.is_empty();
		// Pure formatting diffs in check mode: exit 1 + -l paths is OK.
		if only_check_diffs {
			return Ok(result);
		}

		// Syntax-only failure: exit 1, empty -l, empty error_set parse — or
		// residual unchanged paths while stderr reports errors.
		if result.errors.is_empty() && result.changed.is_empty() {
			// All paths looked "unchanged" but rustfmt failed → errors.
			result.unchanged.clear();
			for p in chunk {
				result.errors.push(format!(
					"{}: {}",
					p.display(),
					stderr_trim
				));
			}
		} else if !result.unchanged.is_empty() && !error_set.is_empty() {
			// Mixed chunk already classified some errors; leave remaining
			// unchanged only if they truly did not appear in diagnostics.
			// (error_set path already handled above)
		} else if result.errors.is_empty()
			&& !result.unchanged.is_empty()
			&& !stderr_trim.is_empty()
			&& result.changed.is_empty()
		{
			// Fallback already handled above.
		} else if !check && output.status.code() != Some(0) && result.errors.is_empty()
		{
			// Write mode hard failure without classified errors.
			if result.changed.is_empty() {
				return Err(format!("rustfmt error: {}", stderr_trim));
			}
			result.errors.push(format!("rustfmt: {}", stderr_trim));
		}
	}

	Ok(result)
}

/// Collect file paths from rustfmt/rustc-style diagnostics (`--> path:line:col`).
fn error_paths_from_stderr(stderr: &str) -> FxHashSet<PathBuf> {
	let mut set =
		FxHashSet::with_capacity_and_hasher(4, Default::default());
	for line in stderr.lines() {
		let line = line.trim();
		let rest = if let Some(r) = line.strip_prefix("-->") {
			r.trim()
		} else {
			continue;
		};
		if let Some(path) = path_from_location_suffix(rest) {
			set.insert(normalize_path(Path::new(path)));
		}
	}
	set
}

/// `file.rs:1:12` or `/abs/file.rs:1:12` → path without `:line:col`.
fn path_from_location_suffix(s: &str) -> Option<&str> {
	let mut parts = s.rsplitn(3, ':');
	let col = parts.next()?;
	let line = parts.next()?;
	let path = parts.next()?;
	if !line.is_empty()
		&& line.chars().all(|c| c.is_ascii_digit())
		&& !col.is_empty()
		&& col.chars().all(|c| c.is_ascii_digit())
	{
		Some(path)
	} else {
		None
	}
}

/// Prefer a short diagnostic line that mentions `path`; else first error line.
fn summarize_stderr_for_path(stderr: &str, path: &Path) -> String {
	let name = path
		.file_name()
		.and_then(|n| n.to_str())
		.unwrap_or("");
	let path_str = path.to_string_lossy();
	for line in stderr.lines() {
		let t = line.trim();
		if t.starts_with("error")
			|| t.contains(name)
			|| t.contains(path_str.as_ref())
		{
			if !t.is_empty() {
				return t.to_string();
			}
		}
	}
	let first = stderr
		.lines()
		.map(str::trim)
		.find(|l| !l.is_empty())
		.unwrap_or("rustfmt failed");
	first.to_string()
}

/// Normalize a path for equality with rustfmt `-l` output.
///
/// rustfmt may print absolute or relative paths, and on macOS may resolve
/// `/var` → `/private/var`. Use one `canonicalize` when the path exists so
/// both sides land on the same real path. Fall back to absolute-join when the
/// path is missing (rare for our batch inputs).
fn normalize_path(path: &Path) -> PathBuf {
	if let Ok(canon) = path.canonicalize() {
		return canon;
	}
	if path.is_absolute() {
		path.to_path_buf()
	} else {
		cwd().join(path)
	}
}

/// Format a file based on its file type
pub fn format_file(
	source: &str,
	file_path: &str,
	file_type: FileType,
) -> Result<String, String> {
	match file_type {
		FileType::Rust => format_rust(source, file_path),
		_ => Err(format!(
			"File type {:?} is not supported by rust-formatter",
			file_type
		)),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::io::Write;

	#[test]
	fn test_format_simple_function() {
		let source = r#"fn main(){println!("Hello");}"#;
		let result = format_rust(source, "test.rs").unwrap();

		// Check that formatting occurred (should have proper indentation)
		assert!(result.contains("fn main()"));
		assert!(result.contains("println!"));
	}

	#[test]
	fn test_format_file_with_rust() {
		let source = r#"fn main(){println!("Hello");}"#;
		let result = format_file(source, "test.rs", FileType::Rust).unwrap();

		// Check that formatting occurred
		assert!(result.contains("fn main()"));
		assert!(result.contains("println!"));
	}

	#[test]
	fn test_format_file_with_unsupported_type() {
		let source = "test";
		let result = format_file(source, "test.js", FileType::JavaScript);
		assert!(result.is_err());
	}

	#[test]
	fn test_format_rust_paths_check() {
		let dir = std::env::temp_dir().join(format!(
			"fama_rustfmt_batch_{}",
			std::process::id()
		));
		let _ = std::fs::remove_dir_all(&dir);
		std::fs::create_dir_all(&dir).unwrap();

		let dirty = dir.join("dirty.rs");
		let mut f = std::fs::File::create(&dirty).unwrap();
		write!(f, "fn main(){{println!(\"x\");}}").unwrap();
		drop(f);

		// Already-formatted sample via format_rust for consistency
		let clean_src =
			format_rust("fn main(){println!(\"y\");}", "c.rs").unwrap();
		let clean = dir.join("clean.rs");
		std::fs::write(&clean, &clean_src).unwrap();

		let result = format_rust_paths(&[dirty.clone(), clean.clone()], true);
		assert!(
			result.errors.is_empty(),
			"errors: {:?}",
			result.errors
		);
		assert_eq!(result.changed.len(), 1, "changed: {:?}", result.changed);
		assert_eq!(
			result.unchanged.len(),
			1,
			"unchanged: {:?}",
			result.unchanged
		);
		assert!(result.changed.iter().any(|p| p == &dirty));
		assert!(result.unchanged.iter().any(|p| p == &clean));

		// check mode must not rewrite
		let still = std::fs::read_to_string(&dirty).unwrap();
		assert!(still.contains("fn main(){println!"));

		let _ = std::fs::remove_dir_all(&dir);
	}

	#[test]
	fn test_format_rust_paths_write() {
		let dir = std::env::temp_dir().join(format!(
			"fama_rustfmt_batch_w_{}",
			std::process::id()
		));
		let _ = std::fs::remove_dir_all(&dir);
		std::fs::create_dir_all(&dir).unwrap();

		let dirty = dir.join("dirty.rs");
		std::fs::write(&dirty, "fn main(){println!(\"x\");}").unwrap();

		let result = format_rust_paths(&[dirty.clone()], false);
		assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
		assert_eq!(result.changed.len(), 1);
		let after = std::fs::read_to_string(&dirty).unwrap();
		assert!(after.contains("fn main()"));
		assert!(after.contains('\n'));

		let _ = std::fs::remove_dir_all(&dir);
	}

	#[test]
	fn test_normalize_path_absolute_and_relative() {
		let abs = PathBuf::from("/tmp/foo.rs");
		assert_eq!(normalize_path(&abs), abs);
		let rel = PathBuf::from("bar.rs");
		assert_eq!(normalize_path(&rel), cwd().join("bar.rs"));
	}

	#[test]
	fn test_format_rust_paths_check_syntax_error_not_unchanged() {
		// Skeptic: invalid Rust must not be classified Unchanged with 0 errors.
		let dir = std::env::temp_dir().join(format!(
			"fama_rustfmt_syn_{}",
			std::process::id()
		));
		let _ = std::fs::remove_dir_all(&dir);
		std::fs::create_dir_all(&dir).unwrap();

		let bad = dir.join("bad.rs");
		std::fs::write(&bad, "fn bad({{{{").unwrap();

		let result = format_rust_paths(&[bad.clone()], true);
		assert!(
			result.unchanged.is_empty(),
			"syntax error must not be Unchanged: {:?}",
			result.unchanged
		);
		assert!(
			result.changed.is_empty(),
			"syntax error must not be Changed: {:?}",
			result.changed
		);
		assert!(
			!result.errors.is_empty(),
			"expected errors for syntax failure, got: {:?}",
			result.errors
		);
		assert!(
			result.errors.iter().any(|e| e.contains("bad.rs")),
			"errors should name the file: {:?}",
			result.errors
		);

		let _ = std::fs::remove_dir_all(&dir);
	}

	#[test]
	fn test_format_rust_paths_check_mixed_dirty_and_syntax() {
		// Skeptic: dirty file still "needs format"; invalid file is Error, not Unchanged.
		let dir = std::env::temp_dir().join(format!(
			"fama_rustfmt_mix_{}",
			std::process::id()
		));
		let _ = std::fs::remove_dir_all(&dir);
		std::fs::create_dir_all(&dir).unwrap();

		let dirty = dir.join("dirty.rs");
		std::fs::write(&dirty, "fn main(){println!(\"x\");}").unwrap();
		let bad = dir.join("bad.rs");
		std::fs::write(&bad, "fn bad({{{{").unwrap();

		let result = format_rust_paths(&[dirty.clone(), bad.clone()], true);
		assert_eq!(
			result.changed.len(),
			1,
			"dirty should need formatting: {:?}",
			result.changed
		);
		assert!(
			result.changed.iter().any(|p| p == &dirty),
			"changed: {:?}",
			result.changed
		);
		assert!(
			!result.unchanged.iter().any(|p| p == &bad),
			"bad.rs must not be Unchanged: {:?}",
			result.unchanged
		);
		assert!(
			result.errors.iter().any(|e| e.contains("bad.rs")),
			"bad.rs must be an error: {:?}",
			result.errors
		);

		let _ = std::fs::remove_dir_all(&dir);
	}

	#[test]
	fn test_path_from_location_suffix() {
		assert_eq!(
			path_from_location_suffix("/tmp/bad.rs:1:12"),
			Some("/tmp/bad.rs")
		);
		assert_eq!(
			path_from_location_suffix("bad.rs:10:1"),
			Some("bad.rs")
		);
		assert_eq!(path_from_location_suffix("no_location"), None);
	}
}
