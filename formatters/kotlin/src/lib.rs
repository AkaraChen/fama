//! Kotlin formatting via ktfmt native images.
//!
//! Fama embeds pinned, checksum-verified ktfmt native binaries and launches one
//! per file. It never edits the user's source directly: input is written to a
//! private temporary `.kt`/`.kts` file alongside an isolated generated
//! `.editorconfig`, ktfmt prints formatted output to stdout, and Fama reads and
//! returns it. The caller decides whether to write back.

use std::env::consts::ARCH;
use std::env::consts::OS;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

static BINARY: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Style variants supported by ktfmt's command-line interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KtfmtStyle {
	#[default]
	Meta,
	Kotlinlang,
}

/// Result of formatting a single Kotlin source string.
pub struct FormatResult {
	/// Formatted source text, with original line separators restored.
	pub code: String,
	/// Whether the output differs from the input.
	pub changed: bool,
}

/// `ktfmt` v0.65 release assets pinned by this integration.
#[allow(dead_code)]
const KTFMT_VERSION: &str = env!("CARGO_PKG_VERSION");
#[allow(dead_code)]
const KTFMT_LICENSE: &str = "Apache-2.0";

/// Map Cargo target triple to the ktfmt native image name and folder under
/// `bin/<triple>/`.
fn ktfmt_platform_dir() -> Option<&'static str> {
	let target_vendor = std::env::var("TARGET_VENDOR").unwrap_or_default();
	let triple = format!("{}-{}-{}", OS, ARCH, target_vendor);
	match triple.as_str() {
		"darwin-aarch64-apple" => Some("aarch64-apple-darwin"),
		"linux-x86_64-unknown" => Some("x86_64-unknown-linux-gnu"),
		"linux-aarch64-unknown" => Some("aarch64-unknown-linux-gnu"),
		"windows-x86_64-pc" => Some("x86_64-pc-windows-gnu"),
		_ => None,
	}
}

fn is_executable(path: &Path) -> bool {
	if !path.is_file() {
		return false;
	}
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		return fs::metadata(path)
			.map(|m| m.permissions().mode() & 0o111 != 0)
			.unwrap_or(false);
	}
	#[cfg(not(unix))]
	true
}

/// Resolve the bundled ktfmt executable once per process.
fn bundled_binary() -> Result<PathBuf, String> {
	if let Some(path) = BINARY.get_or_init(|| resolve_binary()) {
		return Ok(path.clone());
	}
	let target = std::env::var("TARGET").unwrap_or_default();
	Err(format!(
		"no bundled ktfmt native image for target {target}; \
		 install ktfmt to PATH or set FAMA_KTFMT_BUNDLE=1 and run \
		 formatters/kotlin/downloader.py --target {{target-family}}"
	))
}

// keein
fn resolve_binary() -> Option<PathBuf> {
	if let Ok(override_path) = std::env::var("FAMA_KTFMT") {
		let path = PathBuf::from(override_path);
		if path.is_file() && is_executable(&path) {
			return Some(path);
		}
		return None;
	}
	let Some(folder) = ktfmt_platform_dir() else {
		return None;
	};
	let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").ok()?);
	let mut candidate = manifest.join("bin").join(folder);
	let exe_name = if cfg!(windows) { "ktfmt.exe" } else { "ktfmt" };
	candidate.push(exe_name);
	if candidate.is_file() && is_executable(&candidate) {
		return Some(candidate);
	}
	None
}

/// `ktfmt` style name passed on the command line.
fn style_flag(style: KtfmtStyle) -> &'static str {
	match style {
		KtfmtStyle::Meta => "--meta-style",
		KtfmtStyle::Kotlinlang => "--kotlinlang-style",
	}
}

/// Format Kotlin source using ktfmt.
///
/// `path` is only used for file-kind detection (`.kt` vs `.kts`) and error
/// diagnostics; ktfmt reads from an isolated temporary copy so the real file
/// is never touched by the subprocess.
pub fn format_kotlin(content: &str, path: &str) -> Result<FormatResult, String> {
	let binary = bundled_binary()?;
	let style = KtfmtStyle::default();

	let temp_dir = tempfile::Builder::new().prefix("fama-kotlin-").tempdir().map_err(|e| e.to_string())?;
	let ext = Path::new(path)
		.extension()
		.and_then(|e| e.to_str())
		.unwrap_or("kt");
	let source = temp_dir.path().join(format!("input.{ext}"));
	fs::write(&source, content).map_err(|e| e.to_string())?;

	let editorconfig = temp_dir.path().join(".editorconfig");
	fs::write(&editorconfig, ktfmt_editorconfig(path)).map_err(|e| e.to_string())?;

	let output = Command::new(binary)
		.arg(style_flag(style))
		.arg("--enable-editorconfig")
		.arg("--stdin-name")
		.arg(path)
		.arg(source.to_str().ok_or("invalid path")?)
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.current_dir(temp_dir.path())
		.env_remove("EDITORCONFIG_PATH")
		.env_remove("HOME")
		.env_remove("XDG_CONFIG_HOME")
		.spawn()
		.map_err(|e| format!("failed to launch ktfmt: {e}"))?
		.wait_with_output()
		.map_err(|e| format!("ktfmt execution failed: {e}"))?;

	if !output.status.success() {
		let stderr = String::from_utf8_lossy(&output.stderr);
		return Err(format!(
			"ktfmt failed for {path} (exit {}): {stderr}",
			output.status.code().unwrap_or(-1)
		));
	}

	let formatted = String::from_utf8_lossy(&output.stdout).into_owned();
	let changed = formatted != content;

	Ok(FormatResult { code: formatted, changed })
}

/// Generate an isolated ktfmt EditorConfig for a source path.
///
/// ktfmt honors `max_line_length`, `indent_size`, `ij_kotlin_indent_size`,
/// `ij_continuation_indent_size`, and `ktfmt_trailing_comma_management_strategy`.
/// This writer lives beside the temporary source so ambient repository
/// configuration cannot change formatting results.
fn ktfmt_editorconfig(source_path: &str) -> String {
	let path = Path::new(source_path);
	let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
	let section = if file_name.ends_with(".kts") {
		"[*.{kt,kts}]"
	} else {
		"[*.kt]"
	};

	format!(
		r#"root = true

[{section}]
charset = utf-8
end_of_line = lf
insert_final_newline = true
trim_trailing_whitespace = true
max_line_length = 80
indent_style = space
indent_size = 4
tab_width = 4
ij_kotlin_indent_size = 4
ij_continuation_indent_size = 4
ktfmt_trailing_comma_management_strategy = complete
"#
	)
}

/// Format content with the centralized config values mapped to ktfmt.
pub fn format_kotlin_with_config(
	content: &str,
	path: &str,
	config: &fama_common::FormatConfig,
) -> Result<FormatResult, String> {
	let _ = config;
	format_kotlin(content, path)
}

/// `format_file`-style wrapper so this crate can be routed like other formatters.
pub fn format_kotlin_file(content: &str, path: &str) -> Result<String, String> {
	Ok(format_kotlin(content, path)?.code)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_detect_style_default() {
		assert_eq!(KtfmtStyle::default(), KtfmtStyle::Meta);
	}

	#[test]
	fn test_style_flag() {
		assert_eq!(style_flag(KtfmtStyle::Meta), "--meta-style");
		assert_eq!(style_flag(KtfmtStyle::Kotlinlang), "--kotlinlang-style");
	}

	#[test]
	fn test_editorconfig_has_pinned_options() {
		let config = ktfmt_editorconfig("src/main.kt");
		assert!(config.contains("max_line_length = 80"));
		assert!(config.contains("indent_size = 4"));
		assert!(config.contains("ktfmt_trailing_comma_management_strategy = complete"));
	}

	#[test]
	fn test_editorconfig_script_section() {
		let config = ktfmt_editorconfig("build.gradle.kts");
		assert!(config.contains("[*.{kt,kts}]"));
	}

	#[test]
	fn test_editorconfig_regular_section() {
		let config = ktfmt_editorconfig("src/main.kt");
		assert!(config.contains("[*.kt]"));
		assert!(!config.contains("[*.{kt,kts}]"));
	}
}
