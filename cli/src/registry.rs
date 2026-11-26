//! Central formatter registry built from all enabled language formatters.

use fama_common::Formatter;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

/// Registry mapping file extensions and special filenames to `Formatter` instances.
pub struct Registry {
	formatters: Vec<&'static dyn Formatter>,
	by_ext: HashMap<&'static str, usize>,
}

impl Registry {
	fn new() -> Self {
		let mut formatters: Vec<&'static dyn Formatter> =
			Vec::with_capacity(fama_builtin::FORMATTERS.len() + 7);
		formatters.extend_from_slice(fama_builtin::FORMATTERS);
		formatters.extend_from_slice(&[
			&fama_clang::FORMATTER,
			&fama_dart::FORMATTER,
			&fama_swift::FORMATTER,
			&goffi::FORMATTER,
			&zigffi::FORMATTER,
			&ruby_fmt::FORMATTER,
			&fama_kotlin_worker::FORMATTER,
		]);

		let mut by_ext = HashMap::new();
		for (idx, fmt) in formatters.iter().enumerate() {
			for &ext in fmt.extensions() {
				let prev = by_ext.insert(ext, idx);
				assert!(
					prev.is_none(),
					"Duplicate extension .{ext} registered by {}",
					fmt.name()
				);
			}
		}

		Self { formatters, by_ext }
	}

	/// Resolve the formatter `(index, formatter)` responsible for `path`.
	pub fn resolve(&self, path: &Path) -> Option<(usize, &'static dyn Formatter)> {
		if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
			if let Some(&idx) = self.by_ext.get(ext) {
				return Some((idx, self.formatters[idx]));
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

	/// Return `true` if any registered formatter handles `path`.
	pub fn is_supported(&self, path: &Path) -> bool {
		self.resolve(path).is_some()
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

#[cfg(test)]
mod tests {
	use super::*;

	fn resolve_name(path: &str) -> Option<&'static str> {
		registry().resolve(Path::new(path)).map(|(_, fmt)| fmt.name())
	}

	#[test]
	fn test_registry_has_no_collisions_and_all_formatters() {
		let reg = registry();
		assert_eq!(reg.all().len(), 23);
	}

	#[test]
	fn test_resolve_web_and_data_languages() {
		for path in [
			"test.js",
			"test.cjs",
			"test.mjs",
			"test.ts",
			"test.mts",
			"test.jsx",
			"test.mjsx",
			"test.tsx",
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
		assert_eq!(resolve_name("test.dart"), Some("dart"));
		assert_eq!(resolve_name("Sources/App/Main.swift"), Some("swift"));
		assert_eq!(resolve_name("test.kt"), Some("kotlin"));
		assert_eq!(resolve_name("build.gradle.kts"), Some("kotlin"));
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
			"test.cs",
			"test.m",
			"test.mm",
			"Main.java",
			"msg.proto",
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
}
