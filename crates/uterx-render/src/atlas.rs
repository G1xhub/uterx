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

#[derive(Debug, Clone, Copy)]
struct Shelf {
    y: u32,
    height: u32,
    cursor_x: u32,
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
    atlas_width: u32,
    atlas_height: u32,
    shelves: Vec<Shelf>,
    alpha_pixels: Vec<u8>,
}

impl GlyphAtlas {
    /// Create a new glyph atlas with the given font data.
    pub fn new(font_data: &[u8]) -> anyhow::Result<Self> {
        let font = Font::from_bytes(font_data, fontdue::FontSettings::default())
            .map_err(|e| anyhow::anyhow!("failed to load font: {}", e))?;
        Ok(Self {
            font,
            cache: HashMap::new(),
            atlas_width: 1024,
            atlas_height: 1024,
            shelves: Vec::new(),
            alpha_pixels: vec![0; 1024 * 1024],
        })
    }

    /// Create a new glyph atlas with custom dimensions.
    pub fn with_size(font_data: &[u8], atlas_width: u32, atlas_height: u32) -> anyhow::Result<Self> {
        let font = Font::from_bytes(font_data, fontdue::FontSettings::default())
            .map_err(|e| anyhow::anyhow!("failed to load font: {}", e))?;
        let pixel_count = atlas_width
            .checked_mul(atlas_height)
            .ok_or_else(|| anyhow::anyhow!("atlas size overflow"))?;

        Ok(Self {
            font,
            cache: HashMap::new(),
            atlas_width,
            atlas_height,
            shelves: Vec::new(),
            alpha_pixels: vec![0; pixel_count as usize],
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

    /// Insert glyph into atlas if needed and return its cached region.
    pub fn get_or_insert(&mut self, key: GlyphKey) -> anyhow::Result<AtlasRegion> {
        if let Some(region) = self.cache.get(&key).copied() {
            return Ok(region);
        }

        let glyph = self.rasterize(key);
        let region = self.place_rasterized_glyph(&glyph)?;
        self.write_bitmap(&region, &glyph.bitmap);
        self.cache.insert(key, region);
        Ok(region)
    }

    /// Look up a cached glyph region.
    pub fn get_cached(&self, key: &GlyphKey) -> Option<&AtlasRegion> {
        self.cache.get(key)
    }

    pub fn atlas_size(&self) -> (u32, u32) {
        (self.atlas_width, self.atlas_height)
    }

    pub fn alpha_pixels(&self) -> &[u8] {
        &self.alpha_pixels
    }

    fn place_rasterized_glyph(&mut self, glyph: &RasterizedGlyph) -> anyhow::Result<AtlasRegion> {
        let glyph_w = glyph.width.max(1);
        let glyph_h = glyph.height.max(1);

        if glyph_w > self.atlas_width || glyph_h > self.atlas_height {
            return Err(anyhow::anyhow!(
                "glyph {}x{} exceeds atlas {}x{}",
                glyph_w,
                glyph_h,
                self.atlas_width,
                self.atlas_height
            ));
        }

        if let Some((idx, x, y)) = self.find_shelf_for(glyph_w, glyph_h) {
            self.shelves[idx].cursor_x = x + glyph_w;
            return Ok(AtlasRegion {
                x,
                y,
                width: glyph.width,
                height: glyph.height,
            });
        }

        let new_y = self
            .shelves
            .last()
            .map(|s| s.y + s.height)
            .unwrap_or(0);
        if new_y + glyph_h > self.atlas_height {
            return Err(anyhow::anyhow!(
                "atlas full: cannot place glyph {}x{}",
                glyph_w,
                glyph_h
            ));
        }

        self.shelves.push(Shelf {
            y: new_y,
            height: glyph_h,
            cursor_x: glyph_w,
        });

        Ok(AtlasRegion {
            x: 0,
            y: new_y,
            width: glyph.width,
            height: glyph.height,
        })
    }

    fn find_shelf_for(&self, glyph_w: u32, glyph_h: u32) -> Option<(usize, u32, u32)> {
        let mut best: Option<(usize, u32, u32, u32)> = None;
        for (idx, shelf) in self.shelves.iter().enumerate() {
            if glyph_h > shelf.height {
                continue;
            }
            if shelf.cursor_x + glyph_w > self.atlas_width {
                continue;
            }
            let waste = shelf.height - glyph_h;
            let candidate = (idx, shelf.cursor_x, shelf.y, waste);
            match best {
                Some((_, _, _, best_waste)) if waste >= best_waste => {}
                _ => best = Some(candidate),
            }
        }
        best.map(|(idx, x, y, _)| (idx, x, y))
    }

    fn write_bitmap(&mut self, region: &AtlasRegion, bitmap: &[u8]) {
        if region.width == 0 || region.height == 0 || bitmap.is_empty() {
            return;
        }

        let row_len = region.width as usize;
        for row in 0..region.height as usize {
            let dst_row_start = ((region.y as usize + row) * self.atlas_width as usize)
                + region.x as usize;
            let src_row_start = row * row_len;
            let src_row_end = src_row_start + row_len;
            self.alpha_pixels[dst_row_start..dst_row_start + row_len]
                .copy_from_slice(&bitmap[src_row_start..src_row_end]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "windows")]
    fn load_test_font() -> Vec<u8> {
        std::fs::read("C:/Windows/Fonts/consola.ttf")
            .or_else(|_| std::fs::read("C:/Windows/Fonts/Consolas.ttf"))
            .expect("Consolas font should exist on Windows")
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn inserts_and_caches_glyph_region() {
        let font = load_test_font();
        let mut atlas = GlyphAtlas::with_size(&font, 128, 128).unwrap();
        let key = GlyphKey {
            codepoint: 'A',
            size_px: 16,
            bold: false,
            italic: false,
        };

        let r1 = atlas.get_or_insert(key).unwrap();
        let r2 = atlas.get_or_insert(key).unwrap();
        assert_eq!(r1.x, r2.x);
        assert_eq!(r1.y, r2.y);
        assert!(atlas.get_cached(&key).is_some());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn writes_bitmap_data_into_atlas_pixels() {
        let font = load_test_font();
        let mut atlas = GlyphAtlas::with_size(&font, 128, 128).unwrap();
        let key = GlyphKey {
            codepoint: 'W',
            size_px: 18,
            bold: false,
            italic: false,
        };

        let region = atlas.get_or_insert(key).unwrap();
        if region.width > 0 && region.height > 0 {
            let mut any_non_zero = false;
            for row in 0..region.height as usize {
                let start = (region.y as usize + row) * 128 + region.x as usize;
                let end = start + region.width as usize;
                if atlas.alpha_pixels()[start..end].iter().any(|px| *px != 0) {
                    any_non_zero = true;
                    break;
                }
            }
            assert!(any_non_zero);
        }
    }
}
