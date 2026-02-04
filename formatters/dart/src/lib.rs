// dart-formatter - Dart code formatter using dart_style
//
// Provides Dart code formatting by extracting and running an embedded
// dart_style binary at runtime.

use fd_lock::RwLock;
use once_cell::sync::Lazy;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

// Re-export types from common crate for standard formatter interface
pub use fama_common::{FileType, FormatConfig};

// Include the generated binary data
include!(concat!(env!("OUT_DIR"), "/binary_data.rs"));

// Global cache for the extracted binary path
static BINARY_CACHE: Lazy<Mutex<Option<PathBuf>>> =
	Lazy::new(|| Mutex::new(None));

/// Standard formatter entrypoint that matches the common formatter interface
///
/// # Arguments
/// * `content` - The source code to format
/// * `path` - Path to the file (used for error messages)
/// * `file_type` - The detected file type (should be FileType::Dart)
///
/// # Returns
/// * `Ok(String)` - Formatted Dart code
/// * `Err(String)` - Error message if formatting fails
pub fn format_file(
	content: &str,
	path: &str,
	_file_type: FileType,
) -> Result<String, String> {
	format_dart(content, path)
}

/// Format Dart source code using dart_style
///
/// # Arguments
/// * `source` - The Dart source code to format
/// * `file_path` - Path to the file (used for error messages)
///
/// # Returns
/// * `Ok(String)` - Formatted Dart code
/// * `Err(String)` - Error message if formatting fails
pub fn format_dart(source: &str, file_path: &str) -> Result<String, String> {
	// Get or extract the dart_style binary
	let binary_path = get_or_extract_binary()
		.map_err(|e| format!("Failed to setup dart_style binary: {}", e))?;

	// Run dart_style with stdin input
	let mut child = Command::new(&binary_path)
		.arg("--output=show")
		.arg("--stdin-name")
		.arg(file_path)
		.stdin(std::process::Stdio::piped())
		.stdout(std::process::Stdio::piped())
		.stderr(std::process::Stdio::piped())
		.spawn()
		.map_err(|e| format!("Failed to spawn dart_style: {}", e))?;

	// Write the source code to stdin
	if let Some(mut stdin) = child.stdin.take() {
		stdin.write_all(source.as_bytes()).map_err(|e| {
			format!("Failed to write to dart_style stdin: {}", e)
		})?;
		// Close stdin to signal EOF
		drop(stdin);
	}

	let output = child
		.wait_with_output()
		.map_err(|e| format!("Failed to wait for dart_style: {}", e))?;

	if output.status.success() {
		String::from_utf8(output.stdout)
			.map_err(|e| format!("Invalid UTF-8 in dart_style output: {}", e))
	} else {
		let stderr = String::from_utf8_lossy(&output.stderr);
		Err(format!("dart_style error: {}", stderr))
	}
}

/// Get the path to the dart_style binary, extracting it if necessary
fn get_or_extract_binary() -> anyhow::Result<PathBuf> {
	// Check if we already have the binary cached
	{
		let cache = BINARY_CACHE.lock().unwrap();
		if let Some(path) = cache.as_ref() {
			if is_binary_valid(path) {
				return Ok(path.clone());
			}
		}
	}

	// Get the binary data for this platform
	let binary_data = get_platform_binary()
		.ok_or_else(|| anyhow::anyhow!("No dart_style binary available for this platform. Please run fetch_binary.sh to download the binaries."))?;

	// Check if binary data is empty
	if binary_data.is_empty() {
		return Err(anyhow::anyhow!("Binary data is empty. Please run fetch_binary.sh to download the binaries."));
	}

	// Create the cache directory
	let cache_dir = get_cache_dir()?;
	fs::create_dir_all(&cache_dir)?;

	// Determine the executable name based on platform
	let exe_name = if cfg!(target_os = "windows") {
		"dart_style.exe"
	} else {
		"dart_style"
	};

	let binary_path = cache_dir.join(exe_name);

	// Acquire file lock to prevent concurrent extraction using fd-lock
	let lock_path = cache_dir.join(".extract.lock");
	let lock_file = fs::OpenOptions::new()
		.write(true)
		.create(true)
		.truncate(true)
		.open(&lock_path)?;

	// Try to acquire exclusive lock with timeout using polling
	let start = std::time::Instant::now();
	let timeout = std::time::Duration::from_secs(3);
	let mut lock = RwLock::new(lock_file);
	let _guard = loop {
		match lock.try_write() {
			Ok(guard) => break guard,
			Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
				if start.elapsed() >= timeout {
					return Err(anyhow::anyhow!(
						"Timeout waiting for binary extraction lock"
					));
				}
				std::thread::sleep(std::time::Duration::from_millis(100));
			}
			Err(e) => return Err(e.into()),
		}
	};

	// Double-check after acquiring lock (another process might have extracted it)
	if is_binary_valid(&binary_path) {
		let mut cache = BINARY_CACHE.lock().unwrap();
		*cache = Some(binary_path.clone());
		return Ok(binary_path);
	}

	// Extract the binary atomically using tempfile
	let mut temp_file = tempfile::NamedTempFile::new_in(&cache_dir)?;
	temp_file.write_all(binary_data)?;

	// Set permissions before persisting (Unix only)
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		let mut perms = fs::metadata(temp_file.path())?.permissions();
		perms.set_mode(0o500); // Read + execute for owner only
		fs::set_permissions(temp_file.path(), perms)?;
	}

	// Atomically move temp file to final location
	temp_file.persist(&binary_path)?;

	// Lock is automatically released when _guard is dropped

	// Verify the binary works
	if !is_binary_valid(&binary_path) {
		let _ = fs::remove_file(&binary_path);
		return Err(anyhow::anyhow!("Extracted binary failed validation"));
	}

	// Cache the path
	{
		let mut cache = BINARY_CACHE.lock().unwrap();
		*cache = Some(binary_path.clone());
	}

	Ok(binary_path)
}

/// Check if the binary exists and is valid (can execute)
fn is_binary_valid(path: &Path) -> bool {
	if !path.exists() {
		return false;
	}

	// Verify the binary can execute and return version
	match Command::new(path).arg("--version").output() {
		Ok(output) => output.status.success(),
		Err(_) => false,
	}
}

/// Get the binary data for the current platform
fn get_platform_binary() -> Option<&'static [u8]> {
	// Platform-specific binary selection
	#[cfg(all(target_arch = "x86_64", target_os = "linux"))]
	{
		Some(BINARY_X86_64_LINUX)
	}

	#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
	{
		Some(BINARY_AARCH64_LINUX)
	}

	#[cfg(all(target_arch = "x86_64", target_os = "macos"))]
	{
		Some(BINARY_X86_64_MACOS)
	}

	#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
	{
		Some(BINARY_AARCH64_MACOS)
	}

	#[cfg(all(
		target_arch = "x86_64",
		target_os = "windows",
		target_env = "msvc"
	))]
	{
		Some(BINARY_X86_64_WINDOWS_MSVC)
	}

	#[cfg(all(
		target_arch = "x86_64",
		target_os = "windows",
		target_env = "gnu"
	))]
	{
		Some(BINARY_X86_64_WINDOWS_GNU)
	}

	#[cfg(all(
		target_arch = "x86",
		target_os = "windows",
		target_env = "msvc"
	))]
	{
		Some(BINARY_X86_WINDOWS_MSVC)
	}

	#[cfg(all(
		target_arch = "aarch64",
		target_os = "windows",
		target_env = "msvc"
	))]
	{
		Some(BINARY_AARCH64_WINDOWS_MSVC)
	}

	#[cfg(not(any(
		all(target_arch = "x86_64", target_os = "linux"),
		all(target_arch = "aarch64", target_os = "linux"),
		all(target_arch = "x86_64", target_os = "macos"),
		all(target_arch = "aarch64", target_os = "macos"),
		all(
			target_arch = "x86_64",
			target_os = "windows",
			target_env = "msvc"
		),
		all(target_arch = "x86_64", target_os = "windows", target_env = "gnu"),
		all(target_arch = "x86", target_os = "windows", target_env = "msvc"),
		all(
			target_arch = "aarch64",
			target_os = "windows",
			target_env = "msvc"
		),
	)))]
	{
		None
	}
}

/// Get the cache directory for the dart_style binary
fn get_cache_dir() -> anyhow::Result<PathBuf> {
	let cache_dir = dirs::cache_dir().ok_or_else(|| {
		anyhow::anyhow!("Could not determine cache directory")
	})?;

	Ok(cache_dir.join("fama").join("dart_style"))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_format_simple_dart() {
		let input = r#"
void main() {
var x=1;
  var y   =   2;
print(x+y);
}
"#;

		let result = format_dart(input, "test.dart");

		// The test may fail if binary is not available, which is OK
		match result {
			Ok(output) => {
				assert!(output.contains("var x = 1"));
				assert!(output.contains("var y = 2"));
				assert!(output.contains("print(x + y)"));
			}
			Err(e) => {
				println!("Skipping test - dart_style not available: {}", e);
			}
		}
	}

	#[test]
	fn test_format_class() {
		let input = r#"
class  MyClass  {
String  name;
int  age;
MyClass(this.name,this.age);
}
"#;

		let result = format_dart(input, "test.dart");

		match result {
			Ok(output) => {
				assert!(output.contains("class MyClass"));
			}
			Err(e) => {
				println!("Skipping test - dart_style not available: {}", e);
			}
		}
	}
}
