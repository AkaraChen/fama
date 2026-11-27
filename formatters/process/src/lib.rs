//! External CLI ("process mode") formatters and shared subprocess helpers.

pub mod bicep;
pub mod cairo;
pub mod crystal;
pub mod elixir;
pub mod fish;
pub mod gleam;
pub mod just;
pub mod meson;
pub mod mojo;
pub mod move_lang;
pub mod nushell;
pub mod prisma;
pub mod process;
pub mod qml;
pub mod r;
pub mod rust;
pub mod scala;
pub mod solidity;
pub mod v;

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
	&fish::FishFormatter,
	&meson::MesonFormatter,
	&crystal::CrystalFormatter,
	&qml::QmlFormatter,
	&mojo::MojoFormatter,
	&bicep::BicepFormatter,
	&v::VFormatter,
];
