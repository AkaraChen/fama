//! Terminal color utilities for CLI output

/// ANSI color codes for terminal output
#[derive(Clone, Copy)]
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
