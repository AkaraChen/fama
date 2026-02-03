//! Git integration for filtering files by git status

use std::path::PathBuf;
use std::process::Command;

use crate::discovery;

/// Get files from git based on staged or changed status
pub fn get_git_files(staged: bool) -> anyhow::Result<Vec<PathBuf>> {
	// Check if we're in a git repository
	let git_check = Command::new("git")
		.args(["rev-parse", "--git-dir"])
		.output()
		.map_err(|e| anyhow::anyhow!("Failed to run git command: {}", e))?;

	if !git_check.status.success() {
		return Err(anyhow::anyhow!("Not a git repository"));
	}

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
	let current_dir = std::env::current_dir()
		.map_err(|e| anyhow::anyhow!("Failed to get current directory: {}", e))?;

	let files: Vec<PathBuf> = stdout
		.lines()
		.filter(|line| !line.is_empty())
		.map(|line| current_dir.join(line))
		.filter(|path| discovery::is_supported_file(path))
		.collect();

	Ok(files)
}
