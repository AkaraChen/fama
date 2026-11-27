//! Native pure-Rust built-in formatters.

pub mod cmake;
pub mod dockerfile;
pub mod gdscript;
pub mod kdl;
pub mod latex;
pub mod lua;
pub mod nix;
pub mod php;
pub mod python;
pub mod sql;
pub mod toml;
pub mod typst;
pub mod wgsl;
pub mod xml;

use fama_common::Formatter;

/// All native pure-Rust built-in formatters registered by `fama`.
pub const FORMATTERS: &[&dyn Formatter] = &[
	&toml::TomlFormatter,
	&python::PythonFormatter,
	&lua::LuaFormatter,
	&dockerfile::DockerfileFormatter,
	&xml::XmlFormatter,
	&sql::SqlFormatter,
	&php::PhpFormatter,
	&cmake::CmakeFormatter,
	&nix::NixFormatter,
	&wgsl::WgslFormatter,
	&typst::TypstFormatter,
	&kdl::KdlFormatter,
	&gdscript::GdscriptFormatter,
	&latex::LatexFormatter,
];
