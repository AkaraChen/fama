//! Consolidated built-in formatters (pure Rust & lightweight CLI wrappers).

#[allow(clippy::all)]
pub mod biome;
pub mod cmake;
pub mod dockerfile;
#[allow(clippy::all)]
pub mod dprint;
pub mod elixir;
pub mod lua;
pub mod nix;
pub mod php;
pub mod python;
pub mod r;
pub mod rust;
pub mod scala;
pub mod sql;
pub mod toml;
pub mod wgsl;
pub mod xml;

use fama_common::Formatter;

/// All built-in formatters registered by `fama`.
pub const FORMATTERS: &[&dyn Formatter] = &[
	&biome::BiomeFormatter,
	&dprint::DprintFormatter,
	&toml::TomlFormatter,
	&rust::RustFormatter,
	&python::PythonFormatter,
	&lua::LuaFormatter,
	&dockerfile::DockerfileFormatter,
	&xml::XmlFormatter,
	&sql::SqlFormatter,
	&php::PhpFormatter,
	&cmake::CmakeFormatter,
	&nix::NixFormatter,
	&scala::ScalaFormatter,
	&r::RFormatter,
	&elixir::ElixirFormatter,
	&wgsl::WgslFormatter,
];
