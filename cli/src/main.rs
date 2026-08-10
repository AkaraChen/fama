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
use fama_common::FileType;
use formatter::FileOutcome;
use rayon::prelude::*;
use rustc_hash::FxHashSet;
use std::path::PathBuf;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

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

	fn record_outcome(&mut self, path: &std::path::Path, outcome: FileOutcome, debug: bool) {
		match outcome {
			FileOutcome::Formatted => {
				if debug {
					eprintln!(
						"{}",
						Color::Green.paint(&path.display().to_string())
					);
				}
				self.formatted += 1;
			}
			FileOutcome::Unchanged => {
				if debug {
					eprintln!("{}", path.display());
				}
				self.unchanged += 1;
			}
			FileOutcome::Error(e) => {
				if debug {
					eprintln!(
						"{}",
						Color::Red.paint(&path.display().to_string())
					);
				}
				self.errors.push(e);
			}
		}
	}
}

fn run(options: Cli) -> anyhow::Result<()> {
	let patterns = options.pattern;
	let debug = options.debug;
	let check = options.check;
	let quiet = options.quiet;
	let mut all_files: Vec<PathBuf> = Vec::new();

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

	// Remove duplicates while preserving order (FxHash; no clone on duplicates)
	let mut seen =
		FxHashSet::with_capacity_and_hasher(all_files.len(), Default::default());
	let mut files = Vec::with_capacity(all_files.len());
	for p in all_files {
		// contains via Borrow<Path> — skip clone when already seen
		if seen.contains(p.as_path()) {
			continue;
		}
		seen.insert(p.clone());
		files.push(p);
	}

	// Partition rustfmt multi-path batch from the rest. Shell/Go/Hcl go through
	// per-file rayon (goffi *_batch is sequential inside Go and steals pool
	// occupancy from C-family WASM workers).
	let mut rust_files = Vec::new();
	let mut other_files = Vec::new();

	for f in files {
		match formatter::file_type_of(&f) {
			FileType::Rust => rust_files.push(f),
			_ => other_files.push(f),
		}
	}

	// Run rustfmt batch lane alongside per-file par_iter for everything else
	// (including Shell/Go/Hcl and C-family clang WASM).
	let (other_stats, rust_stats) = rayon::join(
		|| format_others(&other_files, check, debug),
		|| format_batch_rust(&rust_files, check, debug),
	);

	let stats = other_stats.merge(rust_stats);

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
			println!(
				"Formatted {} files, {} unchanged, {} errors",
				stats.formatted,
				stats.unchanged,
				stats.errors.len()
			);
		}
	}

	// Exit non-zero when check finds dirty files, or when any path errored
	// (e.g. rustfmt syntax failure) — avoid false-green exit 0 with only errors.
	if (check && stats.formatted > 0) || !stats.errors.is_empty() {
		std::process::exit(1);
	}

	Ok(())
}

fn format_others(files: &[PathBuf], check: bool, debug: bool) -> FormatStats {
	files
		.par_iter()
		.fold(FormatStats::default, |mut stats, file| {
			match formatter::format_file(file, check) {
				Ok(true) => {
					stats.record_outcome(file, FileOutcome::Formatted, debug);
				}
				Ok(false) => {
					stats.record_outcome(file, FileOutcome::Unchanged, debug);
				}
				Err(e) => {
					stats.record_outcome(
						file,
						FileOutcome::Error(e.to_string()),
						debug,
					);
				}
			}
			stats
		})
		.reduce(FormatStats::default, FormatStats::merge)
}

fn format_batch_rust(files: &[PathBuf], check: bool, debug: bool) -> FormatStats {
	if files.is_empty() {
		return FormatStats::default();
	}
	let mut stats = FormatStats::default();
	for (path, outcome) in formatter::format_rust_batch(files, check) {
		stats.record_outcome(&path, outcome, debug);
	}
	stats
}
