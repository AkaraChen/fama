use rubyfmt::{format_buffer, RichFormatError};

pub fn format_ruby(source: &str, _file_path: &str) -> Result<String, String> {
	format_buffer(source).map_err(|e| match e {
		RichFormatError::SyntaxError => "Ruby syntax error".to_string(),
		RichFormatError::IOError(io_err) => format!("Ruby formatting IO error: {}", io_err),
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
