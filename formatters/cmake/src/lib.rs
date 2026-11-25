//! CMake formatter via the cmakefmt-rust crate (Rust-native cmake-format
//! successor).

use fama_common::FileType;

/// Format a CMake file based on its detected type
pub fn format_cmake(source: &str, _path: &str) -> Result<String, String> {
	cmakefmt::format_source(source, &cmakefmt::Config::default())
		.map_err(|e| format!("cmake formatting failed: {e}"))
}

/// Format a file based on its file type
pub fn format_file(
	source: &str,
	file_path: &str,
	file_type: FileType,
) -> Result<String, String> {
	match file_type {
		FileType::CMake => format_cmake(source, file_path),
		_ => Err(format!(
			"File type {:?} is not supported by the cmake formatter",
			file_type
		)),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_format_cmake_lists() {
		let source = "\
CMAKE_MINIMUM_REQUIRED(VERSION 3.20)
project(   myproj   VERSION 1.2.3 LANGUAGES CXX )
    add_executable( myapp
            main.cpp
        util.cpp
        config.h )
if(win32)
      target_link_libraries( myapp   private ws2_32 )
endif()
";
		let formatted = format_cmake(source, "CMakeLists.txt").unwrap();

		// Command names are normalized to lower case.
		assert!(formatted.contains("cmake_minimum_required(VERSION 3.20)"));
		// Whitespace inside commands is collapsed.
		assert!(
			formatted.contains("project(myproj VERSION 1.2.3 LANGUAGES CXX)")
		);
		// Short multi-source commands are fitted onto one line.
		assert!(formatted
			.contains("add_executable(myapp main.cpp util.cpp config.h)"));
		// Block bodies get consistent indentation.
		assert!(
			formatted
				.contains("\n  target_link_libraries(myapp PRIVATE ws2_32)"),
			"block body should be indented:\n{formatted}"
		);
		// Keywords recognized by a command's spec are normalized to upper case.
		assert!(formatted.contains("PRIVATE"));
		// Block structure is preserved.
		assert!(formatted.starts_with("cmake_minimum_required"));
		assert!(formatted.contains("if(win32)"));
		assert!(formatted.trim_end().ends_with("endif()"));
	}

	#[test]
	fn test_format_cmake_is_idempotent() {
		let source = "\
project(myproj)
add_executable( myapp
        main.cpp
    util.cpp )
if( BUILD_TESTS )
    add_executable(  apptest test.cpp  )
    target_link_libraries(apptest private myapp)
endif()
";
		let once = format_cmake(source, "CMakeLists.txt").unwrap();
		let twice = format_cmake(&once, "CMakeLists.txt").unwrap();
		assert_eq!(once, twice, "formatting should be idempotent");
	}

	#[test]
	fn test_format_cmake_invalid_syntax() {
		let result =
			format_cmake("add_executable(myapp\nmain.cpp", "CMakeLists.txt");
		assert!(result.is_err());
	}

	#[test]
	fn test_format_file_dispatches_cmake() {
		let source = "project( demo )\n";
		let formatted =
			format_file(source, "CMakeLists.txt", FileType::CMake).unwrap();
		assert!(formatted.contains("project(demo)"));
	}

	#[test]
	fn test_format_file_rejects_other_types() {
		let result = format_file("x = 1", "x.py", FileType::Python);
		assert!(result.is_err());
	}
}
