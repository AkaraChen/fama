//! Compatibility layer for hosting official dprint WASM Plugin Schema v4
//! modules inside `fama` via `wasmtime` (`fama-wasm`).

pub mod host;
pub mod wasi;

use fama_common::Formatter;
use std::path::{Path, PathBuf};

pub use host::{DprintWasmEngine, DprintWasmSource, PluginInfo};

/// `fama_common::Formatter` adapter backed by any dprint Schema v4 WASM plugin.
pub struct DprintWasmFormatter {
	name: &'static str,
	extensions: &'static [&'static str],
	engine: DprintWasmEngine,
	precompile_embedded: bool,
}

impl DprintWasmFormatter {
	pub const fn new(
		name: &'static str,
		extensions: &'static [&'static str],
		prefix: &'static str,
		source: DprintWasmSource,
	) -> Self {
		let precompile_embedded = !matches!(source, DprintWasmSource::FilePath(_));
		Self {
			name,
			extensions,
			engine: DprintWasmEngine::new(prefix, source),
			precompile_embedded,
		}
	}

	pub fn plugin_info(&self) -> Result<PluginInfo, String> {
		self.engine.plugin_info()
	}
}

impl Formatter for DprintWasmFormatter {
	fn name(&self) -> &'static str {
		self.name
	}

	fn extensions(&self) -> &'static [&'static str] {
		self.extensions
	}

	fn format(&self, content: &str, path: &Path) -> Result<String, String> {
		self.engine.format(content, path)
	}

	fn precompile(&self, output_dir: &Path) -> Option<Result<PathBuf, String>> {
		if self.precompile_embedded {
			Some(self.engine.precompile(output_dir))
		} else {
			None
		}
	}
}

const CSHARPIER_WASM_ZST: &[u8] = include_bytes!("../wasm/csharpier.wasm.zst");
const ROSLYN_WASM_ZST: &[u8] = include_bytes!("../wasm/roslyn.wasm.zst");
const ZIG_WASM_ZST: &[u8] = include_bytes!("../wasm/zig.wasm.zst");

/// C# formatter powered by official `dprint-plugin-csharpier` WASM plugin.
pub static CSHARPIER: DprintWasmFormatter = DprintWasmFormatter::new(
	"csharpier",
	&["cs", "csx"],
	"dprint-csharpier",
	DprintWasmSource::EmbeddedZstd(CSHARPIER_WASM_ZST),
);

/// VB.NET formatter powered by official `dprint-plugin-roslyn` WASM plugin.
pub static ROSLYN: DprintWasmFormatter = DprintWasmFormatter::new(
	"roslyn",
	&["vb"],
	"dprint-roslyn",
	DprintWasmSource::EmbeddedZstd(ROSLYN_WASM_ZST),
);

/// Zig formatter powered by official `dprint-plugin-zig` WASM plugin.
pub static ZIG: DprintWasmFormatter = DprintWasmFormatter::new(
	"zig",
	&["zig", "zon"],
	"dprint-zig",
	DprintWasmSource::EmbeddedZstd(ZIG_WASM_ZST),
);

/// Enabled dprint-WASM-backed formatters registered by `fama`.
pub const FORMATTERS: &[&dyn Formatter] = &[&CSHARPIER, &ROSLYN, &ZIG];

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::Map;

	#[test]
	fn test_missing_plugin_path_errors_cleanly() {
		let fmt = DprintWasmFormatter::new(
			"missing",
			&["xyz"],
			"missing",
			DprintWasmSource::FilePath("/nonexistent/dprint-plugin.wasm"),
		);
		let err = fmt.format("hello", Path::new("a.xyz")).unwrap_err();
		assert!(err.contains("Failed to"), "{err}");
	}

	#[test]
	fn test_config_serialization_shape() {
		let mut map = Map::new();
		map.insert("preferSingleLine".to_string(), serde_json::Value::Bool(true));
		let engine = DprintWasmEngine::with_plugin_config(
			"test",
			DprintWasmSource::FilePath("/nonexistent.wasm"),
			map,
		);
		assert!(engine.plugin_info().is_err());
	}

	#[test]
	fn test_format_csharp_via_csharpier() {
		let messy = "namespace App{public class Greeter{public static string Hello(string name){return $\"Hi, {name}!\";}}}";
		let once = CSHARPIER.format(messy, Path::new("Program.cs")).unwrap();
		assert!(once.contains("public class Greeter"), "{once}");
		assert!(once.contains('\n'), "{once}");
		let twice = CSHARPIER.format(&once, Path::new("Program.cs")).unwrap();
		assert_eq!(once, twice);

		assert!(CSHARPIER
			.format("public class {{{", Path::new("Bad.cs"))
			.is_err());
	}

	#[test]
	fn test_format_vbnet_via_roslyn() {
		let messy = "Module Module1\nSub Main()\nConsole.WriteLine(\"Hi\")\nEnd Sub\nEnd Module\n";
		let once = ROSLYN.format(messy, Path::new("Module1.vb")).unwrap();
		assert!(once.contains("Sub Main()"), "{once}");
		assert!(once.contains("Console.WriteLine"), "{once}");
		let twice = ROSLYN.format(&once, Path::new("Module1.vb")).unwrap();
		assert_eq!(once, twice);
	}

	#[test]
	fn test_format_zig_via_dprint() {
		let simple = ZIG.format("const x=1;", Path::new("test.zig")).unwrap();
		assert_eq!(simple, "const x = 1;\n");

		let func = ZIG
			.format("fn foo()void{return;}", Path::new("test.zig"))
			.unwrap();
		assert!(func.contains("fn foo() void"), "{func}");
		assert_eq!(ZIG.format(&func, Path::new("test.zig")).unwrap(), func);

		let already = "const x = 1;\n";
		assert_eq!(ZIG.format(already, Path::new("test.zig")).unwrap(), already);

		assert!(ZIG.format("const x = ;", Path::new("bad.zig")).is_err());

		let zon = ZIG
			.format(
				".{.name=\"app\",.version=\"0.1.0\",}",
				Path::new("build.zig.zon"),
			)
			.unwrap();
		assert!(zon.contains(".name = \"app\""), "{zon}");
		assert_eq!(ZIG.format(&zon, Path::new("build.zig.zon")).unwrap(), zon);
	}
}
