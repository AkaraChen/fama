use fama_common::{detect_file_type, FileType};
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

const SUPPORTED_EXTENSIONS: &[&str] = &[
	"js", "jsx", "ts", "tsx", "mjs", "mjsx", "mts", "json", "jsonc", "css",
	"scss", "less", "html", "vue", "svelte", "astro", "yaml", "yml", "md",
	"rs", "py", "lua", "rb", "rake", "gemspec", "ru", "sh", "bash", "zsh",
	"go", "hcl", "tf", "tfvars", "toml", "graphql", "gql", "sql", "xml",
];

/// Check if a file is supported for formatting
fn is_supported_path(path: &Path) -> bool {
	// First check by extension (fast path)
	if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
		if SUPPORTED_EXTENSIONS.contains(&ext) {
			return true;
		}
	}
	// For files without supported extension, check if detect_file_type recognizes them
	// This handles special filenames like Dockerfile, Rakefile, Gemfile, etc.
	let path_str = path.to_str().unwrap_or("");
	!matches!(detect_file_type(path_str), FileType::Unknown)
}

/// Check if a file is supported (has supported extension/filename and is a file)
pub fn is_supported_file(path: &Path) -> bool {
	path.is_file() && is_supported_path(path)
}

/// Walk a directory respecting .gitignore rules, optionally filtering by glob pattern
fn walk_with_pattern(
	base: &Path,
	pattern: Option<&glob::Pattern>,
) -> Result<Vec<PathBuf>, String> {
	let mut files: Vec<PathBuf> = WalkBuilder::new(base)
		.hidden(false)
		.build()
		.filter_map(|entry| entry.ok())
		.filter(|entry| entry.file_type().is_some_and(|ft| ft.is_file()))
		.filter(|entry| is_supported_path(entry.path()))
		.filter(|entry| {
			pattern
				.map(|p| p.matches_path(entry.path()))
				.unwrap_or(true)
		})
		.map(|entry| entry.path().to_path_buf())
		.collect();

	files.sort();
	Ok(files)
}

/// Discover files matching the given pattern while respecting .gitignore rules.
///
/// # Arguments
/// * `pattern` - Optional glob pattern. If None, defaults to "**/*"
///
/// Pattern types supported:
/// - Single file: "src/main.rs" → returns that file if extension is supported
/// - Directory: "src/" → walks that directory
/// - Glob pattern: "src/*.rs" or "**/*.js" → expands and filters
///
/// # Returns
/// A sorted list of file paths matching the pattern and supported extensions
pub fn discover_files(pattern: Option<&str>) -> Result<Vec<PathBuf>, String> {
	let pattern = pattern.unwrap_or("**/*");

	// Check if pattern is a literal file path (no glob characters)
	if !pattern.contains(['*', '?', '[']) {
		let path = PathBuf::from(pattern);

		if path.is_file() {
			// Single file - check if supported and return
			if is_supported_path(&path) {
				return Ok(vec![path]);
			} else {
				let ext = path
					.extension()
					.and_then(|e| e.to_str())
					.unwrap_or("(none)");
				return Err(format!(
					"Unsupported file extension '{}': {}",
					ext,
					path.display()
				));
			}
		} else if path.is_dir() {
			// Directory path - walk from there
			return walk_with_pattern(&path, None);
		}
		// Path doesn't exist, fall through to glob attempt
	}

	// It's a glob pattern - walk current directory and filter by pattern
	let glob_pattern = glob::Pattern::new(pattern)
		.map_err(|e| format!("Invalid glob pattern '{}': {}", pattern, e))?;
	walk_with_pattern(Path::new("."), Some(&glob_pattern))
}
