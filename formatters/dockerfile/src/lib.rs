// dockerfile-formatter - Dockerfile formatter using dprint-plugin-dockerfile
//
// Provides Dockerfile formatting using the dprint dockerfile plugin.

use dprint_core::configuration::NewLineKind;
use dprint_plugin_dockerfile::configuration::ConfigurationBuilder;
use dprint_plugin_dockerfile::format_text;
use fama_common::{FormatConfig, LineEnding};
use std::path::PathBuf;

/// Format Dockerfile source code
///
/// # Arguments
/// * `source` - The Dockerfile source code to format
/// * `file_path` - The original file path
///
/// # Returns
/// * `Ok(String)` - Formatted code
/// * `Err(String)` - Error message if formatting fails
pub fn format_dockerfile(source: &str, file_path: &str) -> Result<String, String> {
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
        Ok(None) => Ok(source.to_string()), // Already formatted
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
