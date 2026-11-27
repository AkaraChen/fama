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

	fn linguist_languages(&self, ext: &str) -> &'static [&'static str] {
		match ext {
			"cls" => &["TeX"],
			_ => &[],
		}
	}

	fn format(&self, content: &str, path: &Path) -> Result<String, String> {
		format_latex(content, path)
	}
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
	fn test_cls_linguist_languages() {
		assert!(LatexFormatter.needs_content_check("cls"));
		assert!(!LatexFormatter.needs_content_check("tex"));
		assert_eq!(LatexFormatter.linguist_languages("cls"), &["TeX"]);
		assert!(LatexFormatter.linguist_languages("tex").is_empty());
	}
}
