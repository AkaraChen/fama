//! Nix formatter via the alejandra crate (Rust-native, Unlicense).

use alejandra::format::{in_memory, Status};
use fama_common::FileType;

/// Format a Nix file based on its detected type
pub fn format_nix(source: &str, path: &str) -> Result<String, String> {
	match in_memory(path.to_string(), source.to_string()) {
		(Status::Changed(_), formatted) => Ok(formatted),
		(Status::Error(message), _) => Err(message),
	}
}

/// Format a file based on its file type
pub fn format_file(
	source: &str,
	file_path: &str,
	file_type: FileType,
) -> Result<String, String> {
	match file_type {
		FileType::Nix => format_nix(source, file_path),
		_ => Err(format!(
			"File type {:?} is not supported by the nix formatter",
			file_type
		)),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Format `source`, requiring success, and assert the result is
	/// idempotent (formatting it again changes nothing).
	fn assert_idempotent(source: &str) -> String {
		let once = format_nix(source, "test.nix")
			.expect("valid Nix input should format successfully");
		let twice = format_nix(&once, "test.nix")
			.expect("formatted output should still be valid Nix");
		assert_eq!(once, twice, "formatting must be idempotent");
		once
	}

	#[test]
	fn formats_simple_attrset() {
		let formatted = assert_idempotent("{ foo = 1; }");
		assert_eq!(
			formatted, "{foo = 1;}\n",
			"alejandra keeps one-line attrsets in tight style"
		);
	}

	#[test]
	fn formats_mkderivation() {
		let messy = r#"{ stdenv, fetchurl }:stdenv.mkDerivation rec{name="hello";version="1.0.0";src=fetchurl{url="mirror://gnu/hello/hello-${version}.tar.gz";sha256="0nciibjaw4csm6ct8i5wcy4c7ly3d4z1r6y7akq32n5mxvs89mrp";};meta={description="A program that produces a familiar, friendly greeting";};}"#;
		let formatted = assert_idempotent(messy);
		assert_ne!(formatted, messy, "messy input should be reformatted");
		// Content is preserved and the derivation body is expanded.
		assert!(formatted.contains("name = \"hello\";"));
		assert!(formatted.contains("version = \"1.0.0\";"));
		assert!(formatted.contains("stdenv.mkDerivation rec"));
		assert!(formatted.contains("description = \"A program that produces a familiar, friendly greeting\";"));
	}

	#[test]
	fn formats_mkshell_with_build_inputs() {
		let messy = "{pkgs}:pkgs.mkShell{buildInputs=with pkgs;[ripgrep fd bat];shellHook=\"echo welcome\";}";
		let formatted = assert_idempotent(messy);
		assert_ne!(formatted, messy, "messy input should be reformatted");
		assert!(formatted.contains("pkgs.mkShell"));
		assert!(formatted.contains("ripgrep"));
		assert!(formatted.contains("shellHook = \"echo welcome\";"));
	}

	#[test]
	fn formats_nested_attrsets() {
		let messy = "{ a = { b = { c = 1; d = { e = 2; }; }; }; }";
		let formatted = assert_idempotent(messy);
		assert_eq!(
			formatted,
			"{\n  a = {\n    b = {\n      c = 1;\n      d = {e = 2;};\n    };\n  };\n}\n",
			"nesting gets two-space indentation per level; \
			 attrsets that fit stay tight"
		);
	}

	#[test]
	fn formats_let_in() {
		let messy = "let x = 1;y = x + 1;in y";
		let formatted = assert_idempotent(messy);
		assert_ne!(formatted, messy, "messy input should be reformatted");
		assert!(formatted.contains("x = 1;"));
		assert!(formatted.contains("y = x + 1;"));
		assert!(formatted.contains("in"));
	}

	#[test]
	fn formats_list_of_attrsets() {
		let messy =
			"[ { name = \"a\";value = 1; } { name = \"b\";value = 2; } ]";
		let formatted = assert_idempotent(messy);
		assert_ne!(formatted, messy, "messy input should be reformatted");
		assert!(formatted.contains("name = \"a\";"));
		assert!(formatted.contains("value = 2;"));
		// Each element lands on its own set of lines inside the list.
		assert!(formatted.starts_with("[\n"));
	}

	#[test]
	fn formats_string_interpolation() {
		let messy = r#"{ greeting = "hello, ${user}!"; user = "world"; }"#;
		let formatted = assert_idempotent(messy);
		assert_ne!(formatted, messy, "messy input should be reformatted");
		assert!(
			formatted.contains(r#""hello, ${user}!""#),
			"interpolation is preserved verbatim"
		);
		assert!(formatted.contains("user = \"world\";"));
	}

	#[test]
	fn keeps_already_formatted_input() {
		let canonical = "{\n  foo = 1;\n}\n";
		let formatted = format_nix(canonical, "test.nix")
			.expect("should format successfully");
		assert_eq!(formatted, canonical, "canonical input must pass through");
	}

	#[test]
	fn rejects_garbage_input() {
		for broken in ["{{{", "{ x = ; }", r#"{{ "unterminated "#] {
			let result = format_nix(broken, "broken.nix");
			assert!(
				result.is_err(),
				"{broken:?} is not valid Nix and must produce Err, got {result:?}"
			);
		}
	}

	#[test]
	fn format_file_dispatches_and_rejects_other_types() {
		assert!(format_file("{ foo = 1; }", "a.nix", FileType::Nix).is_ok());
		assert!(
			format_file("anything", "a.txt", FileType::Markdown).is_err(),
			"non-Nix types must be rejected by the dispatch"
		);
	}
}
