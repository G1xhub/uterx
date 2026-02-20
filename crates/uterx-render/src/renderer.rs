//! wgpu-based terminal renderer.

use uterx_core::Grid;

/// Configuration for the renderer.
#[derive(Debug, Clone)]
pub struct RendererConfig {
    pub font_size: f32,
    pub cell_width: f32,
    pub cell_height: f32,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            font_size: 14.0,
            cell_width: 8.0,
            cell_height: 16.0,
        }
    }
}

/// The GPU renderer. Manages the wgpu device, pipeline, and glyph atlas.
pub struct Renderer {
    config: RendererConfig,
    // TODO: wgpu device, queue, surface, pipeline, atlas
}

impl Renderer {
    /// Create a new renderer (placeholder — real init requires a window/surface).
    pub fn new(config: RendererConfig) -> Self {
        Self { config }
    }

    /// Render the terminal grid to the screen.
    ///
    /// This is a placeholder — the real implementation will:
    /// 1. Walk the grid cells
    /// 2. Look up / rasterize glyphs via the atlas
    /// 3. Build vertex buffers
    /// 4. Submit a wgpu render pass
    pub fn render(&mut self, _grid: &Grid) {
        // TODO: implement wgpu render pass
        tracing::trace!("render frame (placeholder)");
    }

    pub fn config(&self) -> &RendererConfig {
        &self.config
    }
}
