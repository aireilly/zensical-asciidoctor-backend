//! Zensical module for AsciiDoc support.
//!
//! This crate provides a Zensical module that renders AsciiDoc files using
//! the `asciidoctor` CLI and post-processes the HTML output for Material
//! theme compatibility.

pub mod config;
pub mod html;
pub mod renderer;

pub use config::Config;
