# Phase 3: GPU Rendering

**Status**: ✅ ~100% Complete

**Goal**: High-performance GPU-accelerated rendering at 60fps.

## Overview

Phase 3 implements GPU-based rendering using wgpu, providing smooth 60fps performance with damage tracking, ligature rendering, and smooth scrolling. The rendering runs on a dedicated thread to keep the UI responsive.

## Components

### uterx-render

The GPU rendering crate.

#### Key Types

- [`Renderer`](../../crates/uterx-render/src/renderer.rs) - Main renderer with wgpu integration
- [`GlyphAtlas`](../../crates/uterx-render/src/atlas.rs) - Texture atlas for glyph caching
- [`RenderThread`](../../crates/uterx-render/src/render_thread.rs) - Dedicated render thread

## Completed Features

### wgpu Initialization

#### GPU State

```rust
pub struct GpuState {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_info: wgpu::AdapterInfo,
}

impl Renderer {
    pub async fn try_init_wgpu() -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("No suitable GPU adapter found"))?;

        let adapter_info = adapter.get_info();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("uterx-device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await?;

        Ok(Self {
            gpu: Some(GpuState {
                instance,
                adapter,
                device,
                queue,
                adapter_info,
            }),
            ..Default::default()
        })
    }
}
```

### Glyph Atlas

#### Atlas Structure

```rust
pub struct GlyphAtlas {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    shelves: Vec<Shelf>,
    cache: HashMap<GlyphKey, GlyphEntry>,
}

pub struct GlyphKey {
    pub c: char,
    pub font_size: u32,
    pub bold: bool,
    pub italic: bool,
}

pub struct GlyphEntry {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
```

#### Shelf Packing Algorithm

```rust
impl GlyphAtlas {
    pub fn get_or_insert(&mut self, key: GlyphKey, bitmap: &[u8], width: u32, height: u32) -> GlyphEntry {
        if let Some(entry) = self.cache.get(&key) {
            return *entry;
        }

        // Find best-fit shelf
        let (shelf_idx, shelf) = self.shelves
            .iter_mut()
            .enumerate()
            .find(|(_, s)| s.height >= height && s.x + width <= self.width)
            .unwrap_or_else(|| {
                // Create new shelf
                let y = self.shelves.last().map_or(0, |s| s.y + s.height);
                let shelf = Shelf {
                    y,
                    height,
                    x: 0,
                };
                self.shelves.push(shelf);
                (self.shelves.len() - 1, self.shelves.last_mut().unwrap())
            });

        let entry = GlyphEntry {
            x: shelf.x,
            y: shelf.y,
            width,
            height,
        };

        // Copy bitmap into atlas
        for row in 0..height {
            let atlas_row = (shelf.y + row) as usize;
            let start = (shelf.x + row * self.width) as usize;
            let end = start + width as usize;
            self.pixels[start..end].copy_from_slice(&bitmap[(row * width) as usize..((row + 1) * width) as usize]);
        }

        shelf.x += width;
        self.cache.insert(key, entry);
        entry
    }
}
```

### Frame Geometry

#### Cell Instance

```rust
#[repr(C)]
pub struct CellInstance {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub fg_color: [f32; 4],
    pub bg_color: [f32; 4],
    pub uv: [f32; 2],
    pub uv_size: [f32; 2],
    pub codepoint: u32,
    pub style_flags: u32,
}

pub struct FrameGeometry {
    pub cells: Vec<CellInstance>,
    pub ligatures: Vec<CellInstance>,
    pub selections: Vec<SelectionInstance>,
    pub cursor: Option<CursorInstance>,
}
```

#### Geometry Builder

```rust
impl Renderer {
    pub fn build_frame_geometry(&self, grid: &Grid, selection: Option<SelectionRange>) -> FrameGeometry {
        let mut cells = Vec::new();
        let mut ligatures = Vec::new();
        let mut selections = Vec::new();

        for row in 0..grid.height {
            let mut ligature_run = String::new();
            let mut ligature_start = 0;

            for col in 0..grid.width {
                if let Some(cell) = grid.get(col, row) {
                    ligature_run.push(cell.c);

                    if is_ligature_pattern(&ligature_run) {
                        continue;
                    }

                    if ligature_run.len() > 1 {
                        // Emit ligature
                        ligatures.push(CellInstance {
                            position: [ligature_start as f32, row as f32],
                            size: [(col - ligature_start + 1) as f32, 1.0],
                            fg_color: cell.attrs.foreground.to_rgba(),
                            bg_color: cell.attrs.background.to_rgba(),
                            uv: [0.0, 0.0],
                            uv_size: [1.0, 1.0],
                            codepoint: 0,
                            style_flags: cell.attrs.to_flags(),
                        });
                    } else {
                        // Emit single cell
                        cells.push(CellInstance {
                            position: [col as f32, row as f32],
                            size: [1.0, 1.0],
                            fg_color: cell.attrs.foreground.to_rgba(),
                            bg_color: cell.attrs.background.to_rgba(),
                            uv: [0.0, 0.0],
                            uv_size: [1.0, 1.0],
                            codepoint: cell.c as u32,
                            style_flags: cell.attrs.to_flags(),
                        });
                    }

                    ligature_run.clear();
                    ligature_start = col + 1;
                }
            }
        }

        // Add selection geometry
        if let Some(sel) = selection {
            selections.extend(self.build_selection_geometry(grid, &sel));
        }

        // Add cursor geometry
        if let Some(cursor) = self.build_cursor_geometry(grid) {
            selections.push(cursor);
        }

        FrameGeometry {
            cells,
            ligatures,
            selections,
            cursor: None,
        }
    }
}
```

### Damage Tracking

#### Damage Report

```rust
pub struct DamageReport {
    pub full_redraw: bool,
    pub dirty_cells: Vec<(u16, u16)>,
    pub dirty_rows: Vec<u16>,
}

impl Renderer {
    pub fn compute_damage(&self, grid: &Grid, prev_grid: Option<&Grid>) -> DamageReport {
        let mut report = DamageReport {
            full_redraw: false,
            dirty_cells: Vec::new(),
            dirty_rows: Vec::new(),
        };

        // Check for full redraw conditions
        if prev_grid.is_none() || grid.width != prev_grid.unwrap().width || grid.height != prev_grid.unwrap().height {
            report.full_redraw = true;
            return report;
        }

        let prev = prev_grid.unwrap();

        // Compare cells
        for row in 0..grid.height {
            let mut row_dirty = false;
            for col in 0..grid.width {
                let current = grid.get(col, row);
                let previous = prev.get(col, row);

                if current != previous {
                    report.dirty_cells.push((col, row));
                    row_dirty = true;
                }
            }
            if row_dirty {
                report.dirty_rows.push(row);
            }
        }

        report
    }

    pub fn build_damage_geometry(&self, grid: &Grid, damage: &DamageReport) -> FrameGeometry {
        if damage.full_redraw {
            return self.build_frame_geometry(grid, None);
        }

        let mut cells = Vec::new();

        for &(col, row) in &damage.dirty_cells {
            if let Some(cell) = grid.get(col, row) {
                cells.push(CellInstance {
                    position: [col as f32, row as f32],
                    size: [1.0, 1.0],
                    fg_color: cell.attrs.foreground.to_rgba(),
                    bg_color: cell.attrs.background.to_rgba(),
                    uv: [0.0, 0.0],
                    uv_size: [1.0, 1.0],
                    codepoint: cell.c as u32,
                    style_flags: cell.attrs.to_flags(),
                });
            }
        }

        FrameGeometry {
            cells,
            ligatures: Vec::new(),
            selections: Vec::new(),
            cursor: None,
        }
    }
}
```

### Render Pipeline

#### WGSL Shader

```wgsl
struct Uniforms {
    viewport: vec2<f32>,
    cell_size: vec2<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var texture: texture_2d<f32>;
@group(0) @binding(2) var sampler: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vertex_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(
        (input.position / uniforms.viewport) * 2.0 - 1.0,
        -((input.position / uniforms.viewport) * 2.0 - 1.0),
        0.0,
        1.0,
    );
    output.uv = input.uv;
    output.color = input.color;
    return output;
}

@fragment
fn fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(texture, sampler, input.uv);
    return tex_color * input.color;
}
```

#### Pipeline Creation

```rust
impl Renderer {
    pub fn try_init_pipeline(&mut self, format: wgpu::TextureFormat) -> Result<()> {
        let gpu = self.gpu.as_ref().ok_or_else(|| anyhow::anyhow!("GPU not initialized"))?;

        let shader = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("uterx-shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_CODE.into()),
        });

        let bind_group_layout = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uterx-bind-group"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("uterx-pipeline-layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("uterx-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vertex_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<CellInstance>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fragment_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        self.pipeline = Some(pipeline);
        self.bind_group_layout = Some(bind_group_layout);
        Ok(())
    }
}
```

### Dedicated Render Thread

#### Render Thread Command

```rust
pub enum RenderCommand {
    UpdateGrid { grid: Grid, selection: Option<SelectionRange> },
    Flush,
    Shutdown,
}

pub struct RenderThreadHandle {
    tx: mpsc::UnboundedSender<RenderCommand>,
}

impl RenderThreadHandle {
    pub fn send_grid(&self, grid: Grid, selection: Option<SelectionRange>) {
        let _ = self.tx.send(RenderCommand::UpdateGrid { grid, selection });
    }

    pub fn flush(&self) {
        let _ = self.tx.send(RenderCommand::Flush);
    }

    pub fn shutdown(&self) {
        let _ = self.tx.send(RenderCommand::Shutdown);
    }
}

pub fn start_render_thread() -> RenderThreadHandle {
    let (tx, rx) = mpsc::unbounded_channel();

    thread::spawn(move || {
        let mut renderer = Renderer::default();
        let mut last_grid: Option<Grid> = None;
        let mut last_selection: Option<SelectionRange> = None;

        while let Ok(cmd) = rx.recv() {
            match cmd {
                RenderCommand::UpdateGrid { grid, selection } => {
                    let damage = renderer.compute_damage(&grid, last_grid.as_ref());
                    let geometry = if damage.full_redraw {
                        renderer.build_frame_geometry(&grid, selection.as_ref())
                    } else {
                        renderer.build_damage_geometry(&grid, &damage)
                    };

                    // Upload geometry and render
                    renderer.render_geometry(&geometry);

                    last_grid = Some(grid);
                    last_selection = selection;
                }
                RenderCommand::Flush => {
                    // Present frame
                }
                RenderCommand::Shutdown => {
                    break;
                }
            }
        }
    });

    RenderThreadHandle { tx }
}
```

### Cursor Rendering

#### Cursor Styles

```rust
pub enum CursorStyle {
    Block,
    Bar,
    Underline,
}

#[repr(C)]
pub struct CursorInstance {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub color: [f32; 4],
}

impl Renderer {
    pub fn build_cursor_geometry(&self, grid: &Grid) -> Option<CursorInstance> {
        let cursor = grid.cursor;
        let style = self.config.cursor_style;

        let (width, height) = match style {
            CursorStyle::Block => (1.0, 1.0),
            CursorStyle::Bar => (0.2, 1.0),
            CursorStyle::Underline => (1.0, 0.2),
        };

        Some(CursorInstance {
            position: [cursor.col as f32, cursor.row as f32],
            size: [width, height],
            color: [1.0, 1.0, 1.0, 1.0], // White cursor
        })
    }
}
```

### Selection Highlighting

#### Selection Geometry

```rust
pub struct SelectionRange {
    pub start: (u16, u16),
    pub end: (u16, u16),
}

#[repr(C)]
pub struct SelectionInstance {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub color: [f32; 4],
}

impl Renderer {
    pub fn build_selection_geometry(&self, grid: &Grid, selection: &SelectionRange) -> Vec<SelectionInstance> {
        let mut instances = Vec::new();

        let (start_col, start_row) = selection.start;
        let (end_col, end_row) = selection.end;

        for row in start_row..=end_row {
            let col_start = if row == start_row { start_col } else { 0 };
            let col_end = if row == end_row { end_col } else { grid.width - 1 };

            instances.push(SelectionInstance {
                position: [col_start as f32, row as f32],
                size: [(col_end - col_start + 1) as f32, 1.0],
                color: [0.3, 0.5, 0.8, 0.5], // Blue selection
            });
        }

        instances
    }
}
```

### Smooth Scrolling

#### Scroll State

```rust
pub struct Renderer {
    pub scroll_offset_px: f32,
    pub scroll_target_px: f32,
}

impl Renderer {
    pub fn tick_smooth_scroll(&mut self, dt: f32) {
        let diff = self.scroll_target_px - self.scroll_offset_px;
        self.scroll_offset_px += diff * self.config.scroll_smoothing_factor * dt;
    }

    pub fn apply_scroll_offset(&self, y: f32) -> f32 {
        y + self.scroll_offset_px
    }
}
```

## Testing

### Unit Tests

16 unit tests passing:

```bash
cargo test -p uterx-render
```

Tests cover:
- Geometry count and content
- Inverse color swapping
- No-op frames
- Single-cell diffs
- Dirty instance generation
- Cursor overlay generation
- Selection overlay generation
- Selection-change damage behavior
- Full-frame ligature grouping
- Damage-path ligature behavior
- Offset application
- Scroll-change damage behavior
- Atlas cache behavior
- Atlas bitmap write validation
- Render thread processing

## Related Documentation

- [Architecture](./architecture.md) - Overall architecture
- [Getting Started](./getting-started.md) - User guide
- [Phase 1](./phase1-core-terminal.md) - Core terminal
