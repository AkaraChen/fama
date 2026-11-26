//! WGSL formatter via the wgsl-analyzer `wgsl-formatter` crate.

use fama_common::{Formatter, IndentStyle, LineEnding, CONFIG};
use std::path::Path;
use wgsl_formatter::{
	AttributeFormattingStyle, FormatDocumentError, FormattingOptions,
	IndentStyle as WgslIndentStyle, LineBreakStyle,
};
use wgsl_syntax::Edition;

/// Formatter implementation for WGSL shaders.
pub struct WgslFormatter;

impl Formatter for WgslFormatter {
	fn name(&self) -> &'static str {
		"wgsl"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["wgsl"]
	}

	fn format(&self, content: &str, path: &Path) -> Result<String, String> {
		format_wgsl(content, path.to_str().unwrap_or(""))
	}
}

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
	fn empty_input_formats_to_empty() {
		assert_eq!(format_wgsl("", "test.wgsl").unwrap(), "");
		assert_eq!(format_wgsl("\n\n  \n", "test.wgsl").unwrap(), "");
	}

	#[test]
	fn bench_fixtures_are_valid() {
		for fixture in [
			include_str!("../benches/wgsl/small.wgsl"),
			include_str!("../benches/wgsl/repeat_block.wgsl"),
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

	#[test]
	#[ignore]
	fn bench_per_file_latency() {
		use std::time::Instant;

		let small = include_str!("../benches/wgsl/small.wgsl");

		let block = include_str!("../benches/wgsl/repeat_block.wgsl");
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
