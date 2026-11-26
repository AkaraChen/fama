//! CMake formatter via the cmakefmt-rust crate.

use fama_common::Formatter;
use std::path::Path;

/// Formatter implementation for CMake files (`*.cmake`, `CMakeLists.txt`, `*.cmake.in`).
pub struct CmakeFormatter;

impl Formatter for CmakeFormatter {
	fn name(&self) -> &'static str {
		"cmake"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["cmake"]
	}

	fn matches_filename(&self, name: &str) -> bool {
		name == "CMakeLists.txt" || name.ends_with(".cmake.in")
	}

	fn format(&self, content: &str, path: &Path) -> Result<String, String> {
		format_cmake(content, path.to_str().unwrap_or(""))
	}
}

pub fn format_cmake(source: &str, _path: &str) -> Result<String, String> {
	cmakefmt::format_source(source, &cmakefmt::Config::default())
		.map_err(|e| format!("cmake formatting failed: {e}"))
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

		assert!(formatted.contains("cmake_minimum_required(VERSION 3.20)"));
		assert!(
			formatted.contains("project(myproj VERSION 1.2.3 LANGUAGES CXX)")
		);
		assert!(formatted
			.contains("add_executable(myapp main.cpp util.cpp config.h)"));
		assert!(
			formatted
				.contains("\n  target_link_libraries(myapp PRIVATE ws2_32)"),
			"block body should be indented:\n{formatted}"
		);
		assert!(formatted.contains("PRIVATE"));
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
}
