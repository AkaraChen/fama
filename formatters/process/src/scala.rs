//! Scala formatting via the scalafmt native binary (batched process mode).

use fama_common::Formatter;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::OnceLock;
use tempfile::TempDir;

/// Formatter implementation for Scala via `scalafmt`.
pub struct ScalaFormatter;

impl Formatter for ScalaFormatter {
	fn name(&self) -> &'static str {
		"scala"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["scala", "sc"]
	}

	fn format(&self, content: &str, path: &Path) -> Result<String, String> {
		format_scala(content, path.to_str().unwrap_or(""))
	}

	fn supports_batch(&self) -> bool {
		true
	}

	fn format_batch(&self, contents: &[&str]) -> Vec<Result<String, String>> {
		format_batch(contents)
	}
}

const SCALAFMT: &str = "scalafmt";
const DIALECT: &str = "scala213";

pub fn format_scala(source: &str, _path: &str) -> Result<String, String> {
	format_via_stdin(source)
}

pub fn format_batch(contents: &[&str]) -> Vec<Result<String, String>> {
	if contents.is_empty() {
		return Vec::new();
	}
	if contents.len() == 1 {
		return vec![format_via_stdin(contents[0])];
	}

	match format_batch_inner(contents) {
		Ok(results) => results.into_iter().map(Ok).collect(),
		Err(err) if err.contains("was not found in PATH") => {
			(0..contents.len()).map(|_| Err(err.clone())).collect()
		}
		Err(_) => {
			let mid = contents.len() / 2;
			let (left, right) = std::thread::scope(|s| {
				let h = s.spawn(|| format_batch(&contents[..mid]));
				let right = format_batch(&contents[mid..]);
				let left = h.join().expect("scalafmt bisect thread panicked");
				(left, right)
			});
			let mut combined = left;
			combined.extend(right);
			combined
		}
	}
}

fn format_via_stdin(source: &str) -> Result<String, String> {
	let config = config_str()?;

	let mut child = Command::new(SCALAFMT)
		.args(["--stdin", "--config-str", &config])
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()
		.map_err(command_error)?;

	child
		.stdin
		.take()
		.ok_or_else(|| format!("{SCALAFMT} stdin was not available"))?
		.write_all(source.as_bytes())
		.map_err(|error| {
			format!("Failed to write to {SCALAFMT} stdin: {error}")
		})?;

	let output = child
		.wait_with_output()
		.map_err(|error| format!("Failed to wait for {SCALAFMT}: {error}"))?;

	success_status(&output)?;
	String::from_utf8(output.stdout).map_err(|error| {
		format!("{SCALAFMT} output was not valid UTF-8: {error}")
	})
}

fn format_batch_inner(contents: &[&str]) -> Result<Vec<String>, String> {
	let config = config_str()?;

	let temp_dir = TempDir::new().map_err(|error| {
		format!("Failed to create temp dir for {SCALAFMT}: {error}")
	})?;

	let mut file_names = Vec::with_capacity(contents.len());
	for (index, content) in contents.iter().enumerate() {
		let name = format!("input_{index}.scala");
		fs::write(temp_dir.path().join(&name), content).map_err(|error| {
			format!("Failed to write temp file for {SCALAFMT}: {error}")
		})?;
		file_names.push(name);
	}

	let output = Command::new(SCALAFMT)
		.args(["--config-str", &config])
		.args(&file_names)
		.current_dir(temp_dir.path())
		.output()
		.map_err(command_error)?;
	success_status(&output)?;

	file_names
		.iter()
		.map(|name| {
			fs::read_to_string(temp_dir.path().join(name)).map_err(|error| {
				format!("Failed to read {SCALAFMT} output: {error}")
			})
		})
		.collect()
}

fn config_str() -> Result<String, String> {
	let version = scalafmt_version()?;

	Ok(format!(
		"version = {version}\n\
		runner.dialect = {DIALECT}\n\
		lineEndings = unix\n\
		maxColumn = {}\n\
		indent.main = {}",
		fama_common::CONFIG.line_width, fama_common::CONFIG.indent_width
	))
}

fn scalafmt_version() -> Result<String, String> {
	static VERSION: OnceLock<Result<String, String>> = OnceLock::new();

	VERSION
		.get_or_init(|| {
			let output = Command::new(SCALAFMT)
				.arg("--version")
				.output()
				.map_err(command_error)?;
			success_status(&output)?;

			let stdout = String::from_utf8_lossy(&output.stdout);
			stdout
				.split_whitespace()
				.next_back()
				.map(str::to_string)
				.ok_or_else(|| format!("{SCALAFMT} printed an empty version"))
		})
		.clone()
}

fn success_status(output: &Output) -> Result<(), String> {
	if output.status.success() {
		return Ok(());
	}

	let stderr = String::from_utf8_lossy(&output.stderr);
	let stdout = String::from_utf8_lossy(&output.stdout);
	let detail = if stderr.trim().is_empty() {
		stdout.trim()
	} else {
		stderr.trim()
	};

	let status = output.status;
	if detail.is_empty() {
		Err(format!("{SCALAFMT} failed with status {status}"))
	} else {
		Err(format!("{SCALAFMT} failed with status {status}: {detail}"))
	}
}

fn command_error(error: std::io::Error) -> String {
	if error.kind() == std::io::ErrorKind::NotFound {
		return format!(
			"{SCALAFMT} CLI was not found in PATH. Install `{SCALAFMT}` to format this language."
		);
	}

	format!("Failed to start {SCALAFMT}: {error}")
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::env;
	use std::path::{Path, PathBuf};

	const DOWNLOAD_URL: &str =
		"https://github.com/scalameta/scalafmt/releases/latest/download";

	static READY: OnceLock<bool> = OnceLock::new();

	fn ensure_scalafmt() -> bool {
		*READY.get_or_init(acquire_scalafmt)
	}

	fn acquire_scalafmt() -> bool {
		if runs(Command::new(SCALAFMT).arg("--version")) {
			return true;
		}

		if let Ok(path) = env::var("FAMA_SCALAFMT_BIN") {
			let path = PathBuf::from(path);
			if runs(Command::new(&path).arg("--version")) {
				if let Some(dir) = path.parent() {
					prepend_path(dir);
					return true;
				}
			}
			return false;
		}

		download_scalafmt()
	}

	#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
	fn download_scalafmt() -> bool {
		for dir in cache_dirs() {
			let target = dir.join(SCALAFMT);
			if runs(Command::new(&target).arg("--version")) {
				prepend_path(&dir);
				return true;
			}

			for asset in ["scalafmt-linux-glibc", "scalafmt-linux-musl"] {
				if download_to(&target, asset)
					&& chmod_executable(&target)
					&& runs(Command::new(&target).arg("--version"))
				{
					prepend_path(&dir);
					return true;
				}
			}
		}

		false
	}

	#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
	fn download_scalafmt() -> bool {
		false
	}

	fn cache_dirs() -> Vec<PathBuf> {
		let mut dirs = Vec::new();
		if let Ok(home) = env::var("HOME") {
			dirs.push(PathBuf::from(&home).join(".cache").join("fama-scala-test"));
		}
		dirs.push(env::temp_dir().join("fama-scala-test"));

		dirs.retain(|dir| {
			let probe = dir.join(".probe");
			let writable = fs::create_dir_all(dir).is_ok()
				&& fs::write(&probe, b"").is_ok();
			let _ = fs::remove_file(&probe);
			if !writable {
				eprintln!("skipping cache dir {}: not writable", dir.display());
			}
			writable
		});
		dirs
	}

	fn download_to(target: &Path, asset: &str) -> bool {
		let status = Command::new("curl")
			.args([
				"-sL",
				"--retry",
				"3",
				"--retry-all-errors",
				"-o",
			])
			.arg(target)
			.arg(format!("{DOWNLOAD_URL}/{asset}"))
			.status();
		status.map(|status| status.success() && target.exists()).unwrap_or(false)
	}

	#[cfg(unix)]
	fn chmod_executable(target: &Path) -> bool {
		use std::os::unix::fs::PermissionsExt;

		fs::set_permissions(target, fs::Permissions::from_mode(0o755)).is_ok()
	}

	#[cfg(not(unix))]
	fn chmod_executable(_target: &Path) -> bool {
		true
	}

	fn runs(command: &mut Command) -> bool {
		command
			.output()
			.map(|output| output.status.success())
			.unwrap_or(false)
	}

	fn prepend_path(dir: &Path) {
		let mut paths: Vec<PathBuf> = env::var_os("PATH")
			.as_deref()
			.map(env::split_paths)
			.map(|paths| paths.collect())
			.unwrap_or_default();
		paths.insert(0, dir.to_path_buf());
		if let Ok(joined) = env::join_paths(&paths) {
			env::set_var("PATH", joined);
		}
	}

	#[test]
	fn test_format_batch_empty() {
		assert!(format_batch(&[]).is_empty());
	}

	#[test]
	fn test_format_scala_reformats_unformatted_source() {
		if !ensure_scalafmt() {
			eprintln!("skipping: no scalafmt binary available");
			return;
		}

		let output =
			format_scala("object   Foo {def add(a:Int,b:Int)=a+b}", "A.scala")
				.unwrap();
		assert_eq!(
			output,
			"object Foo { def add(a: Int, b: Int) = a + b }\n"
		);
	}

	#[test]
	fn test_format_scala_parse_error_is_err() {
		if !ensure_scalafmt() {
			eprintln!("skipping: no scalafmt binary available");
			return;
		}

		let error =
			format_scala("object Bad{def a = }", "Bad.scala").unwrap_err();
		assert!(error.contains("error"), "unexpected message: {error}");
	}

	#[test]
	fn test_format_scala_uses_lf_line_endings() {
		if !ensure_scalafmt() {
			eprintln!("skipping: no scalafmt binary available");
			return;
		}

		let output =
			format_scala("object Crlf {def a = 1}\r\n", "C.scala").unwrap();
		assert!(!output.contains('\r'));
	}

	#[test]
	fn test_format_batch_all_valid_formats_everything() {
		if !ensure_scalafmt() {
			eprintln!("skipping: no scalafmt binary available");
			return;
		}

		let results = format_batch(&[
			"object A{def a = 1}\n",
			"object   B {def b = 2}\n",
		]);
		assert_eq!(results.len(), 2);
		assert_eq!(results[0].as_deref().unwrap(), "object A { def a = 1 }\n");
		assert_eq!(results[1].as_deref().unwrap(), "object B { def b = 2 }\n");
	}

	#[test]
	fn test_format_batch_preserves_order_and_isolates_errors() {
		if !ensure_scalafmt() {
			eprintln!("skipping: no scalafmt binary available");
			return;
		}

		let results = format_batch(&[
			"object B{def g = 2}\n",
			"object Bad{def a = }\n",
			"object   C {def h = 3}\n",
		]);
		assert_eq!(results.len(), 3);
		assert!(results[0].is_ok());
		assert!(results[1].is_err());
		assert!(results[2].is_ok());
		assert_eq!(results[0].as_deref().unwrap(), "object B { def g = 2 }\n");
		assert_eq!(results[2].as_deref().unwrap(), "object C { def h = 3 }\n");
	}

	#[test]
	fn test_format_batch_is_idempotent() {
		if !ensure_scalafmt() {
			eprintln!("skipping: no scalafmt binary available");
			return;
		}

		let sources = [
			"object Doc {\n  val s = \"\"\"  keep   strip margins\"\"\"\n  def m(i: Int) = i + 1\n}\n",
			"object   Two {def a:Int= {1}}\n",
		];
		let once: Vec<String> = format_batch(&sources)
			.into_iter()
			.map(|result| result.unwrap())
			.collect();
		let again: Vec<String> = format_batch(
			&once.iter().map(String::as_str).collect::<Vec<_>>(),
		)
		.into_iter()
		.map(|result| result.unwrap())
		.collect();

		assert_eq!(once, again);
	}
}
