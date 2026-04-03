// git.rs - Git integration for filtering files by git status

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

/// Stage files with git add
/// Returns the number of files successfully staged
pub fn stage_files(files: &[std::path::PathBuf]) -> anyhow::Result<usize> {
	if files.is_empty() {
		return Ok(0);
	}

	// Convert paths to strings for git command
	let path_args: Vec<String> = files
		.iter()
		.filter_map(|p| p.to_str())
		.map(String::from)
		.collect();

	let output = Command::new("git")
		.arg("add")
		.args(&path_args)
		.output()
		.map_err(|e| anyhow::anyhow!("Failed to run git add: {}", e))?;

	if !output.status.success() {
		let stderr = String::from_utf8_lossy(&output.stderr);
		return Err(anyhow::anyhow!("git add failed: {}", stderr));
	}

	Ok(path_args.len())
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::fs;
	use tempfile::TempDir;

	fn git_available() -> bool {
		Command::new("git").arg("--version").output().is_ok()
	}

	fn init_git_repo(dir: &std::path::Path) {
		if !git_available() {
			return;
		}
		let _ = Command::new("git")
			.args(["init", "--quiet"])
			.current_dir(dir)
			.output();
		
		let _ = Command::new("git")
			.args(["config", "user.email", "test@test.com"])
			.current_dir(dir)
			.output();
		
		let _ = Command::new("git")
			.args(["config", "user.name", "Test"])
			.current_dir(dir)
			.output();
	}

	fn stage_file(dir: &std::path::Path, file: &str) {
		if !git_available() {
			return;
		}
		let _ = Command::new("git")
			.args(["add", file])
			.current_dir(dir)
			.output();
	}

	fn commit(dir: &std::path::Path, message: &str) {
		if !git_available() {
			return;
		}
		let _ = Command::new("git")
			.args(["commit", "-m", message, "--quiet"])
			.current_dir(dir)
			.output();
	}

	#[test]
	fn test_get_git_root_success() {
		if !git_available() {
			return;
		}
		let temp_dir = TempDir::new().unwrap();
		init_git_repo(temp_dir.path());

		let original_dir = std::env::current_dir().unwrap();
		let _ = std::env::set_current_dir(temp_dir.path());

		let result = get_git_root();

		let _ = std::env::set_current_dir(original_dir);

		assert!(result.is_ok());
	}

	#[test]
	fn test_get_git_root_not_git_repo() {
		if !git_available() {
			// Skip test if git is not available
			return;
		}
		let temp_dir = TempDir::new().unwrap();
		
		let original_dir = std::env::current_dir().unwrap();
		let _ = std::env::set_current_dir(temp_dir.path());

		let result = get_git_root();

		let _ = std::env::set_current_dir(original_dir);

		assert!(result.is_err());
	}

	#[test]
	fn test_stage_files_empty_list() {
		let result = stage_files(&[]);
		
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), 0);
	}
}