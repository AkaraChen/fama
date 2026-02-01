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

	let (mut formatted, mut unchanged, mut errors) = (0, 0, 0);

	for file in &files {
		match formatter::format_file(file) {
			Ok(true) => formatted += 1,
			Ok(false) => unchanged += 1,
			Err(e) => {
				eprintln!("Error: {}", e);
				errors += 1;
			}
		}
	}

	println!(
		"Formatted {} files, {} unchanged, {} errors",
		formatted, unchanged, errors
	);

	Ok(())
}
