//! V language formatter via `v fmt` (process mode).

use crate::process::{command_error, resolve_binary, success_status};
use fama_common::Formatter;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

pub const V_PATH_ENV: &str = "FAMA_V_PATH";

const INSTALL_HINT: &str =
	"Install V (`v`, https://vlang.io) to format V files, \
	 or point FAMA_V_PATH at the binary.";

/// Formatter implementation for the V programming language via `v fmt`.
pub struct VFormatter;

impl Formatter for VFormatter {
	fn name(&self) -> &'static str {
		"v"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["v"]
	}

	fn needs_content_check(&self, ext: &str) -> bool {
		ext == "v"
	}

	fn match_confidence(&self, path: &Path, content: &str) -> u8 {
		if path.extension().and_then(|e| e.to_str()) == Some("v") {
			v_confidence(content)
		} else {
			0
		}
	}

	fn format(&self, content: &str, _path: &Path) -> Result<String, String> {
		format_v(content)
	}
}

/// Classify whether a `.v` file prefix is V source code rather than Verilog,
/// SystemVerilog, or Coq/Rocq. Fails closed (`0`) when inconclusive.
pub fn v_confidence(content: &str) -> u8 {
	if looks_like_coq(content) || looks_like_verilog(content) {
		return 0;
	}
	if looks_like_v(content) {
		return 95;
	}
	0
}

fn looks_like_coq(content: &str) -> bool {
	const COQ_LINE_PREFIXES: &[&str] = &[
		"Theorem ",
		"Lemma ",
		"Proposition ",
		"Corollary ",
		"Remark ",
		"Fact ",
		"Example ",
		"Proof.",
		"Proof ",
		"Qed.",
		"Defined.",
		"Admitted.",
		"Require ",
		"From ",
		"Inductive ",
		"CoInductive ",
		"Fixpoint ",
		"CoFixpoint ",
		"Definition ",
		"Axiom ",
		"Hypothesis ",
		"Variable ",
		"Variables ",
		"Parameter ",
		"Section ",
		"End ",
		"Notation ",
		"Ltac ",
		"Record ",
		"Class ",
		"Instance ",
		"Check ",
		"Compute ",
		"Hint ",
		"Arguments ",
		"Module ",
	];
	if content.contains("Require Import")
		|| content.contains("Require Export")
		|| content.contains("Proof.")
		|| content.contains("Qed.")
	{
		return true;
	}
	content.lines().any(|line| {
		let trimmed = line.trim_start();
		COQ_LINE_PREFIXES.iter().any(|p| trimmed.starts_with(p))
	})
}

fn looks_like_verilog(content: &str) -> bool {
	const VERILOG_END_KEYWORDS: &[&str] = &[
		"endmodule",
		"endfunction",
		"endtask",
		"endpackage",
		"endinterface",
		"endgenerate",
		"endcase",
		"endprimitive",
		"endspecify",
	];
	if VERILOG_END_KEYWORDS.iter().any(|kw| content.contains(kw)) {
		return true;
	}

	const VERILOG_DIRECTIVES: &[&str] = &[
		"`timescale",
		"`include",
		"`define",
		"`ifdef",
		"`ifndef",
		"`endif",
		"`default_nettype",
	];
	const VERILOG_LINE_PREFIXES: &[&str] = &[
		"always ",
		"always@",
		"always_ff",
		"always_comb",
		"always_latch",
		"assign ",
		"wire ",
		"reg ",
		"logic ",
		"initial ",
		"localparam ",
		"parameter ",
		"input ",
		"output ",
		"inout ",
		"typedef struct",
		"typedef enum",
	];

	for line in content.lines() {
		let trimmed = line.trim_start();
		if trimmed.starts_with("//") || trimmed.starts_with("/*") {
			continue;
		}
		if VERILOG_DIRECTIVES.iter().any(|d| trimmed.starts_with(d))
			|| VERILOG_LINE_PREFIXES.iter().any(|p| trimmed.starts_with(p))
		{
			return true;
		}
		// Verilog `module foo(...);` / `module foo #(...);` / `module foo;`
		if let Some(rest) = trimmed.strip_prefix("module ") {
			let code = rest.split("//").next().unwrap_or("").trim();
			if code.contains('(') || code.contains(';') || code.contains('#') {
				return true;
			}
		}
		// SystemVerilog `import pkg::*;`
		if let Some(rest) = trimmed.strip_prefix("import ") {
			if rest.contains("::") || rest.trim_end().ends_with(';') {
				return true;
			}
		}
	}
	false
}

fn is_v_ident(s: &str) -> bool {
	let mut chars = s.chars();
	let Some(first) = chars.next() else {
		return false;
	};
	(first.is_ascii_alphabetic() || first == '_')
		&& chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn looks_like_v(content: &str) -> bool {
	const V_LINE_PREFIXES: &[&str] = &[
		"fn ",
		"pub fn ",
		"pub struct ",
		"pub enum ",
		"pub interface ",
		"pub const ",
		"pub type ",
		"pub mut:",
		"mut:",
		"@[",
		"$if ",
		"$for ",
	];

	for line in content.lines() {
		let trimmed = line.trim_start();
		if trimmed.starts_with("//") || trimmed.starts_with("/*") {
			continue;
		}
		if V_LINE_PREFIXES.iter().any(|p| trimmed.starts_with(p)) {
			return true;
		}
		// V bare module header: `module main` / `module my_pkg`
		if let Some(rest) = trimmed.strip_prefix("module ") {
			let code = rest.split("//").next().unwrap_or("").trim();
			if is_v_ident(code) {
				return true;
			}
		}
		// V import: `import os` / `import net.http as http` (no `::` or `;`)
		if let Some(rest) = trimmed.strip_prefix("import ") {
			let code = rest.split("//").next().unwrap_or("").trim();
			if !code.is_empty()
				&& !code.contains("::")
				&& !code.ends_with(';')
				&& code
					.split_whitespace()
					.next()
					.is_some_and(|pkg| pkg.split('.').all(is_v_ident))
			{
				return true;
			}
		}
		// V struct / enum block opening without semicolon: `struct Foo {`
		if (trimmed.starts_with("struct ") || trimmed.starts_with("enum "))
			&& trimmed.contains('{')
			&& !trimmed.contains(';')
		{
			return true;
		}
	}

	content.contains("println(")
		|| content.contains("eprintln(")
		|| content.contains(":=")
}

pub fn v_binary() -> String {
	resolve_binary(V_PATH_ENV, "v")
}

pub fn format_v(source: &str) -> Result<String, String> {
	format_v_with_bin(&v_binary(), source)
}

fn format_v_with_bin(bin: &str, source: &str) -> Result<String, String> {
	let temp_dir = TempDir::new()
		.map_err(|e| format!("v formatting: failed to create temp dir: {e}"))?;
	let temp_file = temp_dir.path().join("input.v");
	fs::write(&temp_file, source)
		.map_err(|e| format!("v formatting: failed to write temp file: {e}"))?;

	let output = Command::new(bin)
		.arg("fmt")
		.arg(&temp_file)
		.output()
		.map_err(|e| command_error(bin, e, "v", INSTALL_HINT))?;

	success_status(&output, "v")?;
	String::from_utf8(output.stdout)
		.map_err(|e| format!("v fmt output was not valid UTF-8: {e}"))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_v_metadata_and_env() {
		assert_eq!(VFormatter.name(), "v");
		assert_eq!(VFormatter.extensions(), &["v"]);
		assert!(VFormatter.needs_content_check("v"));
		assert!(!VFormatter.needs_content_check("vv"));
		assert!(!v_binary().is_empty());
	}

	#[test]
	fn test_v_confidence_disambiguates_v_verilog_and_coq() {
		let path = Path::new("main.v");

		// Genuine V sources
		for v_src in [
			"module main\n\nfn main() {\n\tprintln('hi')\n}\n",
			"fn add(a int, b int) int {\n\treturn a + b\n}\n",
			"module helpers\n\n// always check input\nfn parse(input string) string {\n\treturn input\n}\n",
			"pub struct Point {\n\tx int\n\ty int\n}\n",
			"import os\n\nx := os.args.len\nprintln(x)\n",
		] {
			assert!(
				VFormatter.match_confidence(path, v_src) >= 90,
				"expected V match for:\n{v_src}"
			);
		}

		// Verilog / SystemVerilog sources -> decline (0)
		for verilog_src in [
			"module top(input wire a, output wire b);\n\tassign b = a;\nendmodule\n",
			"`timescale 1ns/1ps\nmodule big_alu #(\n\tparameter WIDTH = 32\n) (\n\tinput wire clk\n);\n",
			"import my_pkg::*;\nmodule top(input logic a);\nendmodule\n",
		] {
			assert_eq!(
				VFormatter.match_confidence(path, verilog_src),
				0,
				"expected V to decline Verilog:\n{verilog_src}"
			);
		}

		// Coq / Rocq sources -> decline (0)
		for coq_src in [
			"Theorem foo : 1 = 1. Proof. reflexivity. Qed.\n",
			"Require Import Coq.Arith.Arith.\nDefinition add1 (n : nat) := n + 1.\n",
			"Section Helpers.\nVariable x : nat.\nEnd Helpers.\n",
			"From Stdlib Require Import List.\nInductive tree := Leaf | Node.\n",
		] {
			assert_eq!(
				VFormatter.match_confidence(path, coq_src),
				0,
				"expected V to decline Coq:\n{coq_src}"
			);
		}

		// Ambiguous / empty -> fail closed (0)
		assert_eq!(VFormatter.match_confidence(path, ""), 0);
		assert_eq!(VFormatter.match_confidence(path, "// just a comment\n"), 0);
	}

	#[test]
	fn test_missing_v_reports_helpful_error() {
		let err = format_v_with_bin(
			"fama-nonexistent-v-bin",
			"fn main() { println('hi') }\n",
		)
		.unwrap_err();
		assert!(err.contains("was not found in PATH"), "{err}");
		assert!(err.contains(V_PATH_ENV), "{err}");
	}

	#[test]
	fn test_format_v_live_if_installed() {
		let bin = v_binary();
		if !Command::new(&bin)
			.arg("--version")
			.output()
			.is_ok_and(|o| o.status.success())
		{
			return;
		}
		let once = format_v("fn main(){println('hi')}\n").unwrap();
		let twice = format_v(&once).unwrap();
		assert_eq!(once, twice);
	}
}
