//! Web template formatter (Jinja2, Twig, Nunjucks, Mustache, Handlebars, Vento) via `markup_fmt`.

use fama_common::{Formatter, IndentStyle, LineEnding, CONFIG};
use markup_fmt::config::{FormatOptions, LanguageOptions, LayoutOptions, LineBreak};
use markup_fmt::{format_text, Language};
use std::path::Path;
use std::sync::OnceLock;

/// Formatter implementation for template markup languages via `markup_fmt`.
pub struct MarkupFormatter;

impl Formatter for MarkupFormatter {
	fn name(&self) -> &'static str {
		"markup"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&[
			"jinja",
			"jinja2",
			"j2",
			"njk",
			"twig",
			"hbs",
			"mustache",
			"vto",
		]
	}

	fn format(&self, content: &str, path: &Path) -> Result<String, String> {
		format_markup(content, path)
	}
}

fn options() -> &'static FormatOptions {
	static CFG: OnceLock<FormatOptions> = OnceLock::new();
	CFG.get_or_init(|| FormatOptions {
		layout: LayoutOptions {
			print_width: CONFIG.line_width as usize,
			use_tabs: matches!(CONFIG.indent_style, IndentStyle::Tabs),
			indent_width: CONFIG.indent_width as usize,
			line_break: match CONFIG.line_ending {
				LineEnding::Lf => LineBreak::Lf,
				LineEnding::Crlf => LineBreak::Crlf,
			},
		},
		language: LanguageOptions::default(),
	})
}

fn language_for_path(path: &Path) -> Result<Language, String> {
	match path.extension().and_then(|e| e.to_str()) {
		Some("jinja" | "jinja2" | "j2" | "njk" | "twig") => Ok(Language::Jinja),
		Some("hbs" | "mustache") => Ok(Language::Mustache),
		Some("vto") => Ok(Language::Vento),
		_ => Err(format!(
			"File {} is not supported by markup_fmt",
			path.display()
		)),
	}
}

pub fn format_markup(source: &str, path: &Path) -> Result<String, String> {
	let lang = language_for_path(path)?;
	format_text(source, lang, options(), |code, _| Ok(code.into()))
		.map_err(|e| format!("Markup formatting error: {e}"))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_format_jinja_mustache_vento() {
		let cases = [
			(
				"page.jinja2",
				"<div>{%if ok%}<p>{{name}}</p>{%endif%}</div>",
				"{% if ok %}",
			),
			(
				"card.hbs",
				"<div>{{#if show}}<span>{{title}}</span>{{/if}}</div>",
				"{{#if show}}",
			),
			(
				"view.vto",
				"<div>{{if ok}}<p>{{title}}</p>{{/if}}</div>",
				"{{ if ok }}",
			),
		];
		for (name, input, expected) in cases {
			let once = format_markup(input, Path::new(name)).unwrap();
			assert!(once.contains(expected), "{name}: {once}");
			let twice = format_markup(&once, Path::new(name)).unwrap();
			assert_eq!(once, twice, "{name} not idempotent");
		}
	}

	#[test]
	fn test_format_markup_syntax_error() {
		assert!(format_markup("<div><span></div>", Path::new("bad.j2")).is_err());
	}
}
