//! External CLI ("process mode") formatters and shared subprocess helpers.

pub mod cairo;
pub mod elixir;
pub mod gleam;
pub mod just;
pub mod move_lang;
pub mod nushell;
pub mod prisma;
pub mod process;
pub mod r;
pub mod rust;
pub mod scala;
pub mod solidity;

use fama_common::Formatter;

/// All process-mode formatters registered by `fama`.
pub const FORMATTERS: &[&dyn Formatter] = &[
	&rust::RustFormatter,
	&scala::ScalaFormatter,
	&r::RFormatter,
	&elixir::ElixirFormatter,
	&solidity::SolidityFormatter,
	&gleam::GleamFormatter,
	&prisma::PrismaFormatter,
	&just::JustFormatter,
	&cairo::CairoFormatter,
	&move_lang::MoveFormatter,
	&nushell::NushellFormatter,
];
