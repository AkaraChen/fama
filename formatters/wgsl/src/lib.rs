//! WGSL formatter via the wgsl-analyzer `wgsl-formatter` crate
//!
//! The formatter core is a pure-Rust, dprint-core-based pretty printer that
//! parses WGSL into a rowan syntax tree and regenerates it. Formatting runs
//! fully in-process with no WASM runtime or subprocess; see README.md for
//! provenance and for why the upstream `wasm-fmt/wgslfmt` WASM wrapper was
//! not used.

use fama_common::{FileType, IndentStyle, LineEnding, CONFIG};
use wgsl_formatter::{
	AttributeFormattingStyle, FormatDocumentError, FormattingOptions,
	IndentStyle as WgslIndentStyle, LineBreakStyle,
};
use wgsl_syntax::Edition;

/// Build the upstream formatter options from fama's central config.
///
/// `binding_group_style` is `Inline` so `@group(...) @binding(...)` stays on
/// the same line as its `var` declaration, matching the style used by the
/// WebGPU spec and wgpu examples.
fn options() -> FormattingOptions {
	FormattingOptions {
		max_line_width: u32::from(CONFIG.line_width),
		indent_width: CONFIG.indent_width,
		indent_style: match CONFIG.indent_style {
			IndentStyle::Tabs => WgslIndentStyle::Tabs,
			IndentStyle::Spaces => WgslIndentStyle::Spaces,
		},
		line_break_style: match CONFIG.line_ending {
			LineEnding::Lf => LineBreakStyle::LineFeed,
			LineEnding::Crlf => LineBreakStyle::CarriageReturnLineFeed,
		},
		binding_group_style: AttributeFormattingStyle::Inline,
	}
}

/// Format a WGSL source string.
///
/// Rejects inputs with syntax errors (the upstream formatter refuses to
/// format error trees rather than risking lossy rewrites) and returns the
/// regenerated source with exactly one trailing newline.
pub fn format_wgsl(source: &str, _path: &str) -> Result<String, String> {
	let parse = wgsl_syntax::parse(source, Edition::LATEST);
	if let Some(first) = parse.errors().first() {
		let extra = parse.errors().len().saturating_sub(1);
		let suffix = if extra > 0 {
			format!(" (+{extra} more)")
		} else {
			String::new()
		};
		return Err(format!("{first}{suffix}"));
	}

	let formatted = wgsl_formatter::format_node(&parse.syntax(), &options())
		.map_err(format_document_error)?;

	// `format_node` regenerates the file without its surrounding blank
	// space; restore a single trailing newline, and format empty files as
	// empty.
	if formatted.trim().is_empty() {
		return Ok(String::new());
	}
	let line_end = match CONFIG.line_ending {
		LineEnding::Lf => "\n",
		LineEnding::Crlf => "\r\n",
	};
	Ok(format!(
		"{}{}",
		formatted.trim_end_matches(['\r', '\n']),
		line_end
	))
}

fn format_document_error(error: FormatDocumentError) -> String {
	format!("WGSL formatting failed: {error}")
}

/// Format a file based on its file type
pub fn format_file(
	content: &str,
	path: &str,
	file_type: FileType,
) -> Result<String, String> {
	match file_type {
		FileType::Wgsl => format_wgsl(content, path),
		_ => Err(format!(
			"File type {:?} is not supported by the wgsl formatter",
			file_type
		)),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn formats_messy_shader() {
		let input = r#"struct VertexInput{
@location(0) position:vec4f,
	@location(1) uv:vec2f,   }

@group(0) @binding(0)
var<uniform> camera   :mat4x4f   ;

@vertex fn vs(@location(0) p:vec4f)->@builtin(position) vec4f{
return camera*vec4f(p.xyz,1.0);   // pass through
}
"#;
		let formatted =
			format_wgsl(input, "test.wgsl").expect("valid WGSL should format");

		// Golden output: tabs (per CONFIG), inline binding group, LF,
		// single trailing newline, comments preserved.
		assert_eq!(
			formatted,
			"struct VertexInput {
\t@location(0) position: vec4f,
\t@location(1) uv: vec2f,
}

@group(0) @binding(0) var<uniform> camera: mat4x4f;

@vertex
fn vs(@location(0) p: vec4f) -> @builtin(position) vec4f {
\treturn camera * vec4f(p.xyz, 1.0); // pass through
}
"
		);
	}

	#[test]
	fn formatting_is_idempotent() {
		let input =
			"fn f(x:f32)->f32{if x<0.0{return 0.0;}else if x>1.0{return 1.0;}\
loop{continuing{break if x>10.0;}}\
for(var i:i32=0;i<4;i=i+1){x=x+f32(i);}\
return x;}";
		let once = format_wgsl(input, "test.wgsl").unwrap_or_else(|e| {
			panic!("idempotency input must be valid WGSL: {e}")
		});
		let twice =
			format_wgsl(&once, "test.wgsl").expect("output stays valid WGSL");
		assert_eq!(once, twice, "formatting must be idempotent");
		assert!(once.contains("\treturn x;"), "output: {once}");
	}

	#[test]
	fn syntax_error_returns_err() {
		let result = format_wgsl("fn broken(:) {", "test.wgsl");
		assert!(result.is_err(), "got Ok({:?})", result.unwrap());
		let message = result.unwrap_err();
		assert!(
			message.contains("error at"),
			"error should carry the parser diagnostic: {message}"
		);
	}

	#[test]
	fn dispatch_rejects_other_file_types() {
		assert!(
			format_file("fn f() {}", "test.js", FileType::JavaScript).is_err()
		);
		assert!(format_file("fn f() {}", "test.wgsl", FileType::Wgsl).is_ok());
	}

	#[test]
	fn empty_input_formats_to_empty() {
		assert_eq!(format_wgsl("", "test.wgsl").unwrap(), "");
		assert_eq!(format_wgsl("\n\n  \n", "test.wgsl").unwrap(), "");
	}

	/// The benchmark fixtures must be valid, formatted WGSL (they are only
	/// formatted again by the ignored bench, so check them here).
	#[test]
	fn bench_fixtures_are_valid() {
		for fixture in [
			include_str!("../benches/small.wgsl"),
			include_str!("../benches/repeat_block.wgsl"),
		] {
			let formatted = format_wgsl(fixture, "fixture.wgsl")
				.expect("fixture should format");
			let twice = format_wgsl(&formatted, "fixture.wgsl")
				.expect("fixture output stays valid");
			assert_eq!(formatted, twice, "fixture output is idempotent");
		}
	}

	#[test]
	fn options_follow_fama_config() {
		let options = options();
		assert_eq!(
			options.indent_style,
			WgslIndentStyle::Tabs,
			"fama indents with tabs"
		);
		assert_eq!(options.indent_width, 4);
		assert_eq!(options.max_line_width, 80);
		assert_eq!(options.line_break_style, LineBreakStyle::LineFeed);
	}

	/// Per-file latency over 100 iterations for a small (~50 line) and a
	/// large (~1000 line) shader. Run with:
	/// `cargo test -p fama-wgsl --release -- --ignored --nocapture`
	#[test]
	#[ignore]
	fn bench_per_file_latency() {
		use std::time::Instant;

		let small = include_str!("../benches/small.wgsl");

		let block = include_str!("../benches/repeat_block.wgsl");
		let mut large = String::new();
		for i in 0..25 {
			large.push_str(
				&block.replace("SAMPLE", &format!("{i}")).to_lowercase(),
			);
		}

		for (label, source) in [
			("small (~50 lines)", small),
			("large (~1000 lines)", &large),
		] {
			// Warm-up, then time 100 formatting calls on this thread.
			format_wgsl(source, "bench.wgsl").unwrap();
			let start = Instant::now();
			for _ in 0..100 {
				format_wgsl(source, "bench.wgsl").unwrap();
			}
			let total = start.elapsed();
			println!(
				"{label}: {:>8.1} us/file ({:.1} MB/s)",
				total.as_micros() as f64 / 100.0,
				source.len() as f64
					/ (total.as_secs_f64() / 100.0)
					/ (1024.0 * 1024.0)
			);
		}
	}
}
