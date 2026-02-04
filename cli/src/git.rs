//! Git integration for filtering files by git status

use std::path::PathBuf;
use std::process::Command;

use crate::discovery;

/// Get the git repository root directory
fn get_git_root() -> anyhow::Result<PathBuf> {
	let output = Command::new("git")
		.args(["rev-parse", "--show-toplevel"])
		.output()
		.map_err(|e| anyhow::anyhow!("Failed to run git command: {}", e))?;

	if !output.status.success() {
		return Err(anyhow::anyhow!("Not a git repository"));
	}

	let root = String::from_utf8_lossy(&output.stdout);
	Ok(PathBuf::from(root.trim()))
}

/// Get files from git based on staged or changed status
/// Returns paths relative to current directory (same format as discovery)
pub fn get_git_files(staged: bool) -> anyhow::Result<Vec<PathBuf>> {
	// Get git repository root and current directory
	let git_root = get_git_root()?;
	let current_dir = std::env::current_dir().map_err(|e| {
		anyhow::anyhow!("Failed to get current directory: {}", e)
	})?;

	// Build git command arguments
	let mut args = vec!["diff", "--name-only", "--diff-filter=ACM"];
	if staged {
		args.push("--cached");
	}

	let output = Command::new("git")
		.args(&args)
		.output()
		.map_err(|e| anyhow::anyhow!("Failed to run git diff: {}", e))?;

	if !output.status.success() {
		let stderr = String::from_utf8_lossy(&output.stderr);
		return Err(anyhow::anyhow!("git diff failed: {}", stderr));
	}

	let stdout = String::from_utf8_lossy(&output.stdout);

	// Paths from git diff are relative to repo root
	// Convert to relative paths from current directory for display consistency
	let files: Vec<PathBuf> = stdout
		.lines()
		.filter(|line| !line.is_empty())
		.map(|line| {
			// First get absolute path by joining with git root
			let absolute = git_root.join(line);
			// Then make it relative to current directory
			pathdiff::diff_paths(&absolute, &current_dir).unwrap_or(absolute)
		})
		.filter(|path| discovery::is_supported_file(path))
		.collect();

	Ok(files)
}
