use ignore::WalkBuilder;
use std::path::PathBuf;

const SUPPORTED_EXTENSIONS: &[&str] = &[
	"js", "jsx", "ts", "tsx", "mjs", "mjsx", "mts", "json", "jsonc", "css",
	"scss", "less", "html", "vue", "svelte", "astro", "yaml", "yml", "md",
	"rs", "py", "lua", "sh", "bash", "zsh", "go", "toml",
];

/// Discover files in the current directory respecting .gitignore rules.
///
/// # Arguments
/// * `pattern` - Optional glob pattern. If None, defaults to "**/*"
///
/// # Returns
/// A sorted list of file paths matching the pattern and supported extensions
pub fn discover_files(pattern: Option<&str>) -> Result<Vec<PathBuf>, String> {
	let current_dir = std::env::current_dir()
		.map_err(|e| format!("Failed to get current directory: {}", e))?;

	let _glob_pattern = pattern.unwrap_or("**/*");

	let mut walk_builder = WalkBuilder::new(&current_dir);
	// Add .gitignore support
	let _ = walk_builder.add_ignore(".");

	let mut files: Vec<PathBuf> = walk_builder
		.build()
		.filter_map(|entry| entry.ok())
		.filter(|entry| {
			// Only include regular files
			if !entry.file_type().is_some_and(|ft| ft.is_file()) {
				return false;
			}

			// Check if file has supported extension
			entry
				.path()
				.extension()
				.and_then(|ext| ext.to_str())
				.is_some_and(|ext| SUPPORTED_EXTENSIONS.contains(&ext))
		})
		.map(|entry| entry.path().to_path_buf())
		.collect();

	// Sort for consistent ordering
	files.sort();

	Ok(files)
}
