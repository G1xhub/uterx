//! Glyph atlas for caching rasterized font glyphs.

use fontdue::Font;
use std::collections::HashMap;

/// A rasterized glyph ready for GPU upload.
#[derive(Debug, Clone)]
pub struct RasterizedGlyph {
    pub bitmap: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub advance_x: f32,
    pub offset_x: f32,
    pub offset_y: f32,
}

/// Region in the atlas texture for a cached glyph.
#[derive(Debug, Clone, Copy)]
pub struct AtlasRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Cache key for a glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub codepoint: char,
    pub size_px: u16,
    pub bold: bool,
    pub italic: bool,
}

/// The glyph atlas manages rasterization and GPU texture packing.
pub struct GlyphAtlas {
    font: Font,
    cache: HashMap<GlyphKey, AtlasRegion>,
    // TODO: actual GPU texture handle, packing algorithm
}

impl GlyphAtlas {
    /// Create a new glyph atlas with the given font data.
    pub fn new(font_data: &[u8]) -> anyhow::Result<Self> {
        let font = Font::from_bytes(font_data, fontdue::FontSettings::default())
            .map_err(|e| anyhow::anyhow!("failed to load font: {}", e))?;
        Ok(Self {
            font,
            cache: HashMap::new(),
        })
    }

    /// Rasterize a glyph (or return cached atlas region).
    pub fn rasterize(&mut self, key: GlyphKey) -> RasterizedGlyph {
        let (metrics, bitmap) = self.font.rasterize(key.codepoint, key.size_px as f32);
        RasterizedGlyph {
            bitmap,
            width: metrics.width as u32,
            height: metrics.height as u32,
            advance_x: metrics.advance_width,
            offset_x: metrics.xmin as f32,
            offset_y: metrics.ymin as f32,
        }
    }

    /// Look up a cached glyph region.
    pub fn get_cached(&self, key: &GlyphKey) -> Option<&AtlasRegion> {
        self.cache.get(key)
    }
}
