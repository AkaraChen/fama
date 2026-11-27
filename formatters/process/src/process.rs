//! Shared helpers for external CLI-backed (`process mode`) formatters.

use std::env;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

/// Resolve the CLI binary path from `env_var`, falling back to `default_bin`.
pub fn resolve_binary(env_var: &str, default_bin: &str) -> String {
	env::var(env_var)
		.ok()
		.filter(|path| !path.is_empty())
		.unwrap_or_else(|| default_bin.to_string())
}

/// Format `source` by piping it into `bin` with `args` on stdin and reading stdout.
pub fn run_stdin(
	bin: &str,
	args: &[&str],
	source: &str,
	tool_name: &str,
	install_hint: &str,
) -> Result<String, String> {
	run_stdin_with_dir(bin, args, None, source, tool_name, install_hint)
}

/// Format `source` via stdin/stdout, optionally setting the working directory.
pub fn run_stdin_with_dir(
	bin: &str,
	args: &[&str],
	cwd: Option<&Path>,
	source: &str,
	tool_name: &str,
	install_hint: &str,
) -> Result<String, String> {
	let mut cmd = Command::new(bin);
	cmd.args(args)
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped());
	if let Some(dir) = cwd {
		cmd.current_dir(dir);
	}

	let mut child = cmd
		.spawn()
		.map_err(|e| command_error(bin, e, tool_name, install_hint))?;

	child
		.stdin
		.take()
		.ok_or_else(|| format!("{tool_name} stdin was not available"))?
		.write_all(source.as_bytes())
		.map_err(|e| format!("Failed to write to {tool_name} stdin: {e}"))?;

	let output = child
		.wait_with_output()
		.map_err(|e| format!("Failed to wait for {tool_name}: {e}"))?;

	success_status(&output, tool_name)?;
	String::from_utf8(output.stdout)
		.map_err(|e| format!("{tool_name} output was not valid UTF-8: {e}"))
}

/// Check that a process exited with zero status, returning stderr/stdout detail otherwise.
pub fn success_status(output: &Output, tool_name: &str) -> Result<(), String> {
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
		Err(format!("{tool_name} failed with status {status}"))
	} else {
		Err(format!("{tool_name} failed with status {status}: {detail}"))
	}
}

/// Format a consistent spawn/not-found error for an external CLI binary.
pub fn command_error(
	bin: &str,
	error: std::io::Error,
	tool_name: &str,
	install_hint: &str,
) -> String {
	if error.kind() == std::io::ErrorKind::NotFound {
		return format!(
			"{tool_name} CLI was not found in PATH. {install_hint}"
		);
	}
	format!("Failed to start {tool_name} (`{bin}`): {error}")
}
