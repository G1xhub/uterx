//! uterx-render — GPU-accelerated terminal rendering via wgpu.
//!
//! This crate provides the rendering pipeline for the terminal,
//! including glyph rasterization, atlas management, and damage tracking.

pub mod atlas;
pub mod render_thread;
pub mod renderer;

pub use render_thread::{RenderThreadHandle, RenderThreadStats};
pub use renderer::{
	CursorInstance, CursorStyle, GpuState, Renderer, RendererConfig, SurfaceState,
};
