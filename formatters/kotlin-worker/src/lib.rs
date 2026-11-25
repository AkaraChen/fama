//! Persistent-worker Kotlin formatting client.
//!
//! Spawns the [`ktfmt-worker`](https://github.com/AkaraChen/fama/tree/master/ktfmt-worker)
//! native image (or JVM jar) once per rayon thread and formats every Kotlin file
//! of a run through it over a length-prefixed framing protocol, instead of
//! paying ktfmt's process startup per file.
//!
//! # Wire protocol
//!
//! All integers are unsigned 32-bit big-endian.
//!
//! ```text
//! request   := len:u32 payload:u8[len]
//! payload   := kind:u8 source:u8[len-1]
//!              kind 0 = regular .kt source
//!              kind 1 = Kotlin script (.kts) source
//! response  := status:u8 len:u32 payload:u8[len]
//!              status 0 = ok,    payload := formatted source (UTF-8)
//!              status 1 = error, payload := error message (worker stays alive)
//!              status 2 = fatal, payload := error message (worker exits)
//! ```
//!
//! EOF on stdin shuts the worker down cleanly.
//!
//! # Worker discovery
//!
//! 1. `FAMA_KTFMT_WORKER` — explicit path to the worker executable.
//! 2. `ktfmt-worker` on `PATH`.
//!
//! When neither resolves, or the worker fails at the protocol level, formatting
//! falls back to the pre-existing process-based ktfmt path
//! ([`fama_process::format_kotlin`]) so behavior never regresses for users
//! without the worker.

use std::cell::{Cell, RefCell};
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::OnceLock;

use fama_common::{editorconfig_contents, FormatConfig};

/// Environment variable pointing at the ktfmt-worker executable.
pub const WORKER_ENV: &str = "FAMA_KTFMT_WORKER";

/// Program name looked up on `PATH` when [`WORKER_ENV`] is not set.
const WORKER_PROGRAM: &str = "ktfmt-worker";

/// Request payload kinds (see the crate docs and ktfmt-worker/README.md).
const KIND_REGULAR: u8 = 0;
const KIND_SCRIPT: u8 = 1;

/// Response statuses.
const STATUS_OK: u8 = 0;
const STATUS_FORMAT_ERROR: u8 = 1;
const STATUS_FATAL: u8 = 2;

/// Largest response frame accepted before the protocol is considered desynced.
const MAX_RESPONSE_BYTES: usize = 512 * 1024 * 1024;

/// After this many consecutive protocol-level worker failures on one thread,
/// stop respawning the worker and use the process fallback for the rest of the
/// run. Protects against a broken worker binary costing seconds per file.
const MAX_CONSECUTIVE_WORKER_FAILURES: u32 = 3;

thread_local! {
	/// One worker per rayon thread; files parallelize across workers without a
	/// mutex. Same reuse pattern as the clang formatter's WASM instance.
	static WORKER: RefCell<Option<KotlinWorker>> = const { RefCell::new(None) };

	/// Consecutive protocol-level failures on this thread.
	static WORKER_FAILURES: Cell<u32> = const { Cell::new(0) };
}

/// Format Kotlin source through the per-thread ktfmt-worker, falling back to
/// the process-based ktfmt path when the worker is unavailable or fails.
pub fn format_kotlin(source: &str, path: &str) -> Result<String, String> {
	if worker_disabled() {
		return fallback(source, path);
	}

	let script = path.ends_with(".kts");

	let outcome =
		WORKER.with(|slot| -> Result<Result<String, String>, String> {
			let mut slot = slot.borrow_mut();
			if slot.is_none() {
				*slot = match spawn_worker() {
					Some(worker) => Some(worker),
					None => {
						return Err("ktfmt-worker is not available".to_string())
					}
				};
			}
			slot.as_mut()
				.expect("worker was just spawned")
				.format(source, script)
		});

	match outcome {
		Ok(Ok(formatted)) => {
			note_worker_success();
			Ok(formatted)
		}
		// Per-file format error: the worker is still alive and protocol-synced.
		Ok(Err(message)) => {
			note_worker_success();
			Err(message)
		}
		// Protocol-level failure: drop the worker and let this file fall back.
		// The next file respawns a fresh worker; repeated failures disable the
		// worker for this thread entirely.
		Err(error) => {
			drop_worker();
			note_worker_failure();
			eprintln!("fama: {error}; falling back to process-based ktfmt");
			fallback(source, path)
		}
	}
}

/// Resolve the worker executable, mirroring the discovery rules from the crate
/// docs. Public so tests and tools can probe availability.
pub fn worker_command() -> Option<Command> {
	if let Some(value) = env::var_os(WORKER_ENV) {
		if !value.is_empty() {
			let path = PathBuf::from(&value);
			if is_executable(&path) {
				return Some(Command::new(path));
			}
			warn_once(|| {
				format!(
					"{WORKER_ENV} is set to `{}` but that is not an executable file",
					value.to_string_lossy()
				)
			});
			return None;
		}
	}
	find_on_path(WORKER_PROGRAM).map(Command::new)
}

fn worker_disabled() -> bool {
	WORKER_FAILURES
		.with(|failures| failures.get() >= MAX_CONSECUTIVE_WORKER_FAILURES)
}

fn note_worker_success() {
	WORKER_FAILURES.with(|failures| failures.set(0));
}

fn note_worker_failure() {
	WORKER_FAILURES
		.with(|failures| failures.set(failures.get().saturating_add(1)));
}

fn drop_worker() {
	WORKER.with(|slot| *slot.borrow_mut() = None);
}

fn fallback(source: &str, path: &str) -> Result<String, String> {
	fama_process::format_kotlin(source, path)
}

fn spawn_worker() -> Option<KotlinWorker> {
	let mut command = worker_command()?;
	command
		.args(worker_args())
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::inherit());

	let mut child = command.spawn().ok()?;
	let stdin = child.stdin.take()?;
	let stdout = child.stdout.take()?;

	Some(KotlinWorker {
		child,
		stdin,
		stdout,
	})
}

/// CLI arguments for the worker. fama's central configuration maps to
/// kotlinlang style (the mapping the process-based path always used), plus one
/// generated .editorconfig per run for the width/indent keys.
fn worker_args() -> Vec<String> {
	let mut args = vec![String::from("--kotlinlang-style")];
	if let Some(path) = editorconfig_path() {
		args.push("--editorconfig".to_string());
		args.push(path.display().to_string());
	}
	args
}

/// Path of the single generated .editorconfig, written at most once per process
/// and kept alive (leaked) for the whole run. `None` when the temp dir cannot
/// be created or written.
fn editorconfig_path() -> Option<&'static Path> {
	static EDITORCONFIG_PATH: OnceLock<Option<&'static Path>> = OnceLock::new();

	*EDITORCONFIG_PATH.get_or_init(|| {
		let directory = match tempfile::tempdir() {
			Ok(directory) => directory,
			Err(_) => return None,
		};
		// The directory must survive until the worker reads the file, so it is
		// leaked deliberately; one temp file per run, reclaimed by the OS.
		let directory: &'static tempfile::TempDir =
			Box::leak(Box::new(directory));

		let path: &'static Path =
			Box::leak(directory.path().join(".editorconfig").into_boxed_path());
		if fs::write(path, editorconfig_contents(&FormatConfig::default()))
			.is_err()
		{
			return None;
		}
		Some(path)
	})
}

/// A live worker child process with framed stdin/stdout handles.
struct KotlinWorker {
	child: Child,
	stdin: ChildStdin,
	stdout: ChildStdout,
}

impl KotlinWorker {
	/// Send one request and read one response.
	///
	/// Returns `Ok(Ok(formatted))` on success and `Ok(Err(message))` for a
	/// per-file format error (the worker survives both). An outer `Err` means
	/// the protocol broke or the worker died; the caller must drop the worker.
	fn format(
		&mut self,
		source: &str,
		script: bool,
	) -> Result<Result<String, String>, String> {
		let kind = if script { KIND_SCRIPT } else { KIND_REGULAR };

		let mut request = Vec::with_capacity(source.len() + 1);
		request.push(kind);
		request.extend_from_slice(source.as_bytes());

		write_frame(&mut self.stdin, &request)
			.and_then(|()| self.stdin.flush())
			.map_err(|error| {
				format!("failed to send request to ktfmt-worker: {error}")
			})?;

		let (status, payload) = read_response(&mut self.stdout)?;
		match status {
			STATUS_OK => {
				let formatted =
					String::from_utf8(payload).map_err(|error| {
						format!(
							"ktfmt-worker returned non-UTF-8 output: {error}"
						)
					})?;
				Ok(Ok(formatted))
			}
			STATUS_FORMAT_ERROR => {
				Ok(Err(format!("ktfmt: {}", String::from_utf8_lossy(&payload))))
			}
			STATUS_FATAL => Err(format!(
				"ktfmt-worker reported a fatal error: {}",
				String::from_utf8_lossy(&payload)
			)),
			other => Err(format!(
				"ktfmt-worker sent unknown response status {other}"
			)),
		}
	}
}

impl Drop for KotlinWorker {
	fn drop(&mut self) {
		let _ = self.child.kill();
		let _ = self.child.wait();
	}
}

/// Write one request frame: `len:u32 | payload`.
fn write_frame<W: Write>(
	writer: &mut W,
	payload: &[u8],
) -> std::io::Result<()> {
	writer.write_all(&(payload.len() as u32).to_be_bytes())?;
	writer.write_all(payload)
}

/// Read one response frame: `status:u8 | len:u32 | payload`.
fn read_response<R: Read>(reader: &mut R) -> Result<(u8, Vec<u8>), String> {
	let mut header = [0u8; 5];
	reader.read_exact(&mut header).map_err(|error| {
		format!("failed to read ktfmt-worker response header: {error}")
	})?;

	let status = header[0];
	let length =
		u32::from_be_bytes([header[1], header[2], header[3], header[4]])
			as usize;
	if length > MAX_RESPONSE_BYTES {
		return Err(format!(
			"ktfmt-worker response frame of {length} bytes exceeds the {MAX_RESPONSE_BYTES} byte limit"
		));
	}

	let mut payload = vec![0u8; length];
	reader.read_exact(&mut payload).map_err(|error| {
		format!("failed to read ktfmt-worker response payload: {error}")
	})?;

	Ok((status, payload))
}

fn find_on_path(program: &str) -> Option<PathBuf> {
	let path = env::var_os("PATH")?;

	for directory in env::split_paths(&path) {
		let candidate = directory.join(program);
		if is_executable(&candidate) {
			return Some(candidate);
		}
		#[cfg(windows)]
		{
			let with_exe = directory.join(format!("{program}.exe"));
			if is_executable(&with_exe) {
				return Some(with_exe);
			}
		}
	}
	None
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
	use std::os::unix::fs::PermissionsExt;

	path.is_file()
		&& fs::metadata(path)
			.is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(windows)]
fn is_executable(path: &Path) -> bool {
	path.is_file()
		&& path.extension().is_some_and(|extension| {
			let extension = extension.to_string_lossy().to_ascii_lowercase();
			extension == "exe" || extension == "bat" || extension == "cmd"
		})
}

fn warn_once(message: impl FnOnce() -> String) {
	static WARNED: OnceLock<()> = OnceLock::new();
	if WARNED.set(()).is_ok() {
		eprintln!("fama: {}", message());
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::io::Cursor;

	#[test]
	fn test_frame_roundtrip() {
		let mut buffer = Vec::new();
		write_frame(&mut buffer, b"hello").unwrap();
		// Response framing adds a leading status byte.
		buffer.insert(0, STATUS_OK);

		let mut cursor = Cursor::new(buffer);
		let (status, payload) = read_response(&mut cursor).unwrap();
		assert_eq!(status, STATUS_OK);
		assert_eq!(payload, b"hello");
	}

	#[test]
	fn test_read_response_rejects_oversized_frame() {
		let mut header = vec![STATUS_OK];
		header
			.extend_from_slice(&(MAX_RESPONSE_BYTES as u32 + 1).to_be_bytes());
		let mut cursor = Cursor::new(header);

		let error = read_response(&mut cursor).unwrap_err();
		assert!(error.contains("exceeds"), "{error}");
	}

	#[test]
	fn test_read_response_empty_stream_is_protocol_error() {
		let mut cursor = Cursor::new(Vec::new());
		let error = read_response(&mut cursor).unwrap_err();
		assert!(error.contains("failed to read"), "{error}");
	}

	#[test]
	fn test_worker_args_style_and_editorconfig() {
		let args = worker_args();
		assert_eq!(args[0], "--kotlinlang-style");
		if let Some(path) = editorconfig_path() {
			assert!(path.is_file(), "generated editorconfig must exist");
			let contents = fs::read_to_string(path).unwrap();
			assert!(contents.contains("max_line_length = 80"));
		} else {
			assert!(!args.contains(&"--editorconfig".to_string()));
		}
	}

	#[test]
	fn test_find_on_path_returns_none_for_missing_program() {
		assert!(find_on_path("definitely-not-ktfmt-worker-for-fama").is_none());
	}

	#[test]
	fn test_request_payload_prefixes_kind() {
		// Regular vs script sources differ only in the leading kind byte.
		let mut regular = Vec::new();
		regular.push(KIND_REGULAR);
		regular.extend_from_slice(b"class A");
		let mut script = Vec::new();
		script.push(KIND_SCRIPT);
		script.extend_from_slice(b"class A");
		assert_eq!(regular[0], 0);
		assert_eq!(script[0], 1);
	}
}
