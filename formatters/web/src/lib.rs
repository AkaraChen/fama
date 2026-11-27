//! Web, markup, style, data, and document-container formatters.

#[allow(clippy::all)]
pub mod biome;
#[allow(clippy::all)]
pub mod dprint;
pub mod json5;
pub mod jupyter;
pub mod markup;
pub mod mdx;

use fama_common::Formatter;

/// All web and document formatters registered by `fama`.
pub const FORMATTERS: &[&dyn Formatter] = &[
	&biome::BiomeFormatter,
	&dprint::DprintFormatter,
	&json5::Json5Formatter,
	&jupyter::JupyterFormatter,
	&markup::MarkupFormatter,
	&mdx::MdxFormatter,
];
