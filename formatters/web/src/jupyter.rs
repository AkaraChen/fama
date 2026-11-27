//! Jupyter Notebook (`.ipynb`) formatter via `dprint-plugin-jupyter`.

use fama_common::Formatter;
use std::error::Error;
use std::path::Path;

/// Formatter implementation for Jupyter notebooks via `dprint-plugin-jupyter`.
pub struct JupyterFormatter;

impl Formatter for JupyterFormatter {
	fn name(&self) -> &'static str {
		"jupyter"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["ipynb"]
	}

	fn format(&self, content: &str, _path: &Path) -> Result<String, String> {
		format_jupyter(content)
	}
}

fn format_cell(
	path: &Path,
	code: String,
) -> Result<Option<String>, Box<dyn Error + Send + Sync + 'static>> {
	let formatted = if let Some(res) = fama_common::format_embedded(&code, path)
	{
		res
	} else {
		fallback_format_cell(path, &code)
	};

	match formatted {
		Ok(out) if out != code => Ok(Some(out)),
		Ok(_) => Ok(None),
		Err(e) => Err(e.into()),
	}
}

/// Local fallback used when `fama-web` tests run without `cli`'s `Registry`.
fn fallback_format_cell(path: &Path, code: &str) -> Result<String, String> {
	let path_str = path.to_str().unwrap_or("");
	match path.extension().and_then(|e| e.to_str()) {
		Some("md" | "markdown") => crate::dprint::format_markdown(code, path_str),
		Some("js" | "ts" | "jsx" | "tsx" | "json") => {
			crate::biome::format_file(code, path)
		}
		_ => Ok(code.to_string()),
	}
}

pub fn format_jupyter(source: &str) -> Result<String, String> {
	match dprint_plugin_jupyter::format_text(source, format_cell) {
		Ok(Some(formatted)) => Ok(formatted),
		Ok(None) => Ok(source.to_string()),
		Err(e) => Err(format!("Jupyter formatting error: {e}")),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_format_jupyter_js_and_markdown_cells() {
		let notebook = r##"{
  "cells": [
    {
      "cell_type": "markdown",
      "metadata": {},
      "source": [
        "#   Title\n",
        "\n",
        "*  item"
      ]
    },
    {
      "cell_type": "code",
      "execution_count": null,
      "metadata": {},
      "outputs": [],
      "source": [
        "const x=1+2\n",
        "function f( a , b ){ return a+b }"
      ]
    }
  ],
  "metadata": {
    "kernelspec": {
      "display_name": "Deno",
      "language": "javascript",
      "name": "deno"
    },
    "language_info": {
      "name": "javascript"
    }
  },
  "nbformat": 4,
  "nbformat_minor": 5
}
"##;

		let once = format_jupyter(notebook).unwrap();
		assert!(once.contains("const x = 1 + 2;"), "{once}");
		assert!(once.contains("function f(a, b)"), "{once}");
		let twice = format_jupyter(&once).unwrap();
		assert_eq!(once, twice);
	}

	#[test]
	fn test_format_jupyter_invalid_json() {
		assert!(format_jupyter("{ not valid notebook").is_err());
	}
}
