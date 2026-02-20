//! uterx-render — GPU-accelerated terminal rendering via wgpu.
//!
//! This crate provides the rendering pipeline for the terminal,
//! including glyph rasterization, atlas management, and damage tracking.

pub mod atlas;
pub mod renderer;

pub use renderer::Renderer;
