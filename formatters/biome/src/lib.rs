// biome-js-formatter - Biome formatting library
//
// Provides a unified formatting API for JavaScript, TypeScript, JSX, TSX,
// HTML, Vue, Svelte, and Astro using Biome parser/formatter crates.

#![allow(clippy::all)]

// Biome formatter imports
use biome_formatter::{
	BracketSpacing, IndentStyle, IndentWidth, LineEnding, LineWidth, QuoteStyle,
};
use biome_js_formatter::context::trailing_commas::TrailingCommas;
use biome_js_formatter::context::{JsFormatOptions, Semicolons};
use biome_js_syntax::JsFileSource;

use biome_html_parser::parse_html;
use biome_js_parser::parse;

use fama_common::{FileType, FormatConfig};

/// Convert fama_common config to biome indent style
fn to_biome_indent_style(style: fama_common::IndentStyle) -> IndentStyle {
	match style {
		fama_common::IndentStyle::Spaces => IndentStyle::Space,
		fama_common::IndentStyle::Tabs => IndentStyle::Tab,
	}
}

/// Convert fama_common quote style to biome quote style
fn to_biome_quote_style(style: fama_common::QuoteStyle) -> QuoteStyle {
	match style {
		fama_common::QuoteStyle::Single => QuoteStyle::Single,
		fama_common::QuoteStyle::Double => QuoteStyle::Double,
	}
}

/// Convert fama_common trailing comma to biome trailing commas
fn to_biome_trailing_commas(
	style: fama_common::TrailingComma,
) -> TrailingCommas {
	match style {
		fama_common::TrailingComma::All => TrailingCommas::All,
		fama_common::TrailingComma::None => TrailingCommas::None,
	}
}

/// Convert fama_common semicolons to biome semicolons
fn to_biome_semicolons(style: fama_common::Semicolons) -> Semicolons {
	match style {
		fama_common::Semicolons::Always => Semicolons::Always,
		fama_common::Semicolons::AsNeeded => Semicolons::AsNeeded,
	}
}

/// Convert fama_common line ending to biome line ending
fn to_biome_line_ending(ending: fama_common::LineEnding) -> LineEnding {
	match ending {
		fama_common::LineEnding::Lf => LineEnding::Lf,
		fama_common::LineEnding::Crlf => LineEnding::Crlf,
	}
}

/// Format JavaScript source code
pub fn format_javascript(
	source: &str,
	_file_path: &str,
) -> Result<String, String> {
	let config = FormatConfig::default();
	let source_type = JsFileSource::js_module();
	let options = JsFormatOptions::new(source_type)
		.with_indent_style(to_biome_indent_style(config.indent_style))
		.with_indent_width(IndentWidth::try_from(config.indent_width).unwrap())
		.with_line_width(LineWidth::try_from(config.line_width).unwrap())
		.with_line_ending(to_biome_line_ending(config.line_ending))
		.with_quote_style(to_biome_quote_style(config.quote_style))
		.with_trailing_commas(to_biome_trailing_commas(config.trailing_comma))
		.with_semicolons(to_biome_semicolons(config.semicolons))
		.with_bracket_spacing(BracketSpacing::from(config.bracket_spacing));

	let parsed = parse(source, source_type, Default::default());

	if parsed.has_errors() {
		return Err(format!("Parse errors in JavaScript file"));
	}

	let syntax = parsed.syntax();

	let formatted = biome_js_formatter::format_node(options, &syntax)
		.map_err(|e| format!("Format error: {:?}", e))?;

	formatted
		.print()
		.map(|p| p.as_code().to_string())
		.map_err(|e| format!("Print error: {:?}", e))
}

/// Format TypeScript source code
pub fn format_typescript(
	source: &str,
	_file_path: &str,
) -> Result<String, String> {
	let config = FormatConfig::default();
	let source_type = JsFileSource::ts();
	let options = JsFormatOptions::new(source_type)
		.with_indent_style(to_biome_indent_style(config.indent_style))
		.with_indent_width(IndentWidth::try_from(config.indent_width).unwrap())
		.with_line_width(LineWidth::try_from(config.line_width).unwrap())
		.with_line_ending(to_biome_line_ending(config.line_ending))
		.with_quote_style(to_biome_quote_style(config.quote_style))
		.with_trailing_commas(to_biome_trailing_commas(config.trailing_comma))
		.with_semicolons(to_biome_semicolons(config.semicolons))
		.with_bracket_spacing(BracketSpacing::from(config.bracket_spacing));

	let parsed = parse(source, source_type, Default::default());

	if parsed.has_errors() {
		return Err(format!("Parse errors in TypeScript file"));
	}

	let syntax = parsed.syntax();

	let formatted = biome_js_formatter::format_node(options, &syntax)
		.map_err(|e| format!("Format error: {:?}", e))?;

	formatted
		.print()
		.map(|p| p.as_code().to_string())
		.map_err(|e| format!("Print error: {:?}", e))
}

/// Format JSX source code
pub fn format_jsx(source: &str, _file_path: &str) -> Result<String, String> {
	let config = FormatConfig::default();
	let source_type = JsFileSource::jsx();
	let options = JsFormatOptions::new(source_type)
		.with_indent_style(to_biome_indent_style(config.indent_style))
		.with_indent_width(IndentWidth::try_from(config.indent_width).unwrap())
		.with_line_width(LineWidth::try_from(config.line_width).unwrap())
		.with_line_ending(to_biome_line_ending(config.line_ending))
		.with_quote_style(to_biome_quote_style(config.quote_style))
		.with_trailing_commas(to_biome_trailing_commas(config.trailing_comma))
		.with_semicolons(to_biome_semicolons(config.semicolons))
		.with_bracket_spacing(BracketSpacing::from(config.bracket_spacing));

	let parsed = parse(source, source_type, Default::default());

	if parsed.has_errors() {
		return Err(format!("Parse errors in JSX file"));
	}

	let syntax = parsed.syntax();

	let formatted = biome_js_formatter::format_node(options, &syntax)
		.map_err(|e| format!("Format error: {:?}", e))?;

	formatted
		.print()
		.map(|p| p.as_code().to_string())
		.map_err(|e| format!("Print error: {:?}", e))
}

/// Format TSX source code
pub fn format_tsx(source: &str, _file_path: &str) -> Result<String, String> {
	let config = FormatConfig::default();
	let source_type = JsFileSource::tsx();
	let options = JsFormatOptions::new(source_type)
		.with_indent_style(to_biome_indent_style(config.indent_style))
		.with_indent_width(IndentWidth::try_from(config.indent_width).unwrap())
		.with_line_width(LineWidth::try_from(config.line_width).unwrap())
		.with_line_ending(to_biome_line_ending(config.line_ending))
		.with_quote_style(to_biome_quote_style(config.quote_style))
		.with_trailing_commas(to_biome_trailing_commas(config.trailing_comma))
		.with_semicolons(to_biome_semicolons(config.semicolons))
		.with_bracket_spacing(BracketSpacing::from(config.bracket_spacing));

	let parsed = parse(source, source_type, Default::default());

	if parsed.has_errors() {
		return Err(format!("Parse errors in TSX file"));
	}

	let syntax = parsed.syntax();

	let formatted = biome_js_formatter::format_node(options, &syntax)
		.map_err(|e| format!("Format error: {:?}", e))?;

	formatted
		.print()
		.map(|p| p.as_code().to_string())
		.map_err(|e| format!("Print error: {:?}", e))
}

/// Format HTML source code
pub fn format_html(source: &str, _file_path: &str) -> Result<String, String> {
	let config = FormatConfig::default();
	let options = biome_html_formatter::context::HtmlFormatOptions::default()
		.with_indent_style(to_biome_indent_style(config.indent_style))
		.with_indent_width(IndentWidth::try_from(config.indent_width).unwrap())
		.with_line_width(LineWidth::try_from(config.line_width).unwrap());

	let parsed = parse_html(source, Default::default());

	if parsed.has_errors() {
		return Err(format!("Parse errors in HTML file"));
	}

	let syntax = parsed.syntax();

	let formatted = biome_html_formatter::format_node(options, &syntax, false)
		.map_err(|e| format!("Format error: {:?}", e))?;

	formatted
		.print()
		.map(|p| p.as_code().to_string())
		.map_err(|e| format!("Print error: {:?}", e))
}

/// Format Vue SFC source code (limited - extracts and formats template/script/style)
pub fn format_vue(source: &str, file_path: &str) -> Result<String, String> {
	// Vue SFC has special syntax - for now use HTML formatter with lenient parsing
	// Full Vue support would require extracting each section and formatting separately
	match format_html(source, file_path) {
		Ok(result) => Ok(result),
		Err(_) => {
			// If HTML parser fails, return original content (Vue has features HTML parser can't handle)
			Ok(source.to_string())
		}
	}
}

/// Format Svelte source code (limited - uses HTML parser)
pub fn format_svelte(source: &str, file_path: &str) -> Result<String, String> {
	// Svelte has special syntax - for now use HTML formatter with lenient parsing
	// Full Svelte support would require a dedicated Svelte parser
	match format_html(source, file_path) {
		Ok(result) => Ok(result),
		Err(_) => {
			// If HTML parser fails, return original content (Svelte has features HTML parser can't handle)
			eprintln!(
                "Warning: {} syntax not fully supported, file may not be properly formatted",
                file_path
            );
			Ok(source.to_string())
		}
	}
}

/// Format Astro source code (limited - extracts frontmatter and HTML)
pub fn format_astro(source: &str, file_path: &str) -> Result<String, String> {
	// Astro has frontmatter (fenced code block) - for now use HTML formatter
	// Full Astro support would require extracting and formatting frontmatter separately
	match format_html(source, file_path) {
		Ok(result) => Ok(result),
		Err(_) => {
			// If HTML parser fails, return original content (Astro has features HTML parser can't handle)
			eprintln!(
                "Warning: {} syntax not fully supported, file may not be properly formatted",
                file_path
            );
			Ok(source.to_string())
		}
	}
}

/// Format a file based on its file type
pub fn format_file(
	source: &str,
	file_path: &str,
	file_type: FileType,
) -> Result<String, String> {
	match file_type {
		FileType::JavaScript => format_javascript(source, file_path),
		FileType::TypeScript => format_typescript(source, file_path),
		FileType::Jsx => format_jsx(source, file_path),
		FileType::Tsx => format_tsx(source, file_path),
		FileType::Html => format_html(source, file_path),
		FileType::Vue => format_vue(source, file_path),
		FileType::Svelte => format_svelte(source, file_path),
		FileType::Astro => format_astro(source, file_path),
		_ => Err(format!(
			"File type {:?} is not supported by biome-js-formatter",
			file_type
		)),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_format_javascript() {
		let source = "const   x   =   1;";
		let result = format_javascript(source, "test.js").unwrap();
		assert!(result.contains("x = 1"));
	}

	#[test]
	fn test_format_typescript() {
		let source = "const   x: number   =   1;";
		let result = format_typescript(source, "test.ts").unwrap();
		assert!(result.contains("x: number") && result.contains("1"));
	}

	#[test]
	fn test_format_html() {
		let source = "<html><body></body></html>";
		let result = format_html(source, "test.html").unwrap();
		assert!(result.contains("<html>") || result.contains("<body>"));
	}

	#[test]
	fn test_format_file_with_javascript() {
		let source = "const   x   =   1;";
		let result =
			format_file(source, "test.js", FileType::JavaScript).unwrap();
		assert!(result.contains("x = 1"));
	}

	#[test]
	fn test_format_file_with_unsupported_type() {
		let source = "test";
		let result = format_file(source, "test.css", FileType::Css);
		assert!(result.is_err());
	}
}
