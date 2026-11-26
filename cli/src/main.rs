mod color;
mod discovery;
mod editorconfig;
mod formatter;
mod git;

extern crate biome;
extern crate dockerfile;
extern crate dprint;
extern crate goffi;
extern crate ruff;
extern crate rustfmt;
extern crate stylua;

use clap::Parser;
use std::path::Path;

#[derive(Parser)]
#[command(name = "fama")]
#[command(about = "A code formatter for many languages", long_about = None)]
struct Cli {
	/// Glob patterns to match files
	#[arg(default_values_t = ["**/*".to_string()])]
	pattern: Vec<String>,

	/// Export EditorConfig to stdout
	#[arg(long, short)]
	export: bool,

	/// Print each file being formatted to stderr
	#[arg(long, short)]
	debug: bool,

	/// Check if files are formatted, exit with non-zero if not
	#[arg(long, short)]
	check: bool,

	/// Quiet mode, only output errors
	#[arg(long, short)]
	quiet: bool,

	/// Only format git staged files
	#[arg(long, group = "git_filter")]
	staged: bool,

	/// Only format git changed (uncommitted) files
	#[arg(long, group = "git_filter")]
	changed: bool,

	/// Format changed files and commit with message "style: fmt"
	#[arg(long)]
	commit: bool,
}

fn main() -> anyhow::Result<()> {
	let cli = Cli::parse();

	if cli.export {
		editorconfig::export();
		return Ok(());
	}

	run(cli)
}

/// Warn about glob-like patterns that matched no files.
/// Literal directories and existing files never warn (they either contribute
/// files or fail discovery with an error).
fn warn_empty_patterns(files: &[std::path::PathBuf], patterns: &[String]) {
	for pattern in patterns {
		if pattern == "**/*" && !files.is_empty() {
			continue;
		}
		let is_globish = pattern.contains(['*', '?', '[']);
		let missing_literal = !is_globish
			&& !Path::new(pattern).is_file()
			&& !Path::new(pattern).is_dir();
		if !(is_globish || missing_literal) {
			continue;
		}
		let matched = glob::Pattern::new(pattern)
			.is_ok_and(|glob| files.iter().any(|file| glob.matches_path(file)));
		if !matched {
			eprintln!("Warning: pattern '{}' matched 0 files", pattern);
		}
	}
}

fn run(options: Cli) -> anyhow::Result<()> {
	let patterns = options.pattern;
	let debug = options.debug;
	let check = options.check;
	let quiet = options.quiet;

	// Get files from git if --staged, --changed, or --commit is specified
	let files: Vec<std::path::PathBuf> = if options.staged
		|| options.changed
		|| options.commit
	{
		let git_files = git::get_git_files(options.staged)?;
		if git_files.is_empty() {
			if !quiet {
				println!("No files to format");
			}
			return Ok(());
		}
		let mut seen = std::collections::HashSet::with_capacity(git_files.len());
		git_files
			.into_iter()
			.filter(|p| seen.insert(p.clone()))
			.collect()
	} else {
		let pattern_refs: Vec<&str> =
			patterns.iter().map(String::as_str).collect();
		let files = discovery::discover_files_multi(&pattern_refs)
			.map_err(|e| anyhow::anyhow!("Failed to discover files: {}", e))?;
		if !quiet {
			warn_empty_patterns(&files, &patterns);
		}
		files
	};

	// Parallel formatting; Rust files batched, everything else per file
	let mut stats = formatter::format_files(&files, check, debug);

	// If --staged was used, automatically re-stage formatted files
	let restaged_count = if options.staged && !stats.formatted_files.is_empty()
	{
		match git::stage_files(&stats.formatted_files) {
			Ok(count) => count,
			Err(e) => {
				stats
					.errors
					.push(format!("Failed to re-stage files: {}", e));
				0
			}
		}
	} else {
		0
	};

	// If --commit was used, stage formatted files and commit
	if options.commit && !stats.formatted_files.is_empty() {
		match git::stage_files(&stats.formatted_files) {
			Ok(count) if count > 0 => match git::commit_files("style: fmt") {
				Ok(_) => {
					if !quiet {
						println!(
							"Committed {} files with message 'style: fmt'",
							count
						);
					}
				}
				Err(e) => {
					stats.errors.push(format!("Failed to commit: {}", e));
				}
			},
			Ok(_) => {}
			Err(e) => {
				stats
					.errors
					.push(format!("Failed to stage files for commit: {}", e));
			}
		}
	}

	// Print collected errors (always print errors)
	for error in &stats.errors {
		eprintln!("Error: {}", error);
	}

	// Print stats (unless quiet mode)
	if !quiet {
		if check {
			println!(
				"{} files need formatting, {} unchanged, {} errors",
				stats.formatted,
				stats.unchanged,
				stats.errors.len()
			);
		} else {
			let mut message = format!(
				"Formatted {} files, {} unchanged, {} errors",
				stats.formatted,
				stats.unchanged,
				stats.errors.len()
			);
			if restaged_count > 0 {
				message.push_str(&format!(", restaged {}", restaged_count));
			}
			println!("{}", message);
		}
	}

	// Exit with non-zero if check mode and files need formatting
	if check && stats.formatted > 0 {
		std::process::exit(1);
	}

	Ok(())
}
