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
use color::Color;
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
}

fn main() -> anyhow::Result<()> {
	let cli = Cli::parse();

	if cli.export {
		editorconfig::export();
		return Ok(());
	}

	run(cli)
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

fn run(options: Cli) -> anyhow::Result<()> {
	let patterns = options.pattern;
	let debug = options.debug;
	let check = options.check;
	let quiet = options.quiet;
	let mut all_files: Vec<std::path::PathBuf> = Vec::new();

	// Get files from git if --staged or --changed is specified
	if options.staged || options.changed {
		let git_files = git::get_git_files(options.staged)?;
		if git_files.is_empty() {
			if !quiet {
				println!("No files to format");
			}
			return Ok(());
		}
		all_files.extend(git_files);
	} else {
		for pattern in &patterns {
			let files =
				discovery::discover_files(Some(pattern)).map_err(|e| {
					anyhow::anyhow!("Failed to discover files: {}", e)
				})?;
			if files.is_empty() && !quiet {
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
			match formatter::format_file(file, check) {
				Ok(true) => {
					if debug {
						// Green for formatted files
						eprintln!(
							"{}",
							Color::Green.paint(&file.display().to_string())
						);
					}
					stats.formatted += 1;
				}
				Ok(false) => {
					if debug {
						eprintln!("{}", file.display());
					}
					stats.unchanged += 1;
				}
				Err(e) => {
					if debug {
						eprintln!(
							"{}",
							Color::Red.paint(&file.display().to_string())
						);
					}
					stats.errors.push(e.to_string());
				}
			}
			stats
		})
		.reduce(FormatStats::default, FormatStats::merge);

	// Print collected errors (always print errors)
	for error in &stats.errors {
		eprintln!("Error: {}", error);
	}

	// Print stats (unless quiet mode)
	if !quiet {
		if check {
			if stats.formatted > 0 {
				println!(
					"{} files need formatting, {} unchanged, {} errors",
					stats.formatted,
					stats.unchanged,
					stats.errors.len()
				);
			} else {
				println!(
					"All {} files are properly formatted ({} errors)",
					stats.unchanged,
					stats.errors.len()
				);
			}
		} else {
			println!(
				"Formatted {} files, {} unchanged, {} errors",
				stats.formatted,
				stats.unchanged,
				stats.errors.len()
			);
		}
	}

	// Exit with non-zero if check mode and files need formatting
	if check && stats.formatted > 0 {
		std::process::exit(1);
	}

	Ok(())
}
