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
}

fn main() -> anyhow::Result<()> {
	let cli = Cli::parse();

	if cli.export {
		editorconfig::export();
		return Ok(());
	}

	run(&cli.pattern)
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

fn run(patterns: &[String]) -> anyhow::Result<()> {
	let mut all_files: Vec<std::path::PathBuf> = Vec::new();

	for pattern in patterns {
		let files = discovery::discover_files(Some(pattern))
			.map_err(|e| anyhow::anyhow!("Failed to discover files: {}", e))?;
		if files.is_empty() {
			eprintln!("Warning: pattern '{}' matched 0 files", pattern);
		}
		all_files.extend(files);
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
