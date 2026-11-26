// dockerfile.rs - Dockerfile formatter using dprint-plugin-dockerfile

use dprint_core_dockerfile::configuration::NewLineKind;
use dprint_plugin_dockerfile::configuration::ConfigurationBuilder;
use dprint_plugin_dockerfile::format_text;
use fama_common::{FormatConfig, Formatter, LineEnding};
use std::path::{Path, PathBuf};

/// Formatter implementation for Dockerfiles via dprint-plugin-dockerfile.
pub struct DockerfileFormatter;

impl Formatter for DockerfileFormatter {
	fn name(&self) -> &'static str {
		"dockerfile"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&[]
	}

	fn matches_filename(&self, name: &str) -> bool {
		name == "Dockerfile" || name.starts_with("Dockerfile.")
	}

	fn format(&self, content: &str, path: &Path) -> Result<String, String> {
		format_dockerfile(content, path.to_str().unwrap_or("Dockerfile"))
	}
}

pub fn format_dockerfile(
	source: &str,
	file_path: &str,
) -> Result<String, String> {
	let fmt_config = FormatConfig::default();

	let new_line_kind = match fmt_config.line_ending {
		LineEnding::Lf => NewLineKind::LineFeed,
		LineEnding::Crlf => NewLineKind::CarriageReturnLineFeed,
	};

	let config = ConfigurationBuilder::new()
		.line_width(fmt_config.line_width as u32)
		.new_line_kind(new_line_kind)
		.build();

	match format_text(&PathBuf::from(file_path), source, &config) {
		Ok(Some(formatted)) => Ok(formatted),
		Ok(None) => Ok(source.to_string()),
		Err(e) => Err(format!("Dockerfile formatting error: {}", e)),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_format_dockerfile_from() {
		let source = "FROM    ubuntu:20.04\n";
		let result = format_dockerfile(source, "Dockerfile").unwrap();
		assert!(result.contains("FROM"));
		assert!(result.contains("ubuntu"));
	}

	#[test]
	fn test_format_dockerfile_run() {
		let source = "FROM ubuntu\nRUN    apt-get   update\n";
		let result = format_dockerfile(source, "Dockerfile").unwrap();
		assert!(result.contains("RUN"));
	}

	#[test]
	fn test_format_dockerfile_multiline() {
		let source = "FROM ubuntu\nRUN apt-get update && \\\n    apt-get install -y curl\n";
		let result = format_dockerfile(source, "Dockerfile").unwrap();
		assert!(result.contains("FROM"));
		assert!(result.contains("apt-get"));
	}
}
