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
						if !fmt.needs_content_check(ext)
							|| fmt.match_confidence(path, content) > 0
						{
							Some((*idx, fmt))
						} else {
							None
						}
					}
					ExtSlot::Ambiguous(indices) => {
						let mut best: Option<(
							u8,
							usize,
							&'static dyn Formatter,
						)> = None;
						for &idx in indices {
							let fmt = self.formatters[idx];
							let score = fmt.match_confidence(path, content);
							if score > 0
								&& best.is_none_or(|(best_score, _, _)| {
									score > best_score
								}) {
								best = Some((score, idx, fmt));
							}
						}
						best.map(|(_, idx, fmt)| (idx, fmt))
					}
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
		assert_eq!(reg.all().len(), 41);
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
			"top.v",
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

	struct MockFmt {
		name: &'static str,
		exts: &'static [&'static str],
		needs_check: bool,
		marker: &'static str,
		score: u8,
	}

	impl Formatter for MockFmt {
		fn name(&self) -> &'static str {
			self.name
		}
		fn extensions(&self) -> &'static [&'static str] {
			self.exts
		}
		fn needs_content_check(&self, _ext: &str) -> bool {
			self.needs_check
		}
		fn match_confidence(&self, _path: &Path, content: &str) -> u8 {
			if self.marker.is_empty() || content.contains(self.marker) {
				self.score
			} else {
				0
			}
		}
		fn format(&self, content: &str, _path: &Path) -> Result<String, String> {
			Ok(content.to_string())
		}
	}

	#[test]
	fn test_disambiguation_highest_score_and_decline() {
		static FMT_A: MockFmt = MockFmt {
			name: "alpha",
			exts: &["amb"],
			needs_check: false,
			marker: "ALPHA",
			score: 70,
		};
		static FMT_B: MockFmt = MockFmt {
			name: "beta",
			exts: &["amb"],
			needs_check: false,
			marker: "BETA",
			score: 90,
		};

		let reg = Registry::from_formatters(vec![&FMT_A, &FMT_B]);
		assert!(reg.is_supported(Path::new("file.amb")));
		assert!(matches!(
			reg.resolve_path(Path::new("file.amb")),
			ResolveOutcome::NeedsContent
		));
		assert!(reg.resolve(Path::new("file.amb")).is_none());

		let (_, picked_a) = reg
			.resolve_with_content(Path::new("file.amb"), "ALPHA code")
			.unwrap();
		assert_eq!(picked_a.name(), "alpha");

		let (_, picked_b) = reg
			.resolve_with_content(Path::new("file.amb"), "BETA code")
			.unwrap();
		assert_eq!(picked_b.name(), "beta");

		// Both match -> higher score wins
		let (_, picked_both) = reg
			.resolve_with_content(Path::new("file.amb"), "ALPHA and BETA")
			.unwrap();
		assert_eq!(picked_both.name(), "beta");

		// Both decline -> None
		assert!(reg
			.resolve_with_content(Path::new("file.amb"), "unrelated")
			.is_none());
	}

	#[test]
	fn test_disambiguation_tie_breaks_by_registration_order() {
		static FIRST: MockFmt = MockFmt {
			name: "first",
			exts: &["tie"],
			needs_check: false,
			marker: "",
			score: 50,
		};
		static SECOND: MockFmt = MockFmt {
			name: "second",
			exts: &["tie"],
			needs_check: false,
			marker: "",
			score: 50,
		};

		let reg = Registry::from_formatters(vec![&FIRST, &SECOND]);
		let (_, picked) =
			reg.resolve_with_content(Path::new("a.tie"), "").unwrap();
		assert_eq!(picked.name(), "first");
	}

	#[test]
	fn test_single_formatter_content_guard_can_decline() {
		static GUARDED: MockFmt = MockFmt {
			name: "guarded",
			exts: &["grd"],
			needs_check: true,
			marker: "ACCEPT",
			score: 80,
		};

		let reg = Registry::from_formatters(vec![&GUARDED]);
		assert!(matches!(
			reg.resolve_path(Path::new("x.grd")),
			ResolveOutcome::NeedsContent
		));
		assert_eq!(
			reg.resolve_with_content(Path::new("x.grd"), "ACCEPT")
				.unwrap()
				.1
				.name(),
			"guarded"
		);
		assert!(reg
			.resolve_with_content(Path::new("x.grd"), "decline me")
			.is_none());
	}

	#[test]
	#[should_panic(expected = "Duplicate extension")]
	fn test_same_formatter_duplicate_extension_panics() {
		static DUP: MockFmt = MockFmt {
			name: "dup",
			exts: &["dup", "dup"],
			needs_check: false,
			marker: "",
			score: 50,
		};
		let _ = Registry::from_formatters(vec![&DUP]);
	}
}
