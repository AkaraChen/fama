// biome-js-formatter - Biome formatting module
//
// Provides a unified formatting API for JavaScript, TypeScript, JSX, TSX,
// JSON, JSONC, HTML, Vue, Svelte, Astro, and GraphQL using Biome crates.
// Also provides import sorting via Biome's OrganizeImports analyzer rule.

use biome_formatter::{
	BracketSpacing, IndentStyle, IndentWidth, LineEnding, LineWidth, QuoteStyle,
};
use biome_js_formatter::context::trailing_commas::TrailingCommas;
use biome_js_formatter::context::{JsFormatOptions, Semicolons};
use biome_js_syntax::{AnyJsRoot, JsFileSource};

use biome_graphql_parser::parse_graphql;
use biome_html_parser::{parse_html, HtmlParseOptions};
use biome_js_parser::{parse, JsParserOptions};
use biome_json_parser::parse_json;
use biome_json_syntax::JsonFileSource;

use biome_analyze::{
	ActionCategory, AnalysisFilter, AnalyzerOptions, ControlFlow,
	RuleCategoriesBuilder, RuleFilter, RuleMeta, SourceActionKind,
};
use biome_js_analyze::assist::source::organize_imports::OrganizeImports;
use biome_js_analyze::JsAnalyzerServices;
use biome_module_graph::ModuleGraph;
use biome_project_layout::ProjectLayout;
use biome_rowan::AstNode;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use fama_common::{Formatter, CONFIG};

/// Formatter implementation for web and GraphQL files via Biome.
pub struct BiomeFormatter;

impl Formatter for BiomeFormatter {
	fn name(&self) -> &'static str {
		"biome"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&[
			"js", "cjs", "mjs", "ts", "mts", "cts", "jsx", "mjsx", "cjsx",
			"tsx", "mtsx", "ctsx", "json", "jsonc", "html", "htm", "vue",
			"svelte", "astro", "graphql", "gql",
		]
	}

	fn format(&self, content: &str, path: &Path) -> Result<String, String> {
		format_file(content, path)
	}
}

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

static ORGANIZE_IMPORTS_FILTER: [RuleFilter<'static>; 1] = [RuleFilter::Rule(
	"source",
	<OrganizeImports as RuleMeta>::METADATA.name,
)];

static SHARED_MODULE_GRAPH: OnceLock<Arc<ModuleGraph>> = OnceLock::new();
static SHARED_PROJECT_LAYOUT: OnceLock<Arc<ProjectLayout>> = OnceLock::new();

fn sort_imports(
	root: &AnyJsRoot,
	source_type: JsFileSource,
	file_path: &str,
) -> Option<AnyJsRoot> {
	let categories = RuleCategoriesBuilder::default().with_assist().build();

	let filter = AnalysisFilter {
		categories,
		enabled_rules: Some(&ORGANIZE_IMPORTS_FILTER),
		..AnalysisFilter::default()
	};

	let options = AnalyzerOptions::default().with_file_path(file_path);

	let module_graph =
		SHARED_MODULE_GRAPH.get_or_init(|| Arc::new(ModuleGraph::default()));
	let project_layout =
		SHARED_PROJECT_LAYOUT.get_or_init(|| Arc::new(ProjectLayout::default()));
	let services = JsAnalyzerServices::from((
		module_graph.clone(),
		project_layout.clone(),
		source_type,
	));

	let mut sorted_root: Option<AnyJsRoot> = None;

	let _ = biome_js_analyze::analyze(
		root,
		filter,
		&options,
		&[],
		services,
		|signal| {
			for action in signal.actions() {
				if action.category
					== ActionCategory::Source(SourceActionKind::OrganizeImports)
				{
					let new_syntax = action.mutation.commit();
					if let Some(new_root) = AnyJsRoot::cast(new_syntax) {
						sorted_root = Some(new_root);
					}
				}
			}
			ControlFlow::<()>::Continue(())
		},
	);

	sorted_root
}

fn format_js_family(
	source: &str,
	file_path: &str,
	source_type: JsFileSource,
	file_type_name: &str,
) -> Result<String, String> {
	let options = JsFormatOptions::new(source_type)
		.with_indent_style(BIOME_INDENT_STYLE)
		.with_indent_width(IndentWidth::try_from(BIOME_INDENT_WIDTH).unwrap())
		.with_line_width(LineWidth::try_from(BIOME_LINE_WIDTH).unwrap())
		.with_line_ending(BIOME_LINE_ENDING)
		.with_quote_style(BIOME_QUOTE_STYLE)
		.with_trailing_commas(BIOME_TRAILING_COMMAS)
		.with_semicolons(BIOME_SEMICOLONS)
		.with_bracket_spacing(BracketSpacing::from(BIOME_BRACKET_SPACING));

	let parsed = parse(source, source_type, JsParserOptions::default());

	if parsed.has_errors() {
		return Err(format!("Parse errors in {file_type_name} file"));
	}

	let root = parsed.tree();
	let sorted = if source.contains("import") || source.contains("from") {
		sort_imports(&root, source_type, file_path)
	} else {
		None
	};
	let syntax = match &sorted {
		Some(sorted_root) => sorted_root.syntax(),
		None => root.syntax(),
	};

	let formatted = biome_js_formatter::format_node(options, syntax)
		.map_err(|e| format!("Format error: {e:?}"))?;

	formatted
		.print()
		.map(|p| p.into_code())
		.map_err(|e| format!("Print error: {e:?}"))
}

pub fn format_javascript(source: &str, file_path: &str) -> Result<String, String> {
	format_js_family(source, file_path, JsFileSource::js_module(), "JavaScript")
}

pub fn format_typescript(source: &str, file_path: &str) -> Result<String, String> {
	format_js_family(source, file_path, JsFileSource::ts(), "TypeScript")
}

pub fn format_jsx(source: &str, file_path: &str) -> Result<String, String> {
	format_js_family(source, file_path, JsFileSource::jsx(), "JSX")
}

pub fn format_tsx(source: &str, file_path: &str) -> Result<String, String> {
	format_js_family(source, file_path, JsFileSource::tsx(), "TSX")
}

pub fn format_json(source: &str, _file_path: &str) -> Result<String, String> {
	format_json_internal(source, JsonFileSource::json(), false)
}

pub fn format_jsonc(source: &str, _file_path: &str) -> Result<String, String> {
	format_json_internal(
		source,
		JsonFileSource::json_allow_comments("jsonc"),
		true,
	)
}

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
		.map_err(|e| format!("Format error: {e:?}"))?;

	formatted
		.print()
		.map(|p| p.into_code())
		.map_err(|e| format!("Print error: {e:?}"))
}

pub fn format_html(source: &str, _file_path: &str) -> Result<String, String> {
	let options = biome_html_formatter::context::HtmlFormatOptions::default()
		.with_indent_style(BIOME_INDENT_STYLE)
		.with_indent_width(IndentWidth::try_from(BIOME_INDENT_WIDTH).unwrap())
		.with_line_width(LineWidth::try_from(BIOME_LINE_WIDTH).unwrap());

	let parsed = parse_html(source, HtmlParseOptions::default());

	if parsed.has_errors() {
		return Err("Parse errors in HTML file".to_string());
	}

	let syntax = parsed.syntax();

	let formatted = biome_html_formatter::format_node(options, &syntax, false)
		.map_err(|e| format!("Format error: {e:?}"))?;

	formatted
		.print()
		.map(|p| p.into_code())
		.map_err(|e| format!("Print error: {e:?}"))
}

pub fn format_vue(source: &str, file_path: &str) -> Result<String, String> {
	match format_html(source, file_path) {
		Ok(result) => Ok(result),
		Err(_) => Ok(source.to_string()),
	}
}

pub fn format_svelte(source: &str, file_path: &str) -> Result<String, String> {
	match format_html(source, file_path) {
		Ok(result) => Ok(result),
		Err(_) => {
			eprintln!("Warning: {file_path} syntax not fully supported, file may not be properly formatted");
			Ok(source.to_string())
		}
	}
}

pub fn format_astro(source: &str, file_path: &str) -> Result<String, String> {
	match format_html(source, file_path) {
		Ok(result) => Ok(result),
		Err(_) => {
			eprintln!("Warning: {file_path} syntax not fully supported, file may not be properly formatted");
			Ok(source.to_string())
		}
	}
}

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
		return Err("Parse errors in GraphQL file".to_string());
	}

	let syntax = parsed.syntax();

	let formatted = biome_graphql_formatter::format_node(options, &syntax)
		.map_err(|e| format!("Format error: {e:?}"))?;

	formatted
		.print()
		.map(|p| p.into_code())
		.map_err(|e| format!("Print error: {e:?}"))
}

pub fn format_file(source: &str, path: &Path) -> Result<String, String> {
	let file_path = path.to_str().unwrap_or("");
	match path.extension().and_then(|e| e.to_str()) {
		Some("js" | "cjs" | "mjs") => format_javascript(source, file_path),
		Some("ts" | "mts" | "cts") => format_typescript(source, file_path),
		Some("jsx" | "mjsx" | "cjsx") => format_jsx(source, file_path),
		Some("tsx" | "mtsx" | "ctsx") => format_tsx(source, file_path),
		Some("json") => match format_json(source, file_path) {
			Ok(result) => Ok(result),
			Err(_) => format_jsonc(source, file_path),
		},
		Some("jsonc") => format_jsonc(source, file_path),
		Some("html" | "htm") => format_html(source, file_path),
		Some("vue") => format_vue(source, file_path),
		Some("svelte") => format_svelte(source, file_path),
		Some("astro") => format_astro(source, file_path),
		Some("graphql" | "gql") => format_graphql(source, file_path),
		_ => Err(format!(
			"File {} is not supported by biome",
			path.display()
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
		let result = format_file(source, Path::new("test.js")).unwrap();
		assert!(result.contains("x = 1"));
	}

	#[test]
	fn test_format_file_with_unsupported_type() {
		let source = "test";
		let result = format_file(source, Path::new("test.css"));
		assert!(result.is_err());
	}

	#[test]
	fn test_format_json() {
		let source = r#"{"name":"test","value":1}"#;
		let result = format_json(source, "test.json").unwrap();
		assert!(result.contains("\"name\""));
		assert!(result.contains("\"test\""));
		assert!(result.ends_with('\n'), "JSON should end with newline");
	}

	#[test]
	fn test_format_file_with_json() {
		let source = r#"{"key":"value"}"#;
		let result = format_file(source, Path::new("test.json")).unwrap();
		assert!(result.contains("\"key\""));
	}

	#[test]
	fn test_format_json_with_comments_fallback() {
		let source = r#"{
  // This is a comment
  "name": "test",
  "value": 1
}"#;
		let result = format_file(source, Path::new("test.json")).unwrap();
		assert!(result.contains("\"name\""));
		assert!(result.contains("// This is a comment"));
	}

	#[test]
	fn test_sort_imports_javascript() {
		let source = r#"import z from "./local";
import a from "package-a";
import b from "package-b";
"#;
		let result = format_javascript(source, "test.js").unwrap();
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
		let source = r#"import { Component } from "./Component";
import type { Props } from "./types";
import React from "react";
import path from "node:path";
"#;
		let result = format_typescript(source, "test.ts").unwrap();
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
		let source = r#"import { z, a, m, B } from "package";
"#;
		let result = format_javascript(source, "test.js").unwrap();
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
	fn test_format_typescript_and_jsx_variants() {
		for (path, src) in [
			("mod.cts", "const x:number=1;"),
			("mod.mts", "const x:number=1;"),
			("comp.cjsx", "const el=<div />;"),
			("comp.mtsx", "const el:unknown=<div />;"),
			("comp.ctsx", "const el:unknown=<div />;"),
		] {
			let formatted =
				format_file(src, Path::new(path)).expect(path);
			assert!(
				formatted.contains("const "),
				"unexpected output for {path}: {formatted}"
			);
		}
	}

	#[test]
	fn test_sort_imports_with_side_effects() {
		let source = r#"import "./polyfill";
import a from "package-a";
"#;
		let result = format_javascript(source, "test.js").unwrap();
		let polyfill_pos = result.find("polyfill").unwrap();
		let a_pos = result.find("package-a").unwrap();
		assert!(
			polyfill_pos < a_pos,
			"Side-effect imports should maintain their position. Got: {}",
			result
		);
	}
}
