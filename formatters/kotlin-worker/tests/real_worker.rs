//! Integration test against a REAL ktfmt-worker binary.
//!
//! Skipped unless a worker is configured:
//!
//! ```sh
//! FAMA_KTFMT_WORKER=/path/to/ktfmt-worker \
//!   cargo test -p fama-kotlin-worker -- --ignored
//! ```
//!
//! A `ktfmt-worker` on `PATH` is also picked up. CI never has the worker, so
//! this test is `#[ignore]`d there.

use fama_kotlin_worker::worker_command;

const REGULAR_SOURCE: &str =
	"class Foo(val bar:String){\n\tfun baz(x:Int):Int{\n\t\treturn x+1\n\t}\n}";

const SCRIPT_SOURCE: &str = "plugins {\n\tid(\"java-library\")\n}";

#[test]
#[ignore = "requires a real ktfmt-worker binary (set FAMA_KTFMT_WORKER or install ktfmt-worker on PATH)"]
fn real_worker_formats_kotlin() {
	if worker_command().is_none() {
		eprintln!(
			"skipping: no ktfmt-worker found; set FAMA_KTFMT_WORKER or install ktfmt-worker on PATH"
		);
		return;
	}

	// Regular .kt: kotlinlang style, 4-space indent, trailing newline.
	let formatted =
		fama_kotlin_worker::format_kotlin(REGULAR_SOURCE, "src/Foo.kt")
			.expect("real worker must format valid Kotlin");
	assert_eq!(
		formatted,
		"class Foo(val bar: String) {\n    fun baz(x: Int): Int {\n        return x + 1\n    }\n}\n"
	);

	// Formatting is idempotent.
	let again = fama_kotlin_worker::format_kotlin(&formatted, "src/Foo.kt")
		.expect("real worker must format valid Kotlin");
	assert_eq!(formatted, again);

	// Script (.kts): parsed with the script PSI type; regular parsing of the
	// same content fails with "Expecting a top level declaration".
	let formatted_script =
		fama_kotlin_worker::format_kotlin(SCRIPT_SOURCE, "build.gradle.kts")
			.expect("real worker must format valid Kotlin scripts");
	assert!(
		formatted_script.starts_with("plugins {"),
		"{formatted_script}"
	);

	// Invalid source: per-file error, and the worker recovers for the next file.
	let error = fama_kotlin_worker::format_kotlin("class { ~~~", "Broken.kt")
		.unwrap_err();
	assert!(!error.is_empty(), "expected a parse error message");
	let recovered = fama_kotlin_worker::format_kotlin("class Ok\n", "Ok.kt")
		.expect("worker must survive a format error");
	assert!(recovered.starts_with("class Ok"), "{recovered}");
}
