//! Terminal color utilities for CLI output

/// ANSI color codes for terminal output
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
	Green,
	Red,
}

impl Color {
	/// Wrap text with ANSI color codes
	pub fn paint(self, text: &str) -> String {
		match self {
			Color::Green => format!("\x1b[32m{}\x1b[0m", text),
			Color::Red => format!("\x1b[31m{}\x1b[0m", text),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_color_green_paint() {
		let result = Color::Green.paint("test");
		assert_eq!(result, "\x1b[32mtest\x1b[0m");
	}

	#[test]
	fn test_color_red_paint() {
		let result = Color::Red.paint("error");
		assert_eq!(result, "\x1b[31merror\x1b[0m");
	}

	#[test]
	fn test_color_empty_string() {
		assert_eq!(Color::Green.paint(""), "\x1b[32m\x1b[0m");
		assert_eq!(Color::Red.paint(""), "\x1b[31m\x1b[0m");
	}

	#[test]
	fn test_color_special_characters() {
		assert_eq!(Color::Green.paint("hello\nworld"), "\x1b[32mhello\nworld\x1b[0m");
		assert_eq!(Color::Red.paint("tab\there"), "\x1b[31mtab\there\x1b[0m");
	}

	#[test]
	fn test_color_clone() {
		let green = Color::Green;
		let cloned = green.clone();
		assert_eq!(green, cloned);
		
		let red = Color::Red;
		let cloned_red = red.clone();
		assert_eq!(red, cloned_red);
	}

	#[test]
	fn test_color_copy() {
		let green = Color::Green;
		let copied = green;
		assert_eq!(green, copied);
		
		let red = Color::Red;
		let copied_red = red;
		assert_eq!(red, copied_red);
	}

	#[test]
	fn test_color_equality() {
		assert_eq!(Color::Green, Color::Green);
		assert_eq!(Color::Red, Color::Red);
		assert_ne!(Color::Green, Color::Red);
	}
}
