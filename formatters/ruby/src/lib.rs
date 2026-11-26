use std::path::Path;

use fama_common::Formatter;
use rubyfmt::{format_buffer, RichFormatError};

/// Formatter implementation for Ruby via librubyfmt.
pub struct RubyFormatter;

/// Static formatter instance registered by `fama`.
pub static FORMATTER: RubyFormatter = RubyFormatter;

impl Formatter for RubyFormatter {
	fn name(&self) -> &'static str {
		"ruby"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["rb", "rake", "gemspec", "ru"]
	}

	fn matches_filename(&self, name: &str) -> bool {
		matches!(
			name,
			"Rakefile"
				| "Gemfile"
				| "Guardfile"
				| "Vagrantfile"
				| "Berksfile"
				| "Capfile"
				| "Thorfile"
				| "Fastfile"
				| "Appfile"
				| "Matchfile"
				| "Snapfile"
				| "Deliverfile"
				| "Scanfile"
				| "Gymfile"
		)
	}

	fn format(&self, content: &str, path: &Path) -> Result<String, String> {
		format_ruby(content, path.to_str().unwrap_or(""))
	}
}

pub fn format_ruby(source: &str, _file_path: &str) -> Result<String, String> {
	format_buffer(source).map_err(|e| match e {
		RichFormatError::SyntaxError => "Ruby syntax error".to_string(),
		RichFormatError::IOError(io_err) => {
			format!("Ruby formatting IO error: {}", io_err)
		}
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_format_simple_ruby() {
		let input = r#"def    hello(  name  )
puts   "Hello, #{name}!"
end"#;
		let result = format_ruby(input, "test.rb");
		assert!(result.is_ok());
		let output = result.unwrap();
		assert!(output.contains("def hello(name)"));
		assert!(output.contains("\t")); // Should use tabs
	}

	#[test]
	fn test_format_ruby_class() {
		let input = r#"class   Foo
def   initialize(x)
@x=x
end
end"#;
		let result = format_ruby(input, "test.rb");
		assert!(result.is_ok());
		let output = result.unwrap();
		assert!(output.contains("class Foo"));
	}

	#[test]
	fn test_format_ruby_syntax_error() {
		let input = "def foo(\nend";
		let result = format_ruby(input, "test.rb");
		assert!(result.is_err());
	}
}
