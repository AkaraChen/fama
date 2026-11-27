//! Tree-sitter-driven declarative formatters powered by `topiary-core`.
//!
//! Each language is described declaratively by a [`TopiarySpec`] binding its
//! extensions, compiled Tree-sitter grammar, and Topiary `.scm` formatting query.

use fama_common::{Formatter, IndentStyle, CONFIG};
use std::path::Path;
use std::sync::OnceLock;
use topiary_core::{formatter_str, Language, Operation, TopiaryQuery};

/// Declarative specification for a Topiary-backed language formatter.
struct TopiarySpec {
	name: &'static str,
	extensions: &'static [&'static str],
	grammar: fn() -> tree_sitter::Language,
	query: fn() -> &'static str,
}

/// Formatter implementation backed by `topiary-core` and a cached `TopiaryQuery`.
pub struct TopiaryLangFormatter {
	spec: TopiarySpec,
	compiled: OnceLock<Result<Language, String>>,
}

impl TopiaryLangFormatter {
	const fn new(spec: TopiarySpec) -> Self {
		Self {
			spec,
			compiled: OnceLock::new(),
		}
	}

	fn language(&self) -> Result<&Language, String> {
		self.compiled
			.get_or_init(|| {
				let grammar: topiary_tree_sitter_facade::Language =
					(self.spec.grammar)().into();
				let formatting_query =
					TopiaryQuery::new(&grammar, (self.spec.query)())
						.map_err(|e| {
							format!(
								"Failed to compile {} Topiary query: {e}",
								self.spec.name
							)
						})?;
				let indent = match CONFIG.indent_style {
					IndentStyle::Tabs => "\t".to_string(),
					IndentStyle::Spaces => {
						" ".repeat(CONFIG.indent_width as usize)
					}
				};
				Ok(Language {
					name: self.spec.name.to_owned(),
					formatting_query,
					injection_query: None,
					grammar,
					indent: Some(indent),
				})
			})
			.as_ref()
			.map_err(Clone::clone)
	}
}

impl Formatter for TopiaryLangFormatter {
	fn name(&self) -> &'static str {
		self.spec.name
	}

	fn extensions(&self) -> &'static [&'static str] {
		self.spec.extensions
	}

	fn format(&self, content: &str, _path: &Path) -> Result<String, String> {
		let lang = self.language()?;
		let mut output = Vec::with_capacity(content.len());
		formatter_str(
			content,
			&mut output,
			lang,
			Operation::Format {
				skip_idempotence: true,
				tolerate_parsing_errors: false,
				skip_stage: None,
			},
			None,
		)
		.map_err(|e| format!("{} formatting error: {e}", self.spec.name))?;

		String::from_utf8(output).map_err(|e| {
			format!("{} produced invalid UTF-8: {e}", self.spec.name)
		})
	}
}

fn nickel_grammar() -> tree_sitter::Language {
	tree_sitter::Language::new(tree_sitter_nickel::LANGUAGE)
}

fn openscad_grammar() -> tree_sitter::Language {
	tree_sitter::Language::new(tree_sitter_openscad::LANGUAGE)
}

pub static NICKEL: TopiaryLangFormatter = TopiaryLangFormatter::new(TopiarySpec {
	name: "nickel",
	extensions: &["ncl"],
	grammar: nickel_grammar,
	query: topiary_queries::nickel,
});

pub static OPENSCAD: TopiaryLangFormatter =
	TopiaryLangFormatter::new(TopiarySpec {
		name: "openscad",
		extensions: &["scad"],
		grammar: openscad_grammar,
		query: topiary_queries::openscad,
	});

/// All Topiary-backed formatters exported to `fama`.
pub const FORMATTERS: &[&dyn Formatter] = &[&NICKEL, &OPENSCAD];

#[cfg(test)]
mod tests {
	use super::*;

	fn assert_formats_and_idempotent(
		fmt: &dyn Formatter,
		path: &str,
		input: &str,
	) -> String {
		let once = fmt
			.format(input, Path::new(path))
			.unwrap_or_else(|e| panic!("{path} failed: {e}"));
		let twice = fmt
			.format(&once, Path::new(path))
			.unwrap_or_else(|e| panic!("{path} second pass failed: {e}"));
		assert_eq!(once, twice, "{path} formatting must be idempotent");
		once
	}

	#[test]
	fn test_format_nickel() {
		let messy = "{foo=1,bar=[1,2,3],nested={a=\"x\",b=true}}";
		let formatted =
			assert_formats_and_idempotent(&NICKEL, "config.ncl", messy);
		assert!(formatted.contains("foo = 1"), "{formatted}");
		assert!(NICKEL.format("{{{", Path::new("bad.ncl")).is_err());
	}

	#[test]
	fn test_format_openscad() {
		let messy = "module box(w=10,h=20){cube([w,h,5]);}";
		let formatted =
			assert_formats_and_idempotent(&OPENSCAD, "part.scad", messy);
		assert!(formatted.contains("module box("), "{formatted}");
		assert!(OPENSCAD.format("module {", Path::new("bad.scad")).is_err());
	}
}
