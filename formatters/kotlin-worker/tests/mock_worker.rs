//! End-to-end tests of the worker client against the mock worker binary, so
//! CI never needs the real ktfmt sidecar.
//!
//! The client discovers the worker through `FAMA_KTFMT_WORKER`, which is
//! process-global, and pools workers per thread across calls, so all scenarios
//! run inside one `#[test]` in a deliberate order: the missing-worker case must
//! run before any worker is pooled, because a pooled worker is reused until it
//! fails regardless of the env var.

use std::env;
use std::fs;

use fama_kotlin_worker::WORKER_ENV;

const MOCK: &str = env!("CARGO_BIN_EXE_mock-ktfmt-worker");

/// Format through the client. The mock's successful output always carries the
/// `// formatted` marker; the process fallback (real ktfmt CLI) never would.
fn formatted(source: &str, path: &str) -> Result<String, String> {
	fama_kotlin_worker::format_kotlin(source, path)
}

/// Assert the outcome did NOT come from the mock worker: either the process
/// fallback formatted it (real ktfmt installed) or it errored.
fn assert_not_mock_output(outcome: &Result<String, String>) {
	assert!(
		match outcome {
			Ok(output) => !output.contains("// formatted"),
			Err(error) => !error.contains("// formatted"),
		},
		"must not come from the mock worker, got {outcome:?}"
	);
}

#[test]
fn worker_lifecycle_over_mock() {
	// 1. Missing worker binary: everything goes through the process fallback.
	env::set_var(
		WORKER_ENV,
		"/nonexistent/fama/definitely-not-a-ktfmt-worker",
	);
	let outcome = formatted("class A\n", "A.kt");
	assert_not_mock_output(&outcome);

	// 2. Happy path: the mock echoes the source with a marker appended.
	env::set_var(WORKER_ENV, MOCK);
	let result = formatted("class A\n", "A.kt").expect("worker path must succeed");
	assert_eq!(result, "class A\n// formatted\n");

	// 3. Argument plumbing: style flag and the once-per-run editorconfig.
	let result = formatted("// MOCK-ARGS\n", "A.kt").expect("args probe must succeed");
	let args: Vec<&str> = result.lines().collect();
	assert_eq!(args[0], "--kotlinlang-style");
	assert_eq!(args[1], "--editorconfig");
	let editorconfig = fs::read_to_string(args[2]).expect("editorconfig exists");
	assert!(editorconfig.contains("max_line_length = 80"));
	assert!(editorconfig.contains("indent_style = tab"));

	// 4. Per-file format error: Err for this file...
	let error = formatted("// MOCK-ERR\nclass A\n", "A.kt").unwrap_err();
	assert!(error.contains("mock format error"), "{error}");
	// ...and the SAME worker still serves the next request.
	let result =
		formatted("class B\n", "B.kt").expect("worker must survive a format error");
	assert_eq!(result, "class B\n// formatted\n");

	// 5. Worker crash mid-request: the file falls back to the process path, and
	//    the next file respawns a fresh worker.
	let outcome = formatted("// MOCK-CRASH\nclass A\n", "A.kt");
	assert_not_mock_output(&outcome);
	let result =
		formatted("class C\n", "C.kt").expect("worker respawns after crash");
	assert_eq!(result, "class C\n// formatted\n");

	// 6. Fatal response: same expectations as a crash.
	let outcome = formatted("// MOCK-FATAL\nclass A\n", "A.kt");
	assert_not_mock_output(&outcome);
	let result =
		formatted("class D\n", "D.kt").expect("worker respawns after fatal");
	assert_eq!(result, "class D\n// formatted\n");

	// 7. The .kts extension selects the script kind; the mock's behavior does
	//    not depend on the kind, but the request must not be rejected.
	let result = formatted("plugins {}\n", "build.gradle.kts")
		.expect("script kind must be accepted by the worker");
	assert_eq!(result, "plugins {}\n// formatted\n");
}
