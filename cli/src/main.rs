mod discovery;
mod editorconfig;
mod formatter;

extern crate biome;
extern crate dockerfile;
extern crate dprint;
extern crate ruff;
extern crate rustfmt;
extern crate shfmt;
extern crate stylua;

use clap::Parser;

#[derive(Parser)]
#[command(name = "fama")]
#[command(about = "A code formatter for frontend projects", long_about = None)]
struct Cli {
	/// Glob pattern to match files
	#[arg(default_value = "**/*")]
	pattern: String,

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

fn run(pattern: &str) -> anyhow::Result<()> {
	let files = discovery::discover_files(Some(pattern))
		.map_err(|e| anyhow::anyhow!("Failed to discover files: {}", e))?;

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
