//! Test double speaking the ktfmt-worker framing protocol.
//!
//! Used by the integration tests so CI never needs the real (~56 MB) ktfmt
//! binary. Built via the `[[bin]]` target in Cargo.toml; tests reference it
//! through `CARGO_BIN_EXE_mock-ktfmt-worker`.
//!
//! Behavior driven by the request source:
//!
//! - `// MOCK-ERR\n...`   → status 1 ("mock format error"), worker stays alive
//! - `// MOCK-FATAL\n...` → status 2 ("mock fatal"), worker exits
//! - `// MOCK-CRASH\n...` → no response at all, worker exits immediately
//! - `// MOCK-ARGS\n...`  → status 0, payload is the worker's argv, one per line
//! - anything else        → status 0, payload is the source with `// formatted`
//!   appended

use std::env;
use std::io::{Read, Write};
use std::process::exit;

const STATUS_OK: u8 = 0;
const STATUS_FORMAT_ERROR: u8 = 1;
const STATUS_FATAL: u8 = 2;

fn main() {
	let args: Vec<String> = env::args().skip(1).collect();
	let mut stdin = std::io::stdin().lock();
	let mut stdout = std::io::stdout().lock();

	while let Ok((kind, source)) = read_frame(&mut stdin) {
		if kind != 0 && kind != 1 {
			let payload = format!("mock: unknown request kind {kind}");
			let _ = write_frame(&mut stdout, STATUS_FATAL, payload.as_bytes());
			let _ = stdout.flush();
			exit(2);
		}

		if source.starts_with("// MOCK-ERR\n") {
			write_frame(&mut stdout, STATUS_FORMAT_ERROR, b"mock format error")
				.unwrap();
		} else if source.starts_with("// MOCK-FATAL\n") {
			write_frame(&mut stdout, STATUS_FATAL, b"mock fatal").unwrap();
			let _ = stdout.flush();
			exit(3);
		} else if source.starts_with("// MOCK-CRASH\n") {
			// Die without answering: simulates a worker killed mid-request.
			exit(4);
		} else if source.starts_with("// MOCK-ARGS\n") {
			let payload = args.join("\n");
			write_frame(&mut stdout, STATUS_OK, payload.as_bytes()).unwrap();
		} else {
			let mut formatted = source;
			formatted.push_str("// formatted\n");
			write_frame(&mut stdout, STATUS_OK, formatted.as_bytes()).unwrap();
		}
		stdout.flush().unwrap();
	}
	// EOF on stdin: clean shutdown.
}

fn read_frame(reader: &mut impl Read) -> std::io::Result<(u8, String)> {
	let mut header = [0u8; 4];
	reader.read_exact(&mut header)?;
	let length = u32::from_be_bytes(header) as usize;
	let mut payload = vec![0u8; length];
	reader.read_exact(&mut payload)?;
	let kind = payload[0];
	let source = String::from_utf8_lossy(&payload[1..]).into_owned();
	Ok((kind, source))
}

fn write_frame(
	writer: &mut impl Write,
	status: u8,
	payload: &[u8],
) -> std::io::Result<()> {
	writer.write_all(&[status])?;
	writer.write_all(&(payload.len() as u32).to_be_bytes())?;
	writer.write_all(payload)
}
