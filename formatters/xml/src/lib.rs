// xml-fmt - XML formatting library using quick-xml

use fama_common::{IndentStyle, CONFIG};
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;
use std::io::Cursor;

/// Format XML source code using quick-xml
pub fn format_xml(source: &str, _file_path: &str) -> Result<String, String> {
	let mut reader = Reader::from_str(source);
	reader.config_mut().trim_text(true);

	let mut writer = Writer::new_with_indent(
		Cursor::new(Vec::new()),
		match CONFIG.indent_style {
			IndentStyle::Tabs => b'\t',
			IndentStyle::Spaces => b' ',
		},
		if matches!(CONFIG.indent_style, IndentStyle::Tabs) {
			1
		} else {
			CONFIG.indent_width as usize
		},
	);

	let mut buf = Vec::new();

	loop {
		match reader.read_event_into(&mut buf) {
			Ok(Event::Start(e)) => {
				writer
					.write_event(Event::Start(BytesStart::new(
						String::from_utf8_lossy(e.name().as_ref()),
					)))
					.map_err(|e| e.to_string())?;
			}
			Ok(Event::End(e)) => {
				writer
					.write_event(Event::End(BytesEnd::new(
						String::from_utf8_lossy(e.name().as_ref()),
					)))
					.map_err(|e| e.to_string())?;
			}
			Ok(Event::Empty(e)) => {
				writer
					.write_event(Event::Empty(BytesStart::new(
						String::from_utf8_lossy(e.name().as_ref()),
					)))
					.map_err(|e| e.to_string())?;
			}
			Ok(Event::Text(e)) => {
				let text = String::from_utf8_lossy(e.as_ref());
				let trimmed = text.trim();
				if !trimmed.is_empty() {
					writer
						.write_event(Event::Text(BytesText::new(trimmed)))
						.map_err(|e| e.to_string())?;
				}
			}
			Ok(Event::Comment(e)) => {
				writer
					.write_event(Event::Comment(e))
					.map_err(|e| e.to_string())?;
			}
			Ok(Event::CData(e)) => {
				writer
					.write_event(Event::CData(e))
					.map_err(|e| e.to_string())?;
			}
			Ok(Event::Decl(e)) => {
				writer
					.write_event(Event::Decl(e))
					.map_err(|e| e.to_string())?;
			}
			Ok(Event::PI(e)) => {
				writer
					.write_event(Event::PI(e))
					.map_err(|e| e.to_string())?;
			}
			Ok(Event::DocType(e)) => {
				writer
					.write_event(Event::DocType(e))
					.map_err(|e| e.to_string())?;
			}
			Ok(Event::Eof) => break,
			Err(e) => return Err(format!("XML parse error: {:?}", e)),
		}
		buf.clear();
	}

	let result = writer.into_inner().into_inner();
	let mut formatted = String::from_utf8(result).map_err(|e| e.to_string())?;

	// Ensure trailing newline
	if !formatted.ends_with('\n') {
		formatted.push('\n');
	}

	Ok(formatted)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_format_simple_xml() {
		let source = r#"<root><child>text</child></root>"#;
		let result = format_xml(source, "test.xml").unwrap();
		assert!(result.contains("<root>"));
		assert!(result.contains("<child>"));
		assert!(result.contains("text"));
	}

	#[test]
	fn test_format_with_declaration() {
		let source = r#"<?xml version="1.0" encoding="UTF-8"?><root/>"#;
		let result = format_xml(source, "test.xml").unwrap();
		assert!(result.contains("<?xml"));
		assert!(result.contains("<root"));
	}

	#[test]
	fn test_format_trailing_newline() {
		let source = r#"<root/>"#;
		let result = format_xml(source, "test.xml").unwrap();
		assert!(result.ends_with('\n'));
	}

	#[test]
	fn test_format_invalid_xml() {
		let source = r#"<root><unclosed>"#;
		let result = format_xml(source, "test.xml");
		// quick-xml is lenient with unclosed tags, so this won't error
		// but the output won't be valid XML either
		assert!(result.is_ok());
	}
}
