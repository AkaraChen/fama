mod discovery;
mod editorconfig;
mod formatter;

extern crate biome;
extern crate dockerfile;
extern crate dprint;
extern crate goffi;
extern crate ruff;
extern crate rustfmt;
extern crate stylua;

use clap::Parser;
use rayon::prelude::*;

#[derive(Parser)]
#[command(name = "fama")]
#[command(about = "A code formatter for frontend projects", long_about = None)]
struct Cli {
	/// Glob patterns to match files
	#[arg(default_values_t = ["**/*".to_string()])]
	pattern: Vec<String>,

	/// Export EditorConfig to stdout
	#[arg(long, short)]
	export: bool,

	/// Only format git staged files
	#[arg(long, group = "git_filter")]
	staged: bool,

	/// Only format git changed (uncommitted) files
	#[arg(long, group = "git_filter")]
	changed: bool,
}

fn main() -> anyhow::Result<()> {
	let cli = Cli::parse();

	if cli.export {
		editorconfig::export();
		return Ok(());
	}

	// Get files from git if --staged or --changed is specified
	let files = if cli.staged || cli.changed {
		get_git_files(cli.staged)?
	} else {
		Vec::new()
	};

	if (cli.staged || cli.changed) && files.is_empty() {
		println!("No files to format");
		return Ok(());
	}

	run(
		&cli.pattern,
		if cli.staged || cli.changed {
			Some(&files)
		} else {
			None
		},
	)
}

/// Statistics collected during formatting
#[derive(Default)]
struct FormatStats {
	formatted: usize,
	unchanged: usize,
	errors: Vec<String>,
}

impl FormatStats {
	/// Merge two FormatStats instances (used in parallel reduce)
	fn merge(mut self, other: FormatStats) -> FormatStats {
		self.formatted += other.formatted;
		self.unchanged += other.unchanged;
		self.errors.extend(other.errors);
		self
	}
}

fn run(
	patterns: &[String],
	git_files: Option<&[std::path::PathBuf]>,
) -> anyhow::Result<()> {
	let mut all_files: Vec<std::path::PathBuf> = Vec::new();

	// If git_files is provided, use those directly
	if let Some(files) = git_files {
		all_files.extend(files.iter().cloned());
	} else {
		for pattern in patterns {
			let files =
				discovery::discover_files(Some(pattern)).map_err(|e| {
					anyhow::anyhow!("Failed to discover files: {}", e)
				})?;
			if files.is_empty() {
				eprintln!("Warning: pattern '{}' matched 0 files", pattern);
			}
			all_files.extend(files);
		}
	}

	// Remove duplicates while preserving order
	let mut seen = std::collections::HashSet::new();
	let files: Vec<_> = all_files
		.into_iter()
		.filter(|p| seen.insert(p.clone()))
		.collect();

	// Parallel formatting with fold/reduce pattern
	let stats = files
		.par_iter()
		.fold(FormatStats::default, |mut stats, file| {
			match formatter::format_file(file) {
				Ok(true) => stats.formatted += 1,
				Ok(false) => stats.unchanged += 1,
				Err(e) => stats.errors.push(e.to_string()),
			}
			stats
		})
		.reduce(FormatStats::default, FormatStats::merge);

	// Print collected errors
	for error in &stats.errors {
		eprintln!("Error: {}", error);
	}

	println!(
		"Formatted {} files, {} unchanged, {} errors",
		stats.formatted,
		stats.unchanged,
		stats.errors.len()
	);

	Ok(())
}

/// Get files from git based on staged or changed status
fn get_git_files(staged: bool) -> anyhow::Result<Vec<std::path::PathBuf>> {
	use std::process::Command;

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
	let current_dir = std::env::current_dir().map_err(|e| {
		anyhow::anyhow!("Failed to get current directory: {}", e)
	})?;

	let files: Vec<std::path::PathBuf> = stdout
		.lines()
		.filter(|line| !line.is_empty())
		.map(|line| current_dir.join(line))
		.filter(|path| discovery::is_supported_file(path))
		.collect();

	Ok(files)
}
