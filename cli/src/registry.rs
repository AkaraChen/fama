//! Central formatter registry built from all enabled language formatters.

use fama_common::Formatter;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

enum ExtSlot {
	Unique(usize),
	Ambiguous(Vec<usize>),
}

/// Result of path-only resolution before inspecting file contents.
pub enum ResolveOutcome {
	/// Unambiguously handled by a single formatter.
	Unique(usize, &'static dyn Formatter),
	/// Extension is claimed by multiple formatters or guarded by content checks.
	NeedsContent,
	/// Not claimed by any formatter.
	None,
}

/// Registry mapping file extensions and special filenames to `Formatter` instances.
pub struct Registry {
	formatters: Vec<&'static dyn Formatter>,
	by_ext: HashMap<&'static str, ExtSlot>,
}

impl Registry {
	fn new() -> Self {
		fama_common::register_host_formatter(format_via_registry);
		let mut formatters: Vec<&'static dyn Formatter> = Vec::with_capacity(
			fama_builtin::FORMATTERS.len()
				+ fama_web::FORMATTERS.len()
				+ fama_process::FORMATTERS.len()
				+ fama_topiary::FORMATTERS.len()
				+ fama_dprint_wasm::FORMATTERS.len()
				+ 6,
		);
		formatters.extend_from_slice(fama_builtin::FORMATTERS);
		formatters.extend_from_slice(fama_web::FORMATTERS);
		formatters.extend_from_slice(fama_process::FORMATTERS);
		formatters.extend_from_slice(fama_topiary::FORMATTERS);
		formatters.extend_from_slice(fama_dprint_wasm::FORMATTERS);
		formatters.extend_from_slice(&[
			&fama_clang::FORMATTER,
			&fama_dart::FORMATTER,
			&fama_swift::FORMATTER,
			&goffi::FORMATTER,
			&ruby_fmt::FORMATTER,
			&fama_kotlin_worker::FORMATTER,
		]);
		Self::from_formatters(formatters)
	}

	/// Build a registry from a custom list of formatters.
	pub fn from_formatters(formatters: Vec<&'static dyn Formatter>) -> Self {
		let mut by_ext: HashMap<&'static str, ExtSlot> = HashMap::new();
		for (idx, fmt) in formatters.iter().enumerate() {
			for &ext in fmt.extensions() {
				match by_ext.get_mut(ext) {
					None => {
						by_ext.insert(ext, ExtSlot::Unique(idx));
					}
					Some(ExtSlot::Unique(prev)) => {
						assert_ne!(
							*prev,
							idx,
							"Duplicate extension .{ext} registered by {}",
							fmt.name()
						);
						let first = *prev;
						by_ext.insert(
							ext,
							ExtSlot::Ambiguous(vec![first, idx]),
						);
					}
					Some(ExtSlot::Ambiguous(indices)) => {
						assert!(
							!indices.contains(&idx),
							"Duplicate extension .{ext} registered by {}",
							fmt.name()
						);
						indices.push(idx);
					}
				}
			}
		}

		Self { formatters, by_ext }
	}

	/// Path-only resolution step. Does not touch disk.
	pub fn resolve_path(&self, path: &Path) -> ResolveOutcome {
		if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
			if let Some(slot) = self.by_ext.get(ext) {
				return match slot {
					ExtSlot::Unique(idx) => {
						let fmt = self.formatters[*idx];
						if fmt.needs_content_check(ext) {
							ResolveOutcome::NeedsContent
						} else {
							ResolveOutcome::Unique(*idx, fmt)
						}
					}
					ExtSlot::Ambiguous(_) => ResolveOutcome::NeedsContent,
				};
			}
		}
		if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
			for (idx, &fmt) in self.formatters.iter().enumerate() {
				if fmt.matches_filename(name) {
					return ResolveOutcome::Unique(idx, fmt);
				}
			}
		}
		ResolveOutcome::None
	}

	/// Resolve the formatter `(index, formatter)` using file path and content/prefix.
	pub fn resolve_with_content(
		&self,
		path: &Path,
		content: &str,
	) -> Option<(usize, &'static dyn Formatter)> {
		if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
			if let Some(slot) = self.by_ext.get(ext) {
				return match slot {
					ExtSlot::Unique(idx) => {
						let fmt = self.formatters[*idx];
						if !fmt.needs_content_check(ext) {
							Some((*idx, fmt))
						} else {
							self.disambiguate_candidates(
								path,
								ext,
								content,
								std::slice::from_ref(idx),
							)
						}
					}
					ExtSlot::Ambiguous(indices) => self.disambiguate_candidates(
						path,
						ext,
						content,
						indices,
					),
				};
			}
		}
		if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
			for (idx, &fmt) in self.formatters.iter().enumerate() {
				if fmt.matches_filename(name) {
					return Some((idx, fmt));
				}
			}
		}
		None
	}

	/// Disambiguate among `candidates` for extension `ext` via Linguist
	/// (modeline first, then content heuristics). Fails closed (`None`) when
	/// Linguist detects no language or detects a language not claimed by a
	/// candidate formatter.
	fn disambiguate_candidates(
		&self,
		path: &Path,
		ext: &str,
		content: &str,
		candidates: &[usize],
	) -> Option<(usize, &'static dyn Formatter)> {
		let detected = match linguist::detect_language_by_modeline(content) {
			Ok(langs) if !langs.is_empty() => langs,
			_ => match linguist::disambiguate(path, content) {
				Ok(langs) => langs,
				Err(_) => return None,
			},
		};
		if detected.is_empty() {
			return None;
		}
		candidates.iter().find_map(|&idx| {
			let fmt = self.formatters[idx];
			let langs = fmt.linguist_languages(ext);
			detected
				.iter()
				.all(|lang| langs.contains(&lang.name))
				.then_some((idx, fmt))
		})
	}

	/// Resolve the formatter `(index, formatter)` responsible for `path` without content.
	/// Returns `None` for ambiguous extensions claimed by multiple formatters.
	#[cfg(test)]
	pub fn resolve(&self, path: &Path) -> Option<(usize, &'static dyn Formatter)> {
		if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
			if let Some(slot) = self.by_ext.get(ext) {
				return match slot {
					ExtSlot::Unique(idx) => Some((*idx, self.formatters[*idx])),
					ExtSlot::Ambiguous(_) => None,
				};
			}
		}
		if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
			for (idx, &fmt) in self.formatters.iter().enumerate() {
				if fmt.matches_filename(name) {
					return Some((idx, fmt));
				}
			}
		}
		None
	}

	/// Return `true` if any registered formatter claims `path` (fast path, no disk I/O).
	pub fn is_supported(&self, path: &Path) -> bool {
		if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
			if self.by_ext.contains_key(ext) {
				return true;
			}
		}
		if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
			return self.formatters.iter().any(|fmt| fmt.matches_filename(name));
		}
		false
	}

	/// Return every registered formatter in deterministic order.
	pub fn all(&self) -> &[&'static dyn Formatter] {
		&self.formatters
	}
}

/// Global singleton formatter registry.
pub fn registry() -> &'static Registry {
	static REGISTRY: OnceLock<Registry> = OnceLock::new();
	REGISTRY.get_or_init(Registry::new)
}

fn format_via_registry(
	content: &str,
	path: &Path,
) -> Option<Result<String, String>> {
	registry()
		.resolve_with_content(path, content)
		.map(|(_, fmt)| fmt.format(content, path))
}

#[cfg(test)]
mod tests {
	use super::*;

	fn resolve_name(path: &str) -> Option<&'static str> {
		registry().resolve(Path::new(path)).map(|(_, fmt)| fmt.name())
	}

	#[test]
	fn test_registry_has_no_collisions_and_all_formatters() {
		let reg = registry();
		assert_eq!(reg.all().len(), 48);
	}

	#[test]
	fn test_resolve_web_and_data_languages() {
		for path in [
			"test.js",
			"test.cjs",
			"test.mjs",
			"test.ts",
			"test.mts",
			"test.cts",
			"test.jsx",
			"test.mjsx",
			"test.cjsx",
			"test.tsx",
			"test.mtsx",
			"test.ctsx",
			"test.json",
			"package.json",
			"tsconfig.jsonc",
			"test.html",
			"test.htm",
			"test.vue",
			"test.svelte",
			"test.astro",
			"query.graphql",
			"query.gql",
		] {
			assert_eq!(resolve_name(path), Some("biome"), "{path}");
		}

		for path in [
			"test.yaml",
			"test.yml",
			"test.md",
			"test.markdown",
			"test.css",
			"test.scss",
			"test.less",
			"test.sass",
		] {
			assert_eq!(resolve_name(path), Some("dprint"), "{path}");
		}
	}

	#[test]
	fn test_resolve_systems_and_scripting_languages() {
		assert_eq!(resolve_name("test.toml"), Some("toml"));
		assert_eq!(resolve_name("Cargo.toml"), Some("toml"));
		assert_eq!(resolve_name("path/to/config.toml"), Some("toml"));
		assert_eq!(resolve_name("test.rs"), Some("rust"));
		assert_eq!(resolve_name("test.py"), Some("python"));
		assert_eq!(resolve_name("test.lua"), Some("lua"));
		assert_eq!(resolve_name("test.xml"), Some("xml"));
		assert_eq!(resolve_name("test.sql"), Some("sql"));
		assert_eq!(resolve_name("test.php"), Some("php"));
		assert_eq!(resolve_name("template.phtml"), Some("php"));
		assert_eq!(resolve_name("test.nix"), Some("nix"));
		assert_eq!(resolve_name("test.scala"), Some("scala"));
		assert_eq!(resolve_name("build.sc"), Some("scala"));
		assert_eq!(resolve_name("script.R"), Some("r"));
		assert_eq!(resolve_name("script.r"), Some("r"));
		assert_eq!(resolve_name("app.ex"), Some("elixir"));
		assert_eq!(resolve_name("mix.exs"), Some("elixir"));
		assert_eq!(resolve_name("shader.wgsl"), Some("wgsl"));
		assert_eq!(resolve_name("test.zig"), Some("zig"));
		assert_eq!(resolve_name("build.zig.zon"), Some("zig"));
		assert_eq!(resolve_name("test.dart"), Some("dart"));
		assert_eq!(resolve_name("Sources/App/Main.swift"), Some("swift"));
		assert_eq!(resolve_name("test.kt"), Some("kotlin"));
		assert_eq!(resolve_name("build.gradle.kts"), Some("kotlin"));
		assert_eq!(resolve_name("doc.typ"), Some("typst"));
		assert_eq!(resolve_name("tmpl.jinja2"), Some("markup"));
		assert_eq!(resolve_name("tmpl.hbs"), Some("markup"));
		assert_eq!(resolve_name("tmpl.vto"), Some("markup"));
		assert_eq!(resolve_name("config.kdl"), Some("kdl"));
		assert_eq!(resolve_name("player.gd"), Some("gdscript"));
		assert_eq!(resolve_name("paper.tex"), Some("latex"));
		assert_eq!(resolve_name("refs.bib"), Some("latex"));
		assert_eq!(resolve_name("Token.sol"), Some("solidity"));
		assert_eq!(resolve_name("app.gleam"), Some("gleam"));
		assert_eq!(resolve_name("schema.prisma"), Some("prisma"));
		assert_eq!(resolve_name("recipes.just"), Some("just"));
		assert_eq!(resolve_name("contract.cairo"), Some("cairo"));
		assert_eq!(resolve_name("coin.move"), Some("move"));
		assert_eq!(resolve_name("env.nu"), Some("nushell"));
		assert_eq!(resolve_name("config.fish"), Some("fish"));
		assert_eq!(resolve_name("app.cr"), Some("crystal"));
		assert_eq!(resolve_name("Main.qml"), Some("qml"));
		assert_eq!(resolve_name("kernel.mojo"), Some("mojo"));
		assert_eq!(resolve_name("kernel.🔥"), Some("mojo"));
		assert_eq!(resolve_name("main.bicep"), Some("bicep"));
		assert_eq!(resolve_name("prod.bicepparam"), Some("bicep"));
		assert_eq!(resolve_name("config.ncl"), Some("nickel"));
		assert_eq!(resolve_name("part.scad"), Some("openscad"));
		assert_eq!(resolve_name("cfg.json5"), Some("json5"));
		assert_eq!(resolve_name("analysis.ipynb"), Some("jupyter"));
		assert_eq!(resolve_name("Program.cs"), Some("csharpier"));
		assert_eq!(resolve_name("script.csx"), Some("csharpier"));
		assert_eq!(resolve_name("Module1.vb"), Some("roslyn"));
	}

	#[test]
	fn test_resolve_c_family_and_goffi() {
		for path in [
			"test.c",
			"test.h",
			"test.cpp",
			"test.cc",
			"test.cxx",
			"test.hpp",
			"test.hxx",
			"test.hh",
			"test.m",
			"test.mm",
			"Main.java",
			"msg.proto",
			"kernel.cu",
			"kernel.cuh",
			"shader.hlsl",
			"shader.hlsli",
			"shader.metal",
			"shader.glsl",
			"shader.vert",
			"shader.frag",
			"shader.comp",
			"top.sv",
			"top.svh",
			"defs.vh",
		] {
			assert_eq!(resolve_name(path), Some("clang"), "{path}");
		}

		for path in [
			"test.sh",
			"test.bash",
			"test.zsh",
			"main.go",
			"test.hcl",
			"main.tf",
			"variables.tfvars",
			"defs.bzl",
			"script.star",
			"cfg.cue",
			"main.jsonnet",
			"lib.libsonnet",
			"policy.rego",
		] {
			assert_eq!(resolve_name(path), Some("goffi"), "{path}");
		}

		for path in [
			"BUILD",
			"BUILD.bazel",
			"WORKSPACE",
			"WORKSPACE.bazel",
			"MODULE.bazel",
		] {
			assert_eq!(resolve_name(path), Some("goffi"), "{path}");
		}
	}

	#[test]
	fn test_resolve_special_filenames_and_unknown() {
		assert_eq!(resolve_name("Dockerfile"), Some("dockerfile"));
		assert_eq!(resolve_name("Dockerfile.dev"), Some("dockerfile"));
		assert_eq!(resolve_name("path/to/Dockerfile.prod"), Some("dockerfile"));

		assert_eq!(resolve_name("CMakeLists.txt"), Some("cmake"));
		assert_eq!(resolve_name("foo.cmake"), Some("cmake"));
		assert_eq!(resolve_name("config.cmake.in"), Some("cmake"));

		for name in ["justfile", "Justfile", ".justfile"] {
			assert_eq!(resolve_name(name), Some("just"), "{name}");
		}

		for name in [
			"meson.build",
			"subproject/meson.build",
			"meson.options",
			"meson_options.txt",
		] {
			assert_eq!(resolve_name(name), Some("meson"), "{name}");
		}

		for path in [
			"test.rb",
			"tasks.rake",
			"mygem.gemspec",
			"config.ru",
			"Rakefile",
			"Gemfile",
			"Guardfile",
			"Vagrantfile",
			"Fastfile",
		] {
			assert_eq!(resolve_name(path), Some("ruby"), "{path}");
		}

		assert_eq!(resolve_name("unknown.xyz"), None);
		assert_eq!(resolve_name("README"), None);
	}

	#[test]
	fn test_resolve_v_extension_disambiguates_v_verilog_and_coq() {
		let reg = registry();
		let path = Path::new("sample.v");

		assert!(reg.is_supported(path));
		assert!(matches!(reg.resolve_path(path), ResolveOutcome::NeedsContent));
		assert!(reg.resolve(path).is_none());

		let resolve = |content: &str| {
			reg.resolve_with_content(path, content)
				.map(|(_, fmt)| fmt.name())
		};

		// V sources (matched by Linguist heuristics)
		for v_src in [
			"module main\n\nfn main() {\n\tprintln('hi')\n}\n",
			"fn add(a int, b int) int {\n\treturn a + b\n}\n",
			"module helpers\n\n// always check input\nfn parse(input string) string {\n\treturn input\n}\n",
		] {
			assert_eq!(resolve(v_src), Some("v"), "V source:\n{v_src}");
		}

		// Verilog / SystemVerilog sources
		for verilog_src in [
			"module top(input wire a, output wire b);\n\tassign b = a;\nendmodule\n",
			"`timescale 1ns/1ps\nmodule big_alu #(\n\tparameter WIDTH = 32\n) (\n\tinput wire clk\n);\n",
			"import my_pkg::*;\nmodule top(input logic a);\n\tassign b = a;\nendmodule\n",
		] {
			assert_eq!(
				resolve(verilog_src),
				Some("clang"),
				"Verilog source:\n{verilog_src}"
			);
		}

		// Coq / Rocq sources -> declined (None)
		for coq_src in [
			"Theorem foo : 1 = 1. Proof. reflexivity. Qed.\n",
			"Require Import Coq.Arith.Arith.\nDefinition add1 (n : nat) := n + 1.\n",
			"Section Helpers.\nVariable x : nat.\nEnd Helpers.\n",
			"From Stdlib Require Import List.\nInductive tree := Leaf | Node.\n",
		] {
			assert_eq!(resolve(coq_src), None, "Coq source:\n{coq_src}");
		}

		// Inconclusive / empty -> fail closed (None)
		assert_eq!(resolve(""), None);
		assert_eq!(resolve("// just a comment\n"), None);
	}

	struct MockFmt {
		name: &'static str,
		exts: &'static [&'static str],
		langs: &'static [&'static str],
	}

	impl Formatter for MockFmt {
		fn name(&self) -> &'static str {
			self.name
		}
		fn extensions(&self) -> &'static [&'static str] {
			self.exts
		}
		fn linguist_languages(&self, _ext: &str) -> &'static [&'static str] {
			self.langs
		}
		fn format(&self, content: &str, _path: &Path) -> Result<String, String> {
			Ok(content.to_string())
		}
	}

	#[test]
	fn test_resolve_path_unique_vs_needs_content() {
		let reg = registry();
		for path in [
			"test.c",
			"test.cpp",
			"top.sv",
			"paper.tex",
			"main.rs",
			"app.ts",
		] {
			assert!(
				matches!(reg.resolve_path(Path::new(path)), ResolveOutcome::Unique(_, _)),
				"expected Unique for {path}"
			);
		}
		for path in ["top.v", "App.m", "article.cls"] {
			assert!(
				matches!(reg.resolve_path(Path::new(path)), ResolveOutcome::NeedsContent),
				"expected NeedsContent for {path}"
			);
		}
		assert!(matches!(
			reg.resolve_path(Path::new("unknown.xyz")),
			ResolveOutcome::None
		));
	}

	#[test]
	fn test_linguist_disambiguation_v_files() {
		let reg = registry();
		let resolve = |path: &str, content: &str| {
			reg.resolve_with_content(Path::new(path), content)
				.map(|(_, fmt)| fmt.name())
		};

		// Authentic Verilog accepted
		assert_eq!(
			resolve(
				"top.v",
				"module top(input wire a, output wire b);\nassign b = a;\nendmodule\n"
			),
			Some("clang")
		);
		assert_eq!(
			resolve("tb.v", "`timescale 1ns/1ps\nmodule tb;\nendmodule\n"),
			Some("clang")
		);

		// Coq / Rocq Prover declined
		assert_eq!(
			resolve(
				"proof.v",
				"Require Import Arith.\nTheorem foo : 1 = 1.\nProof. reflexivity. Qed.\n"
			),
			None
		);

		// V language routed to `v` formatter
		assert_eq!(
			resolve(
				"main.v",
				"module main\nfn main() {\n\tprintln('hi')\n}\n"
			),
			Some("v")
		);

		// Inconclusive .v fails closed
		assert_eq!(resolve("ambig.v", "// just a comment\nx = 1;\n"), None);
		assert_eq!(resolve("empty.v", ""), None);

		// Modeline overrides respected (both accept and decline)
		assert_eq!(
			resolve("hint.v", "// vim: ft=verilog\n// inconclusive body\n"),
			Some("clang")
		);
		assert_eq!(
			resolve("hint.v", "// -*- mode: verilog -*-\n// inconclusive body\n"),
			Some("clang")
		);
		assert_eq!(
			resolve(
				"override_coq.v",
				"// vim: ft=coq\nmodule top(input wire a);\nendmodule\n"
			),
			None
		);
		assert_eq!(
			resolve(
				"override_v.v",
				"// vim: ft=vlang\nmodule top(input wire a);\nendmodule\n"
			),
			Some("v")
		);
	}

	#[test]
	fn test_linguist_disambiguation_m_files() {
		let reg = registry();
		let resolve = |path: &str, content: &str| {
			reg.resolve_with_content(Path::new(path), content)
				.map(|(_, fmt)| fmt.name())
		};

		// Authentic Objective-C accepted
		assert_eq!(
			resolve(
				"App.m",
				"#import <Foundation/Foundation.h>\n@interface App : NSObject\n@end\n"
			),
			Some("clang")
		);

		// MATLAB declined
		assert_eq!(
			resolve(
				"calc.m",
				"% MATLAB script\nfunction y = calc(x)\n  y = x + 1;\nend\n"
			),
			None
		);

		// Inconclusive .m fails closed (plain C without Objective-C syntax)
		assert_eq!(resolve("plain.m", "int main() { return 0; }\n"), None);
		assert_eq!(resolve("empty.m", ""), None);

		// Modeline overrides respected (both accept and decline)
		assert_eq!(
			resolve("hint.m", "// vim: ft=objc\nint main() { return 0; }\n"),
			Some("clang")
		);
		assert_eq!(
			resolve(
				"hint.m",
				"// -*- mode: objc -*-\nint main() { return 0; }\n"
			),
			Some("clang")
		);
		assert_eq!(
			resolve(
				"override_matlab.m",
				"// vim: ft=matlab\n@interface App : NSObject\n@end\n"
			),
			None
		);
	}

	#[test]
	fn test_linguist_disambiguation_cls_files() {
		let reg = registry();
		let resolve = |path: &str, content: &str| {
			reg.resolve_with_content(Path::new(path), content)
				.map(|(_, fmt)| fmt.name())
		};

		assert_eq!(
			resolve(
				"article.cls",
				"\\NeedsTeXFormat{LaTeX2e}\n\\ProvidesClass{myclass}\n"
			),
			Some("latex")
		);
		assert_eq!(
			resolve(
				"AccountHandler.cls",
				"public class AccountHandler {\n}\n"
			),
			None
		);
		assert_eq!(
			resolve("Form1.cls", "VERSION 1.0 CLASS\nBEGIN\nEND\n"),
			None
		);
		assert_eq!(
			resolve("hint.cls", "% vim: ft=latex\n\\documentclass{article}\n"),
			Some("latex")
		);
	}

	#[test]
	fn test_shared_extension_routes_via_linguist_languages() {
		static VERILOG_FMT: MockFmt = MockFmt {
			name: "verilog",
			exts: &["v"],
			langs: &["Verilog", "SystemVerilog"],
		};
		static V_FMT: MockFmt = MockFmt {
			name: "vfmt",
			exts: &["v"],
			langs: &["V"],
		};

		let reg = Registry::from_formatters(vec![&VERILOG_FMT, &V_FMT]);
		assert!(reg.is_supported(Path::new("file.v")));
		assert!(matches!(
			reg.resolve_path(Path::new("file.v")),
			ResolveOutcome::NeedsContent
		));
		assert!(reg.resolve(Path::new("file.v")).is_none());

		let (_, picked_verilog) = reg
			.resolve_with_content(
				Path::new("top.v"),
				"module top(input wire a);\nendmodule\n",
			)
			.unwrap();
		assert_eq!(picked_verilog.name(), "verilog");

		let (_, picked_v) = reg
			.resolve_with_content(
				Path::new("main.v"),
				"module main\nfn main() {\n\tprintln('hi')\n}\n",
			)
			.unwrap();
		assert_eq!(picked_v.name(), "vfmt");

		// Modeline routes to the declared language
		let (_, picked_v_modeline) = reg
			.resolve_with_content(
				Path::new("hint.v"),
				"// vim: ft=vlang\n// inconclusive\n",
			)
			.unwrap();
		assert_eq!(picked_v_modeline.name(), "vfmt");

		let (_, picked_verilog_modeline) = reg
			.resolve_with_content(
				Path::new("hint.v"),
				"// vim: ft=verilog\n// inconclusive\n",
			)
			.unwrap();
		assert_eq!(picked_verilog_modeline.name(), "verilog");

		// Unclaimed language (Rocq Prover) and inconclusive content fail closed
		assert!(reg
			.resolve_with_content(
				Path::new("proof.v"),
				"Require Import Arith.\nProof. Qed.\n",
			)
			.is_none());
		assert!(reg
			.resolve_with_content(Path::new("ambig.v"), "// nothing decisive\n")
			.is_none());
	}

	#[test]
	fn test_disambiguation_tie_breaks_by_registration_order() {
		static FIRST: MockFmt = MockFmt {
			name: "first",
			exts: &["v"],
			langs: &["Verilog"],
		};
		static SECOND: MockFmt = MockFmt {
			name: "second",
			exts: &["v"],
			langs: &["Verilog"],
		};

		let reg = Registry::from_formatters(vec![&FIRST, &SECOND]);
		let (_, picked) = reg
			.resolve_with_content(
				Path::new("top.v"),
				"module top(input wire a);\nendmodule\n",
			)
			.unwrap();
		assert_eq!(picked.name(), "first");
	}

	#[test]
	fn test_single_formatter_content_guard_can_decline() {
		static GUARDED: MockFmt = MockFmt {
			name: "guarded",
			exts: &["v"],
			langs: &["Verilog"],
		};

		let reg = Registry::from_formatters(vec![&GUARDED]);
		assert!(matches!(
			reg.resolve_path(Path::new("top.v")),
			ResolveOutcome::NeedsContent
		));
		assert_eq!(
			reg.resolve_with_content(
				Path::new("top.v"),
				"module top(input wire a);\nendmodule\n"
			)
			.unwrap()
			.1
			.name(),
			"guarded"
		);
		assert!(reg
			.resolve_with_content(Path::new("x.v"), "decline me")
			.is_none());
	}

	#[test]
	#[should_panic(expected = "Duplicate extension")]
	fn test_same_formatter_duplicate_extension_panics() {
		static DUP: MockFmt = MockFmt {
			name: "dup",
			exts: &["dup", "dup"],
			langs: &[],
		};
		let _ = Registry::from_formatters(vec![&DUP]);
	}
}
