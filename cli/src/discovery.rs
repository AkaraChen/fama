use ignore::gitignore::Gitignore;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

const SUPPORTED_EXTENSIONS: &[&str] = &[
	"js", "jsx", "ts", "tsx", "mjs", "mjsx", "mts", "json", "jsonc", "css",
	"scss", "less", "html", "vue", "svelte", "astro", "yaml", "yml", "md",
	"rs", "py", "lua", "sh", "bash", "zsh", "go", "toml", "graphql", "gql",
	"sql", "xml",
];

/// Check if a file has a supported extension
fn has_supported_extension(path: &Path) -> bool {
	path.extension()
		.and_then(|ext| ext.to_str())
		.is_some_and(|ext| SUPPORTED_EXTENSIONS.contains(&ext))
}

/// Walk a directory respecting .gitignore rules
fn walk_directory(base: &Path) -> Result<Vec<PathBuf>, String> {
	let mut walk_builder = WalkBuilder::new(base);
	// Add .gitignore support
	let _ = walk_builder.add_ignore(".");

	let mut files: Vec<PathBuf> = walk_builder
		.build()
		.filter_map(|entry| entry.ok())
		.filter(|entry| entry.file_type().is_some_and(|ft| ft.is_file()))
		.filter(|entry| has_supported_extension(entry.path()))
		.map(|entry| entry.path().to_path_buf())
		.collect();

	files.sort();
	Ok(files)
}

/// Collect files from a glob pattern
fn collect_from_glob(pattern: &str) -> Result<Vec<PathBuf>, String> {
	let current_dir = std::env::current_dir()
		.map_err(|e| format!("Failed to get current directory: {}", e))?;

	// Build gitignore matcher for filtering results
	let gitignore = Gitignore::new(current_dir.join(".gitignore")).0;

	// Convert to absolute path if relative
	let glob_pattern = if Path::new(pattern).is_absolute() {
		pattern.to_string()
	} else {
		current_dir.join(pattern).to_string_lossy().to_string()
	};

	let mut files: Vec<PathBuf> = glob::glob(&glob_pattern)
		.map_err(|e| format!("Invalid glob pattern '{}': {}", pattern, e))?
		.filter_map(Result::ok)
		.filter(|path| path.is_file())
		.filter(|path| has_supported_extension(path))
		.filter(|path| {
			// Check if file is ignored
			!matches!(gitignore.matched(path, false), ignore::Match::Ignore(_))
		})
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
			// Single file - check extension and return
			if has_supported_extension(&path) {
				return Ok(vec![path]);
			} else {
				return Ok(vec![]);
			}
		} else if path.is_dir() {
			// Directory path - walk from there
			return walk_directory(&path);
		}
		// Path doesn't exist, fall through to glob attempt
	}

	// It's a glob pattern or a non-existent path - use glob
	collect_from_glob(pattern)
}
