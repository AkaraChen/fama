//! LaTeX / BibTeX formatter via `tex-fmt`.

use fama_common::{Formatter, IndentStyle, CONFIG};
use std::path::Path;
use tex_fmt::args::{Args, OptionArgs, TabChar};
use tex_fmt::format::format_file;
use tex_fmt::logging::Log;

/// Formatter implementation for LaTeX files via `tex-fmt`.
pub struct LatexFormatter;

impl Formatter for LatexFormatter {
	fn name(&self) -> &'static str {
		"latex"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["tex", "bib", "sty", "cls"]
	}

	fn needs_content_check(&self, ext: &str) -> bool {
		ext == "cls"
	}

	fn match_confidence(&self, path: &Path, content: &str) -> u8 {
		if path.extension().and_then(|e| e.to_str()) == Some("cls") {
			cls_confidence(content)
		} else {
			50
		}
	}

	fn format(&self, content: &str, path: &Path) -> Result<String, String> {
		format_latex(content, path)
	}
}

fn cls_confidence(content: &str) -> u8 {
	const TEX_CLS_MARKERS: &[&str] = &[
		"\\NeedsTeXFormat",
		"\\ProvidesClass",
		"\\LoadClass",
		"\\DeclareOption",
		"\\documentclass",
		"\\usepackage",
		"\\RequirePackage",
	];
	if TEX_CLS_MARKERS.iter().any(|m| content.contains(m)) {
		return 90;
	}

	const NON_TEX_CLS_MARKERS: &[&str] = &[
		"public class ",
		"global class ",
		"private class ",
		"@isTest",
		"VERSION 1.0 CLASS",
	];
	if NON_TEX_CLS_MARKERS.iter().any(|m| content.contains(m)) {
		return 0;
	}

	50
}

pub fn format_latex(source: &str, path: &Path) -> Result<String, String> {
	let opts = OptionArgs {
		wraplen: Some(CONFIG.line_width as usize),
		tabsize: Some(CONFIG.indent_width),
		tabchar: Some(match CONFIG.indent_style {
			IndentStyle::Tabs => TabChar::Tab,
			IndentStyle::Spaces => TabChar::Space,
		}),
		..OptionArgs::default()
	};
	let args = Args::from(opts);
	let mut logs: Vec<Log> = Vec::new();
	Ok(format_file(source, path, &args, &mut logs))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_format_latex_and_idempotence() {
		let messy = "\\begin{document}\n\\begin{itemize}\n\\item Hello\n\\end{itemize}\n\\end{document}\n";
		let once = format_latex(messy, Path::new("main.tex")).unwrap();
		assert!(once.contains("\t\\item Hello"), "{once}");
		let twice = format_latex(&once, Path::new("main.tex")).unwrap();
		assert_eq!(once, twice);
	}

	#[test]
	fn test_cls_disambiguation() {
		assert!(LatexFormatter.needs_content_check("cls"));
		assert!(!LatexFormatter.needs_content_check("tex"));

		assert_eq!(
			LatexFormatter.match_confidence(
				Path::new("article.cls"),
				"\\NeedsTeXFormat{LaTeX2e}\n\\ProvidesClass{myclass}"
			),
			90
		);
		assert_eq!(
			LatexFormatter.match_confidence(
				Path::new("AccountHandler.cls"),
				"public class AccountHandler {\n}"
			),
			0
		);
	}
}
