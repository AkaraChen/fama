//! Process-based formatter support for host-installed CLIs.

use fama_common::{editorconfig_contents, FormatConfig};
use std::ffi::OsStr;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessMode {
	StdinStdout,
	TempFile,
}

#[derive(Debug, Clone, Copy)]
pub struct ProcessFormatter {
	pub name: &'static str,
	pub command: &'static str,
	pub args: &'static [&'static str],
	pub mode: ProcessMode,
	pub write_editorconfig: bool,
}

impl ProcessFormatter {
	fn format(self, source: &str, file_path: &str) -> Result<String, String> {
		match self.mode {
			ProcessMode::StdinStdout => self.format_via_stdin(source),
			ProcessMode::TempFile => {
				self.format_via_temp_file(source, file_path)
			}
		}
	}

	fn format_via_stdin(self, source: &str) -> Result<String, String> {
		let mut command = Command::new(self.command);
		command
			.args(self.args)
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.stderr(Stdio::piped());

		let mut child = command
			.spawn()
			.map_err(|error| command_error(self.name, self.command, error))?;

		child
			.stdin
			.as_mut()
			.ok_or_else(|| format!("{} stdin was not available", self.name))?
			.write_all(source.as_bytes())
			.map_err(|error| {
				format!("Failed to write to {} stdin: {}", self.name, error)
			})?;

		let output = child.wait_with_output().map_err(|error| {
			format!("Failed to wait for {}: {}", self.name, error)
		})?;

		success_output(self.name, output)
	}

	fn format_via_temp_file(
		self,
		source: &str,
		original_path: &str,
	) -> Result<String, String> {
		let temp_dir = tempfile::tempdir().map_err(|error| {
			format!("Failed to create temp dir for {}: {}", self.name, error)
		})?;
		let temp_file = temp_file_path(temp_dir.path(), original_path);

		if self.write_editorconfig {
			let editorconfig_path = temp_dir.path().join(".editorconfig");
			fs::write(
				&editorconfig_path,
				editorconfig_contents(&FormatConfig::default()),
			)
			.map_err(|error| {
				format!(
					"Failed to write .editorconfig for {}: {}",
					self.name, error
				)
			})?;
		}

		fs::write(&temp_file, source).map_err(|error| {
			format!("Failed to write temp file for {}: {}", self.name, error)
		})?;

		let output = self
			.command_with_file(&temp_file)
			.current_dir(temp_dir.path())
			.output()
			.map_err(|error| command_error(self.name, self.command, error))?;
		success_status(self.name, &output)?;

		fs::read_to_string(&temp_file).map_err(|error| {
			format!("Failed to read {} output: {}", self.name, error)
		})
	}

	fn command_with_file(self, file_path: &Path) -> Command {
		let file_path = file_path.to_string_lossy();
		let mut command = Command::new(self.command);
		command.args(
			self.args
				.iter()
				.map(|arg| arg.replace("{file}", &file_path)),
		);
		command
	}
}

pub fn format_with_process(
	source: &str,
	file_path: &str,
	formatter: ProcessFormatter,
) -> Result<String, String> {
	formatter.format(source, file_path)
}

pub fn format_kotlin(source: &str, file_path: &str) -> Result<String, String> {
	format_with_process(
		source,
		file_path,
		ProcessFormatter {
			name: "ktfmt",
			command: "ktfmt",
			args: &["--kotlinlang-style", "--enable-editorconfig", "{file}"],
			mode: ProcessMode::TempFile,
			write_editorconfig: true,
		},
	)
}

fn temp_file_path(base_dir: &Path, original_path: &str) -> PathBuf {
	let file_name = Path::new(original_path)
		.file_name()
		.filter(|name| !name.is_empty())
		.unwrap_or_else(|| OsStr::new("input"));

	base_dir.join(file_name)
}

fn success_output(name: &str, output: Output) -> Result<String, String> {
	success_status(name, &output)?;
	String::from_utf8(output.stdout).map_err(|error| {
		format!("{} output was not valid UTF-8: {}", name, error)
	})
}

fn success_status(name: &str, output: &Output) -> Result<(), String> {
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

	if detail.is_empty() {
		Err(format!("{} failed with status {}", name, output.status))
	} else {
		Err(format!(
			"{} failed with status {}: {}",
			name, output.status, detail
		))
	}
}

fn command_error(name: &str, command: &str, error: std::io::Error) -> String {
	if error.kind() == ErrorKind::NotFound {
		return format!(
			"{} CLI was not found in PATH. Install `{}` to format this language.",
			name, command
		);
	}

	format!("Failed to start {} (`{}`): {}", name, command, error)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[cfg(unix)]
	fn stdin_formatter() -> ProcessFormatter {
		ProcessFormatter {
			name: "stdin-test",
			command: "sh",
			args: &["-c", "tr '[:lower:]' '[:upper:]'"],
			mode: ProcessMode::StdinStdout,
			write_editorconfig: false,
		}
	}

	#[cfg(windows)]
	fn stdin_formatter() -> ProcessFormatter {
		ProcessFormatter {
			name: "stdin-test",
			command: "powershell.exe",
			args: &[
				"-NoProfile",
				"-Command",
				"[Console]::Out.Write(([Console]::In.ReadToEnd()).ToUpperInvariant())",
			],
			mode: ProcessMode::StdinStdout,
			write_editorconfig: false,
		}
	}

	#[cfg(unix)]
	fn temp_file_formatter() -> ProcessFormatter {
		ProcessFormatter {
			name: "temp-file-test",
			command: "sh",
			args: &[
				"-c",
				"printf 'formatted' > \"$1\"",
				"temp-file-test",
				"{file}",
			],
			mode: ProcessMode::TempFile,
			write_editorconfig: true,
		}
	}

	#[cfg(windows)]
	fn temp_file_formatter() -> ProcessFormatter {
		ProcessFormatter {
			name: "temp-file-test",
			command: "powershell.exe",
			args: &[
				"-NoProfile",
				"-Command",
				"[System.IO.File]::WriteAllText($args[0], 'formatted')",
				"{file}",
			],
			mode: ProcessMode::TempFile,
			write_editorconfig: true,
		}
	}

	#[test]
	fn test_format_with_process_stdin_stdout() {
		let result =
			format_with_process("hello", "test.txt", stdin_formatter())
				.unwrap();
		assert_eq!(result, "HELLO");
	}

	#[test]
	fn test_format_with_process_temp_file() {
		let result = format_with_process(
			"hello",
			"build.gradle.kts",
			temp_file_formatter(),
		)
		.unwrap();
		assert_eq!(result, "formatted");
	}

	#[test]
	fn test_format_with_process_command_not_found() {
		let result = format_with_process(
			"hello",
			"test.txt",
			ProcessFormatter {
				name: "missing",
				command: "definitely-not-a-real-command",
				args: &[],
				mode: ProcessMode::StdinStdout,
				write_editorconfig: false,
			},
		);

		let error = result.unwrap_err();
		assert!(error.contains("CLI was not found in PATH"));
	}

	#[test]
	fn test_temp_file_path_preserves_filename() {
		let path = temp_file_path(Path::new("/tmp"), "nested/build.gradle.kts");
		assert_eq!(path.file_name(), Some(OsStr::new("build.gradle.kts")));
	}
}
