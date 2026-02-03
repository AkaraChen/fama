// biome-js-formatter - Biome formatting library
//
// Provides a unified formatting API for JavaScript, TypeScript, JSX, TSX,
// JSON, JSONC, HTML, Vue, Svelte, and Astro using Biome parser/formatter crates.
// Also provides import sorting via Biome's OrganizeImports analyzer rule.

#![allow(clippy::all)]

// Biome formatter imports
use biome_formatter::{
	BracketSpacing, IndentStyle, IndentWidth, LineEnding, LineWidth, QuoteStyle,
};
use biome_js_formatter::context::trailing_commas::TrailingCommas;
use biome_js_formatter::context::{JsFormatOptions, Semicolons};
use biome_js_syntax::{AnyJsRoot, JsFileSource};

use biome_graphql_parser::parse_graphql;
use biome_html_parser::parse_html;
use biome_js_parser::parse;
use biome_json_parser::parse_json;
use biome_json_syntax::JsonFileSource;

// Analyzer imports for import sorting
use biome_analyze::{
	ActionCategory, AnalysisFilter, AnalyzerOptions, ControlFlow,
	RuleCategoriesBuilder, SourceActionKind,
};
use biome_js_analyze::JsAnalyzerServices;
use biome_module_graph::ModuleGraph;
use biome_project_layout::ProjectLayout;
use biome_rowan::AstNode;
use std::sync::Arc;

use fama_common::{FileType, CONFIG};

// Module-level constants - pre-converted config values for optimal performance
const BIOME_INDENT_STYLE: IndentStyle = match CONFIG.indent_style {
	fama_common::IndentStyle::Spaces => IndentStyle::Space,
	fama_common::IndentStyle::Tabs => IndentStyle::Tab,
};
const BIOME_INDENT_WIDTH: u8 = CONFIG.indent_width;
const BIOME_LINE_WIDTH: u16 = CONFIG.line_width;
const BIOME_LINE_ENDING: LineEnding = match CONFIG.line_ending {
	fama_common::LineEnding::Lf => LineEnding::Lf,
	fama_common::LineEnding::Crlf => LineEnding::Crlf,
};
const BIOME_QUOTE_STYLE: QuoteStyle = match CONFIG.quote_style {
	fama_common::QuoteStyle::Single => QuoteStyle::Single,
	fama_common::QuoteStyle::Double => QuoteStyle::Double,
};
const BIOME_TRAILING_COMMAS: TrailingCommas = match CONFIG.trailing_comma {
	fama_common::TrailingComma::All => TrailingCommas::All,
	fama_common::TrailingComma::None => TrailingCommas::None,
};
const BIOME_SEMICOLONS: Semicolons = match CONFIG.semicolons {
	fama_common::Semicolons::Always => Semicolons::Always,
	fama_common::Semicolons::AsNeeded => Semicolons::AsNeeded,
};
const BIOME_BRACKET_SPACING: bool = CONFIG.bracket_spacing;

/// Sort imports in a JavaScript/TypeScript file using Biome's OrganizeImports analyzer rule.
///
/// This function runs the analyzer to detect unsorted imports and applies the
/// OrganizeImports code action to reorder them according to Biome's sorting rules:
/// 1. URLs (https://, http://)
/// 2. Packages with protocol (node:, bun:, jsr:, npm:)
/// 3. Bare packages (@scope/pkg, pkg)
/// 4. Aliases (#, @/, ~, $, %)
/// 5. Relative/absolute paths
fn sort_imports(
	root: &AnyJsRoot,
	source_type: JsFileSource,
	file_path: &str,
) -> AnyJsRoot {
	// Build a filter that enables the assist category (which includes organizeImports)
	let categories = RuleCategoriesBuilder::default().with_assist().build();

	let filter = AnalysisFilter {
		categories,
		..AnalysisFilter::default()
	};

	// Create analyzer options
	let options = AnalyzerOptions::default().with_file_path(file_path);

	// Create minimal services required by the analyzer
	let services = JsAnalyzerServices::from((
		Arc::new(ModuleGraph::default()),
		Arc::new(ProjectLayout::default()),
		source_type,
	));

	// Run the analyzer and collect OrganizeImports actions
	let mut result_root = root.clone();

	let _ = biome_js_analyze::analyze(
		root,
		filter,
		&options,
		&[], // No plugins
		services,
		|signal| {
			// Check if this signal has the organizeImports action
			for action in signal.actions() {
				if action.category
					== ActionCategory::Source(SourceActionKind::OrganizeImports)
				{
					// Apply the mutation to sort imports
					let new_syntax = action.mutation.commit();
					if let Some(new_root) = AnyJsRoot::cast(new_syntax) {
						result_root = new_root;
					}
				}
			}
			ControlFlow::<()>::Continue(())
		},
	);

	result_root
}

/// Format JavaScript source code
pub fn format_javascript(
	source: &str,
	file_path: &str,
) -> Result<String, String> {
	let source_type = JsFileSource::js_module();
	let options = JsFormatOptions::new(source_type)
		.with_indent_style(BIOME_INDENT_STYLE)
		.with_indent_width(IndentWidth::try_from(BIOME_INDENT_WIDTH).unwrap())
		.with_line_width(LineWidth::try_from(BIOME_LINE_WIDTH).unwrap())
		.with_line_ending(BIOME_LINE_ENDING)
		.with_quote_style(BIOME_QUOTE_STYLE)
		.with_trailing_commas(BIOME_TRAILING_COMMAS)
		.with_semicolons(BIOME_SEMICOLONS)
		.with_bracket_spacing(BracketSpacing::from(BIOME_BRACKET_SPACING));

	let parsed = parse(source, source_type, Default::default());

	if parsed.has_errors() {
		return Err(format!("Parse errors in JavaScript file"));
	}

	// Sort imports before formatting
	let root = parsed.tree();
	let sorted_root = sort_imports(&root, source_type, file_path);
	let syntax = sorted_root.syntax();

	let formatted = biome_js_formatter::format_node(options, syntax)
		.map_err(|e| format!("Format error: {:?}", e))?;

	formatted
		.print()
		.map(|p| p.as_code().to_string())
		.map_err(|e| format!("Print error: {:?}", e))
}

/// Format TypeScript source code
pub fn format_typescript(
	source: &str,
	file_path: &str,
) -> Result<String, String> {
	let source_type = JsFileSource::ts();
	let options = JsFormatOptions::new(source_type)
		.with_indent_style(BIOME_INDENT_STYLE)
		.with_indent_width(IndentWidth::try_from(BIOME_INDENT_WIDTH).unwrap())
		.with_line_width(LineWidth::try_from(BIOME_LINE_WIDTH).unwrap())
		.with_line_ending(BIOME_LINE_ENDING)
		.with_quote_style(BIOME_QUOTE_STYLE)
		.with_trailing_commas(BIOME_TRAILING_COMMAS)
		.with_semicolons(BIOME_SEMICOLONS)
		.with_bracket_spacing(BracketSpacing::from(BIOME_BRACKET_SPACING));

	let parsed = parse(source, source_type, Default::default());

	if parsed.has_errors() {
		return Err(format!("Parse errors in TypeScript file"));
	}

	// Sort imports before formatting
	let root = parsed.tree();
	let sorted_root = sort_imports(&root, source_type, file_path);
	let syntax = sorted_root.syntax();

	let formatted = biome_js_formatter::format_node(options, syntax)
		.map_err(|e| format!("Format error: {:?}", e))?;

	formatted
		.print()
		.map(|p| p.as_code().to_string())
		.map_err(|e| format!("Print error: {:?}", e))
}

/// Format JSX source code
pub fn format_jsx(source: &str, file_path: &str) -> Result<String, String> {
	let source_type = JsFileSource::jsx();
	let options = JsFormatOptions::new(source_type)
		.with_indent_style(BIOME_INDENT_STYLE)
		.with_indent_width(IndentWidth::try_from(BIOME_INDENT_WIDTH).unwrap())
		.with_line_width(LineWidth::try_from(BIOME_LINE_WIDTH).unwrap())
		.with_line_ending(BIOME_LINE_ENDING)
		.with_quote_style(BIOME_QUOTE_STYLE)
		.with_trailing_commas(BIOME_TRAILING_COMMAS)
		.with_semicolons(BIOME_SEMICOLONS)
		.with_bracket_spacing(BracketSpacing::from(BIOME_BRACKET_SPACING));

	let parsed = parse(source, source_type, Default::default());

	if parsed.has_errors() {
		return Err(format!("Parse errors in JSX file"));
	}

	// Sort imports before formatting
	let root = parsed.tree();
	let sorted_root = sort_imports(&root, source_type, file_path);
	let syntax = sorted_root.syntax();

	let formatted = biome_js_formatter::format_node(options, syntax)
		.map_err(|e| format!("Format error: {:?}", e))?;

	formatted
		.print()
		.map(|p| p.as_code().to_string())
		.map_err(|e| format!("Print error: {:?}", e))
}

/// Format TSX source code
pub fn format_tsx(source: &str, file_path: &str) -> Result<String, String> {
	let source_type = JsFileSource::tsx();
	let options = JsFormatOptions::new(source_type)
		.with_indent_style(BIOME_INDENT_STYLE)
		.with_indent_width(IndentWidth::try_from(BIOME_INDENT_WIDTH).unwrap())
		.with_line_width(LineWidth::try_from(BIOME_LINE_WIDTH).unwrap())
		.with_line_ending(BIOME_LINE_ENDING)
		.with_quote_style(BIOME_QUOTE_STYLE)
		.with_trailing_commas(BIOME_TRAILING_COMMAS)
		.with_semicolons(BIOME_SEMICOLONS)
		.with_bracket_spacing(BracketSpacing::from(BIOME_BRACKET_SPACING));

	let parsed = parse(source, source_type, Default::default());

	if parsed.has_errors() {
		return Err(format!("Parse errors in TSX file"));
	}

	// Sort imports before formatting
	let root = parsed.tree();
	let sorted_root = sort_imports(&root, source_type, file_path);
	let syntax = sorted_root.syntax();

	let formatted = biome_js_formatter::format_node(options, syntax)
		.map_err(|e| format!("Format error: {:?}", e))?;

	formatted
		.print()
		.map(|p| p.as_code().to_string())
		.map_err(|e| format!("Print error: {:?}", e))
}

/// Format JSON source code
pub fn format_json(source: &str, _file_path: &str) -> Result<String, String> {
	format_json_internal(source, JsonFileSource::json(), false)
}

/// Format JSONC (JSON with comments) source code
pub fn format_jsonc(source: &str, _file_path: &str) -> Result<String, String> {
	format_json_internal(
		source,
		JsonFileSource::json_allow_comments("jsonc"),
		true,
	)
}

/// Internal JSON formatting with configurable source type
fn format_json_internal(
	source: &str,
	source_type: JsonFileSource,
	allow_comments: bool,
) -> Result<String, String> {
	use biome_json_parser::JsonParserOptions;

	let options =
		biome_json_formatter::context::JsonFormatOptions::new(source_type)
			.with_indent_style(BIOME_INDENT_STYLE)
			.with_indent_width(
				IndentWidth::try_from(BIOME_INDENT_WIDTH).unwrap(),
			)
			.with_line_width(LineWidth::try_from(BIOME_LINE_WIDTH).unwrap())
			.with_line_ending(BIOME_LINE_ENDING);

	let parser_options = if allow_comments {
		JsonParserOptions::default().with_allow_comments()
	} else {
		JsonParserOptions::default()
	};

	let parsed = parse_json(source, parser_options);

	if parsed.has_errors() {
		return Err("Parse errors in JSON file".to_string());
	}

	let syntax = parsed.syntax();

	let formatted = biome_json_formatter::format_node(options, &syntax)
		.map_err(|e| format!("Format error: {:?}", e))?;

	formatted
		.print()
		.map(|p| p.as_code().to_string())
		.map_err(|e| format!("Print error: {:?}", e))
}

/// Format HTML source code
pub fn format_html(source: &str, _file_path: &str) -> Result<String, String> {
	let options = biome_html_formatter::context::HtmlFormatOptions::default()
		.with_indent_style(BIOME_INDENT_STYLE)
		.with_indent_width(IndentWidth::try_from(BIOME_INDENT_WIDTH).unwrap())
		.with_line_width(LineWidth::try_from(BIOME_LINE_WIDTH).unwrap());

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

/// Format GraphQL source code
pub fn format_graphql(
	source: &str,
	_file_path: &str,
) -> Result<String, String> {
	let options =
		biome_graphql_formatter::context::GraphqlFormatOptions::default()
			.with_indent_style(BIOME_INDENT_STYLE)
			.with_indent_width(
				IndentWidth::try_from(BIOME_INDENT_WIDTH).unwrap(),
			)
			.with_line_width(LineWidth::try_from(BIOME_LINE_WIDTH).unwrap())
			.with_line_ending(BIOME_LINE_ENDING);

	let parsed = parse_graphql(source);

	if parsed.has_errors() {
		return Err(format!("Parse errors in GraphQL file"));
	}

	let syntax = parsed.syntax();

	let formatted = biome_graphql_formatter::format_node(options, &syntax)
		.map_err(|e| format!("Format error: {:?}", e))?;

	formatted
		.print()
		.map(|p| p.as_code().to_string())
		.map_err(|e| format!("Print error: {:?}", e))
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
		FileType::Json => {
			// Try standard JSON first, if that fails try JSON with comments
			match format_json(source, file_path) {
				Ok(result) => Ok(result),
				Err(_) => format_jsonc(source, file_path),
			}
		}
		FileType::Jsonc => format_jsonc(source, file_path),
		FileType::Html => format_html(source, file_path),
		FileType::Vue => format_vue(source, file_path),
		FileType::Svelte => format_svelte(source, file_path),
		FileType::Astro => format_astro(source, file_path),
		FileType::GraphQL => format_graphql(source, file_path),
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

	#[test]
	fn test_format_json() {
		let source = r#"{"name":"test","value":1}"#;
		let result = format_json(source, "test.json").unwrap();
		// JSON should be formatted with proper indentation
		assert!(result.contains("\"name\""));
		assert!(result.contains("\"test\""));
		// Biome keeps compact JSON on single line, adding trailing newline
		assert!(result.ends_with('\n'), "JSON should end with newline");
	}

	#[test]
	fn test_format_file_with_json() {
		let source = r#"{"key":"value"}"#;
		let result = format_file(source, "test.json", FileType::Json).unwrap();
		assert!(result.contains("\"key\""));
	}

	#[test]
	fn test_format_json_with_comments_fallback() {
		// JSON file with comments should fallback to JSONC mode
		let source = r#"{
  // This is a comment
  "name": "test",
  "value": 1
}"#;
		let result = format_file(source, "test.json", FileType::Json).unwrap();
		assert!(result.contains("\"name\""));
		assert!(result.contains("// This is a comment"));
	}

	#[test]
	fn test_sort_imports_javascript() {
		// Imports in wrong order: relative paths should come after packages
		let source = r#"import z from "./local";
import a from "package-a";
import b from "package-b";
"#;
		let result = format_javascript(source, "test.js").unwrap();
		// After sorting: packages should come before relative paths
		// package-a and package-b should come before ./local
		let a_pos = result.find("package-a").unwrap();
		let b_pos = result.find("package-b").unwrap();
		let local_pos = result.find("./local").unwrap();
		assert!(
			a_pos < local_pos && b_pos < local_pos,
			"Package imports should come before relative imports. Got: {}",
			result
		);
	}

	#[test]
	fn test_sort_imports_typescript() {
		// Imports in wrong order with mixed types
		let source = r#"import { Component } from "./Component";
import type { Props } from "./types";
import React from "react";
import path from "node:path";
"#;
		let result = format_typescript(source, "test.ts").unwrap();
		// After sorting: node: should come first, then packages, then relative
		let node_pos = result.find("node:path").unwrap();
		let react_pos = result.find("react").unwrap();
		let component_pos = result.find("./Component").unwrap();
		assert!(
			node_pos < react_pos,
			"node: imports should come before package imports. Got: {}",
			result
		);
		assert!(
			react_pos < component_pos,
			"Package imports should come before relative imports. Got: {}",
			result
		);
	}

	#[test]
	fn test_sort_named_specifiers() {
		// Named specifiers should be sorted alphabetically (case-insensitive, then uppercase first)
		let source = r#"import { z, a, m, B } from "package";
"#;
		let result = format_javascript(source, "test.js").unwrap();
		// Biome sorts case-insensitively: a < B (because 'a' < 'b') < m < z
		// Within same letter, uppercase comes first (A < a < B < b)
		let a_pos = result.find(" a,").unwrap();
		let b_pos = result.find(" B,").unwrap();
		let m_pos = result.find(" m,").unwrap();
		let z_pos = result.find(" z ").unwrap();
		assert!(
			a_pos < b_pos && b_pos < m_pos && m_pos < z_pos,
			"Named specifiers should be sorted (a, B, m, z). Got: {}",
			result
		);
	}

	#[test]
	fn test_sort_imports_with_side_effects() {
		// Side-effect imports should not be reordered with regular imports
		let source = r#"import "./polyfill";
import a from "package-a";
"#;
		let result = format_javascript(source, "test.js").unwrap();
		// Side-effect import should stay at top (it forms its own chunk)
		let polyfill_pos = result.find("polyfill").unwrap();
		let a_pos = result.find("package-a").unwrap();
		assert!(
			polyfill_pos < a_pos,
			"Side-effect imports should maintain their position. Got: {}",
			result
		);
	}
}
