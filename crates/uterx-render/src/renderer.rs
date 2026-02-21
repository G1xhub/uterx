//! wgpu-based terminal renderer.

use std::collections::HashSet;

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use uterx_core::{Grid, cell::Color as CoreColor};

const RENDER_SHADER_WGSL: &str = r#"
struct Globals {
    viewport: vec2<f32>,
    _pad: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> globals: Globals;

struct VsOut {
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VsOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(3.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );
    var out: VsOut;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    return out;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    let tint = clamp(globals.viewport.x / 4000.0, 0.0, 0.08);
    return vec4<f32>(0.117 + tint, 0.117 + tint, 0.180 + tint, 1.0);
}
"#;

const LIGATURE_PATTERNS: [&str; 15] = [
    "<==>", "<=>", "==>", "->", "=>", "==", "!=", "<=", ">=", "&&", "||", "::",
    "++", "--", "..",
];

const FLAG_BOLD: u32 = 1 << 0;
const FLAG_ITALIC: u32 = 1 << 1;
const FLAG_UNDERLINE: u32 = 1 << 2;
const FLAG_STRIKETHROUGH: u32 = 1 << 3;
const FLAG_HIDDEN: u32 = 1 << 4;
const FLAG_LIGATURE: u32 = 1 << 5;

/// Runtime GPU state (headless for now; no surface binding yet).
pub struct GpuState {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_info: wgpu::AdapterInfo,
}

/// Pipeline state for the headless render path.
pub struct PipelineState {
    pub target_format: wgpu::TextureFormat,
    pub globals_buffer: wgpu::Buffer,
    pub globals_bind_group: wgpu::BindGroup,
    pub globals_layout: wgpu::BindGroupLayout,
    pub render_pipeline: wgpu::RenderPipeline,
}

pub struct SurfaceState {
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
}

/// Per-cell instance data for future GPU upload.
#[derive(Debug, Clone, PartialEq)]
pub struct CellInstance {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub fg: [f32; 4],
    pub bg: [f32; 4],
    pub codepoint: u32,
    pub text: String,
    pub flags: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorStyle {
    Block,
    Bar,
    Underline,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CursorInstance {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub color: [f32; 4],
    pub style: CursorStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionRange {
    pub start_row: usize,
    pub start_col: usize,
    pub end_row: usize,
    pub end_col: usize,
}

impl SelectionRange {
    pub fn new(start_row: usize, start_col: usize, end_row: usize, end_col: usize) -> Self {
        Self {
            start_row,
            start_col,
            end_row,
            end_col,
        }
    }

    fn normalized(self) -> Self {
        if (self.start_row, self.start_col) <= (self.end_row, self.end_col) {
            self
        } else {
            Self {
                start_row: self.end_row,
                start_col: self.end_col,
                end_row: self.start_row,
                end_col: self.start_col,
            }
        }
    }

    fn contains(self, row: usize, col: usize) -> bool {
        let n = self.normalized();
        if row < n.start_row || row > n.end_row {
            return false;
        }
        if n.start_row == n.end_row {
            return row == n.start_row && col >= n.start_col && col <= n.end_col;
        }
        if row == n.start_row {
            return col >= n.start_col;
        }
        if row == n.end_row {
            return col <= n.end_col;
        }
        true
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectionInstance {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub color: [f32; 4],
}

/// Geometry generated for a single frame from terminal grid state.
#[derive(Debug, Clone, Default)]
pub struct FrameGeometry {
    pub instances: Vec<CellInstance>,
    pub selections: Vec<SelectionInstance>,
    pub cursor: Option<CursorInstance>,
    pub cols: usize,
    pub rows: usize,
}

#[derive(Debug, Clone, Default)]
pub struct DamageReport {
    pub full_redraw: bool,
    pub changed_cells: Vec<(usize, usize)>,
    pub changed_rows: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CellSnapshot {
    content: String,
    width: u8,
    attrs: uterx_core::cell::CellAttributes,
}

/// Configuration for the renderer.
#[derive(Debug, Clone)]
pub struct RendererConfig {
    pub font_size: f32,
    pub cell_width: f32,
    pub cell_height: f32,
    pub cursor_style: CursorStyle,
    pub selection_color: [f32; 4],
    pub scroll_smoothing_factor: f32,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            font_size: 14.0,
            cell_width: 8.0,
            cell_height: 16.0,
            cursor_style: CursorStyle::Block,
            selection_color: [137.0 / 255.0, 180.0 / 255.0, 250.0 / 255.0, 0.35],
            scroll_smoothing_factor: 0.22,
        }
    }
}

/// The GPU renderer. Manages the wgpu device, pipeline, and glyph atlas.
pub struct Renderer {
    config: RendererConfig,
    gpu: Option<GpuState>,
    surface: Option<SurfaceState>,
    pipeline: Option<PipelineState>,
    last_frame: Option<FrameGeometry>,
    last_damage: DamageReport,
    previous_cells: Option<Vec<CellSnapshot>>,
    previous_cols: usize,
    previous_rows: usize,
    previous_cursor: Option<(usize, usize)>,
    selection: Option<SelectionRange>,
    previous_selection: Option<SelectionRange>,
    scroll_offset_px: f32,
    scroll_target_px: f32,
    previous_scroll_offset_px: f32,
}

impl Renderer {
    /// Create a renderer without initializing GPU resources.
    ///
    /// Use `try_init_wgpu()` to request adapter/device/queue.
    pub fn new(config: RendererConfig) -> Self {
        Self {
            config,
            gpu: None,
            surface: None,
            pipeline: None,
            last_frame: None,
            last_damage: DamageReport::default(),
            previous_cells: None,
            previous_cols: 0,
            previous_rows: 0,
            previous_cursor: None,
            selection: None,
            previous_selection: None,
            scroll_offset_px: 0.0,
            scroll_target_px: 0.0,
            previous_scroll_offset_px: 0.0,
        }
    }

    pub fn set_selection(&mut self, selection: SelectionRange) {
        self.selection = Some(selection.normalized());
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    pub fn selection(&self) -> Option<SelectionRange> {
        self.selection
    }

    pub fn scroll_offset_px(&self) -> f32 {
        self.scroll_offset_px
    }

    pub fn scroll_target_px(&self) -> f32 {
        self.scroll_target_px
    }

    pub fn set_scroll_offset_px(&mut self, offset_px: f32) {
        self.scroll_offset_px = offset_px;
        self.scroll_target_px = offset_px;
    }

    pub fn set_scroll_target_px(&mut self, target_px: f32) {
        self.scroll_target_px = target_px;
    }

    pub fn tick_smooth_scroll(&mut self) -> bool {
        let diff = self.scroll_target_px - self.scroll_offset_px;
        if diff.abs() <= 0.01 {
            self.scroll_offset_px = self.scroll_target_px;
            return false;
        }

        let factor = self.config.scroll_smoothing_factor.clamp(0.05, 1.0);
        self.scroll_offset_px += diff * factor;
        if (self.scroll_target_px - self.scroll_offset_px).abs() <= 0.01 {
            self.scroll_offset_px = self.scroll_target_px;
        }
        true
    }

    /// Initialize wgpu in headless mode (no surface/window binding yet).
    pub async fn try_init_wgpu(&mut self) -> anyhow::Result<()> {
        if self.gpu.is_some() {
            return Ok(());
        }

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("failed to request wgpu adapter"))?;

        let adapter_info = adapter.get_info();
        tracing::info!(
            "wgpu adapter selected: {} ({:?})",
            adapter_info.name,
            adapter_info.backend
        );

        let limits = wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits());
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("uterx-render-device"),
                required_features: wgpu::Features::empty(),
                required_limits: limits,
                memory_hints: wgpu::MemoryHints::Performance,
            }, None)
            .await
            .map_err(|e| anyhow::anyhow!("failed to request wgpu device: {}", e))?;

        self.gpu = Some(GpuState {
            instance,
            adapter,
            device,
            queue,
            adapter_info,
        });
        Ok(())
    }

    /// Render the terminal grid to the screen.
    ///
    /// This is a placeholder — the real implementation will:
    /// 1. Walk the grid cells
    /// 2. Look up / rasterize glyphs via the atlas
    /// 3. Build vertex buffers
    /// 4. Submit a wgpu render pass
    pub fn render(&mut self, grid: &Grid) {
        self.tick_smooth_scroll();
        let damage = self.compute_damage(grid);
        let geometry = if damage.full_redraw {
            self.build_frame_geometry(grid)
        } else {
            self.build_damage_geometry(grid, &damage)
        };
        self.last_damage = damage;
        self.last_frame = Some(geometry);
        if self.gpu.is_some() {
            tracing::trace!("render frame (wgpu initialized, render pass TODO)");
        } else {
            tracing::trace!("render frame (placeholder, wgpu not initialized)");
        }
    }

    pub fn config(&self) -> &RendererConfig {
        &self.config
    }

    pub fn gpu_state(&self) -> Option<&GpuState> {
        self.gpu.as_ref()
    }

    pub fn pipeline_state(&self) -> Option<&PipelineState> {
        self.pipeline.as_ref()
    }

    pub fn surface_state(&self) -> Option<&SurfaceState> {
        self.surface.as_ref()
    }

    pub fn last_frame(&self) -> Option<&FrameGeometry> {
        self.last_frame.as_ref()
    }

    pub fn last_damage(&self) -> &DamageReport {
        &self.last_damage
    }

    /// Create a basic render pipeline (shader + bind group + draw state) for a target format.
    pub fn try_init_pipeline(&mut self, target_format: wgpu::TextureFormat) -> anyhow::Result<()> {
        let gpu = self
            .gpu
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("wgpu not initialized; call try_init_wgpu() first"))?;

        if let Some(p) = &self.pipeline {
            if p.target_format == target_format {
                return Ok(());
            }
        }

        let shader = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("uterx-render-shader"),
            source: wgpu::ShaderSource::Wgsl(RENDER_SHADER_WGSL.into()),
        });

        let globals_layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("uterx-globals-layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let globals_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uterx-globals-buffer"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        gpu.queue
            .write_buffer(&globals_buffer, 0, &globals_uniform_bytes(0.0, 0.0));

        let globals_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uterx-globals-bind-group"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("uterx-render-pipeline-layout"),
                bind_group_layouts: &[&globals_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("uterx-render-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        self.pipeline = Some(PipelineState {
            target_format,
            globals_buffer,
            globals_bind_group,
            globals_layout,
            render_pipeline,
        });

        Ok(())
    }

    /// Initialize a presentation surface from a native window/display handle.
    ///
    /// # Safety
    /// The caller must ensure the provided window outlives the renderer's surface usage.
    pub unsafe fn try_init_surface_from_window<W>(&mut self, window: &W) -> anyhow::Result<()>
    where
        W: HasWindowHandle + HasDisplayHandle,
    {
        let gpu = self
            .gpu
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("wgpu not initialized; call try_init_wgpu() first"))?;

        if self.surface.is_some() {
            return Ok(());
        }

        let target = unsafe { wgpu::SurfaceTargetUnsafe::from_window(window) }
            .map_err(|e| anyhow::anyhow!("failed to create surface target: {}", e))?;
        let surface = unsafe { gpu.instance.create_surface_unsafe(target) }
            .map_err(|e| anyhow::anyhow!("failed to create surface: {}", e))?;

        self.surface = Some(SurfaceState {
            surface,
            config: wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
                width: 1,
                height: 1,
                present_mode: wgpu::PresentMode::Fifo,
                alpha_mode: wgpu::CompositeAlphaMode::Auto,
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            },
        });
        Ok(())
    }

    /// Configure surface for current size and adapter capabilities.
    pub fn configure_surface(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        let gpu = self
            .gpu
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("wgpu not initialized; call try_init_wgpu() first"))?;
        let surface_state = self
            .surface
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("surface not initialized"))?;

        let caps = surface_state.surface.get_capabilities(&gpu.adapter);
        let format = choose_surface_format(&caps.formats)
            .ok_or_else(|| anyhow::anyhow!("no surface formats available"))?;
        let present_mode = choose_present_mode(&caps.present_modes)
            .ok_or_else(|| anyhow::anyhow!("no present modes available"))?;
        let alpha_mode = choose_alpha_mode(&caps.alpha_modes)
            .ok_or_else(|| anyhow::anyhow!("no alpha modes available"))?;

        surface_state.config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface_state
            .surface
            .configure(&gpu.device, &surface_state.config);
        Ok(())
    }

    /// Render and present into the configured surface.
    pub fn render_to_surface(
        &mut self,
        grid: &Grid,
        width: u32,
        height: u32,
    ) -> anyhow::Result<()> {
        if self.surface.is_none() {
            return Err(anyhow::anyhow!("surface not initialized"));
        }
        if self.gpu.is_none() {
            return Err(anyhow::anyhow!("wgpu not initialized"));
        }

        let need_reconfigure = {
            let surface_state = self.surface.as_ref().expect("checked above");
            surface_state.config.width != width.max(1)
                || surface_state.config.height != height.max(1)
        };
        if need_reconfigure {
            self.configure_surface(width, height)?;
        }

        let format = self
            .surface
            .as_ref()
            .expect("checked above")
            .config
            .format;
        self.try_init_pipeline(format)?;
        self.update_viewport_uniform(width as f32, height as f32);
        self.render(grid);

        let frame = {
            let surface_state = self.surface.as_mut().expect("checked above");
            match surface_state.surface.get_current_texture() {
                Ok(frame) => frame,
                Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                    self.configure_surface(width, height)?;
                    return Ok(());
                }
                Err(wgpu::SurfaceError::Timeout) => {
                    tracing::debug!("surface timeout; skipping frame");
                    return Ok(());
                }
                Err(wgpu::SurfaceError::OutOfMemory) => {
                    return Err(anyhow::anyhow!("surface out of memory"));
                }
                Err(wgpu::SurfaceError::Other) => {
                    tracing::warn!("surface error: other");
                    return Ok(());
                }
            }
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let gpu = self.gpu.as_ref().expect("checked above");
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("uterx-surface-encoder"),
            });
        self.encode_render_pass(&mut encoder, &view)?;
        gpu.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }

    /// Update viewport uniform (used by shader/pipeline skeleton).
    pub fn update_viewport_uniform(&self, width: f32, height: f32) {
        let Some(gpu) = &self.gpu else {
            return;
        };
        let Some(pipeline) = &self.pipeline else {
            return;
        };
        gpu.queue.write_buffer(
            &pipeline.globals_buffer,
            0,
            &globals_uniform_bytes(width, height),
        );
    }

    /// Encode a minimal draw call into the provided target view.
    pub fn encode_render_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
    ) -> anyhow::Result<()> {
        let pipeline = self
            .pipeline
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("render pipeline not initialized"))?;

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("uterx-render-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.10,
                        g: 0.10,
                        b: 0.12,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&pipeline.render_pipeline);
        pass.set_bind_group(0, &pipeline.globals_bind_group, &[]);
        pass.draw(0..3, 0..1);
        Ok(())
    }

    /// Build per-cell frame geometry from grid content.
    pub fn build_frame_geometry(&self, grid: &Grid) -> FrameGeometry {
        let cols = grid.cols();
        let rows = grid.rows();
        let mut instances = Vec::with_capacity(cols.saturating_mul(rows));

        for row in 0..rows {
            instances.extend(self.build_row_instances(grid, row));
        }

        FrameGeometry {
            instances,
            selections: self.build_selection_instances(grid, None),
            cursor: self.build_cursor_instance(grid),
            cols,
            rows,
        }
    }

    pub fn compute_damage(&mut self, grid: &Grid) -> DamageReport {
        let cols = grid.cols();
        let rows = grid.rows();
        let size_changed = self.previous_cells.is_none()
            || self.previous_cols != cols
            || self.previous_rows != rows;

        let snapshots = snapshot_grid_cells(grid);
        let selection_changed = self.selection != self.previous_selection;
        let scroll_changed =
            (self.scroll_offset_px - self.previous_scroll_offset_px).abs() > 0.01;
        if size_changed {
            self.previous_cells = Some(snapshots);
            self.previous_cols = cols;
            self.previous_rows = rows;
            self.previous_selection = self.selection;
            self.previous_scroll_offset_px = self.scroll_offset_px;
            return DamageReport {
                full_redraw: true,
                changed_cells: Vec::new(),
                changed_rows: (0..rows).collect(),
            };
        }

        if selection_changed {
            self.previous_cells = Some(snapshots);
            self.previous_cols = cols;
            self.previous_rows = rows;
            self.previous_selection = self.selection;
            self.previous_scroll_offset_px = self.scroll_offset_px;
            self.previous_cursor = Some((
                grid.cursor_row.min(rows.saturating_sub(1)),
                grid.cursor_col.min(cols.saturating_sub(1)),
            ));
            return DamageReport {
                full_redraw: true,
                changed_cells: Vec::new(),
                changed_rows: (0..rows).collect(),
            };
        }

        if scroll_changed {
            self.previous_cells = Some(snapshots);
            self.previous_cols = cols;
            self.previous_rows = rows;
            self.previous_selection = self.selection;
            self.previous_scroll_offset_px = self.scroll_offset_px;
            self.previous_cursor = Some((
                grid.cursor_row.min(rows.saturating_sub(1)),
                grid.cursor_col.min(cols.saturating_sub(1)),
            ));
            return DamageReport {
                full_redraw: true,
                changed_cells: Vec::new(),
                changed_rows: (0..rows).collect(),
            };
        }

        let previous = match &self.previous_cells {
            Some(prev) => prev,
            None => {
                self.previous_cells = Some(snapshots);
                self.previous_cols = cols;
                self.previous_rows = rows;
                self.previous_scroll_offset_px = self.scroll_offset_px;
                return DamageReport {
                    full_redraw: true,
                    changed_cells: Vec::new(),
                    changed_rows: (0..rows).collect(),
                };
            }
        };

        let mut changed_cells = Vec::new();
        let mut changed_rows = Vec::new();
        for row in 0..rows {
            let mut row_changed = false;
            for col in 0..cols {
                let idx = row * cols + col;
                if snapshots[idx] != previous[idx] {
                    changed_cells.push((row, col));
                    row_changed = true;
                }
            }
            if row_changed {
                changed_rows.push(row);
            }
        }

        let current_cursor = (
            grid.cursor_row.min(rows.saturating_sub(1)),
            grid.cursor_col.min(cols.saturating_sub(1)),
        );
        if let Some((prev_row, prev_col)) = self.previous_cursor {
            if (prev_row, prev_col) != current_cursor {
                push_unique_cell(&mut changed_cells, prev_row, prev_col, rows, cols);
                push_unique_cell(
                    &mut changed_cells,
                    current_cursor.0,
                    current_cursor.1,
                    rows,
                    cols,
                );
                push_unique_row(&mut changed_rows, prev_row, rows);
                push_unique_row(&mut changed_rows, current_cursor.0, rows);
            }
        }

        self.previous_cells = Some(snapshots);
        self.previous_cols = cols;
        self.previous_rows = rows;
        self.previous_cursor = Some(current_cursor);
        self.previous_selection = self.selection;
        self.previous_scroll_offset_px = self.scroll_offset_px;

        DamageReport {
            full_redraw: false,
            changed_cells,
            changed_rows,
        }
    }

    pub fn build_damage_geometry(&self, grid: &Grid, damage: &DamageReport) -> FrameGeometry {
        if damage.full_redraw {
            return self.build_frame_geometry(grid);
        }

        let mut instances = Vec::new();
        let mut consumed = HashSet::new();

        for row in &damage.changed_rows {
            if *row >= grid.rows() {
                continue;
            }
            if self.row_contains_ligature(grid, *row) {
                instances.extend(self.build_row_instances(grid, *row));
                for col in 0..grid.cols() {
                    consumed.insert((*row, col));
                }
            }
        }

        for (row, col) in &damage.changed_cells {
            if consumed.contains(&(*row, *col)) {
                continue;
            }
            let Some(cell) = grid.cell(*row, *col) else {
                continue;
            };
            instances.push(self.build_cell_instance(*row, *col, cell));
        }

        FrameGeometry {
            instances,
            selections: self.build_selection_instances(grid, Some(damage)),
            cursor: self.build_cursor_instance(grid),
            cols: grid.cols(),
            rows: grid.rows(),
        }
    }

    fn build_cell_instance(
        &self,
        row: usize,
        col: usize,
        cell: &uterx_core::Cell,
    ) -> CellInstance {
        let mut fg = cell.attrs.fg;
        let mut bg = cell.attrs.bg;
        if cell.attrs.inverse {
            std::mem::swap(&mut fg, &mut bg);
        }

        let codepoint = if cell.attrs.hidden {
            ' ' as u32
        } else {
            cell.content.chars().next().unwrap_or(' ') as u32
        };
        let text = if cell.attrs.hidden {
            " ".to_string()
        } else {
            cell.content.clone()
        };

        let flags = cell_style_flags(cell);

        let width_cols = cell.width.max(1) as f32;
        CellInstance {
            pos: [
                col as f32 * self.config.cell_width,
                self.row_y(row),
            ],
            size: [
                self.config.cell_width * width_cols,
                self.config.cell_height,
            ],
            fg: resolve_color(fg, true),
            bg: resolve_color(bg, false),
            codepoint,
            text,
            flags,
        }
    }

    fn build_row_instances(&self, grid: &Grid, row: usize) -> Vec<CellInstance> {
        let cols = grid.cols();
        let mut out = Vec::with_capacity(cols);
        let mut col = 0usize;

        while col < cols {
            if let Some((pattern, len)) = self.match_ligature_at(grid, row, col) {
                if let Some(instance) = self.build_ligature_instance(grid, row, col, pattern, len) {
                    out.push(instance);
                    col += len;
                    continue;
                }
            }

            if let Some(cell) = grid.cell(row, col) {
                out.push(self.build_cell_instance(row, col, cell));
            }
            col += 1;
        }

        out
    }

    fn row_contains_ligature(&self, grid: &Grid, row: usize) -> bool {
        let cols = grid.cols();
        for col in 0..cols {
            if self.match_ligature_at(grid, row, col).is_some() {
                return true;
            }
        }
        false
    }

    fn match_ligature_at(&self, grid: &Grid, row: usize, start_col: usize) -> Option<(&'static str, usize)> {
        let cols = grid.cols();
        if start_col >= cols {
            return None;
        }

        for pattern in LIGATURE_PATTERNS {
            let len = pattern.chars().count();
            if start_col + len > cols {
                continue;
            }
            if self.cells_match_pattern(grid, row, start_col, pattern) {
                return Some((pattern, len));
            }
        }
        None
    }

    fn cells_match_pattern(&self, grid: &Grid, row: usize, start_col: usize, pattern: &str) -> bool {
        let mut chars = pattern.chars();
        let Some(first_cell) = grid.cell(row, start_col) else {
            return false;
        };
        if first_cell.attrs.hidden || first_cell.width != 1 {
            return false;
        }
        let base_attrs = first_cell.attrs;

        for (offset, pattern_char) in chars.by_ref().enumerate() {
            let col = start_col + offset;
            let Some(cell) = grid.cell(row, col) else {
                return false;
            };
            if cell.attrs.hidden || cell.width != 1 {
                return false;
            }
            if cell.attrs != base_attrs {
                return false;
            }
            if cell.content.chars().next().unwrap_or(' ') != pattern_char {
                return false;
            }
        }

        true
    }

    fn build_ligature_instance(
        &self,
        grid: &Grid,
        row: usize,
        col: usize,
        pattern: &str,
        len: usize,
    ) -> Option<CellInstance> {
        let cell = grid.cell(row, col)?;
        let mut fg = cell.attrs.fg;
        let mut bg = cell.attrs.bg;
        if cell.attrs.inverse {
            std::mem::swap(&mut fg, &mut bg);
        }

        Some(CellInstance {
            pos: [
                col as f32 * self.config.cell_width,
                self.row_y(row),
            ],
            size: [self.config.cell_width * len as f32, self.config.cell_height],
            fg: resolve_color(fg, true),
            bg: resolve_color(bg, false),
            codepoint: pattern.chars().next().unwrap_or(' ') as u32,
            text: pattern.to_string(),
            flags: cell_style_flags(cell) | FLAG_LIGATURE,
        })
    }

    fn build_cursor_instance(&self, grid: &Grid) -> Option<CursorInstance> {
        if grid.rows() == 0 || grid.cols() == 0 {
            return None;
        }

        let row = grid.cursor_row.min(grid.rows().saturating_sub(1));
        let col = grid.cursor_col.min(grid.cols().saturating_sub(1));
        let x = col as f32 * self.config.cell_width;
        let y = self.row_y(row);

        let (w, h) = match self.config.cursor_style {
            CursorStyle::Block => (self.config.cell_width, self.config.cell_height),
            CursorStyle::Bar => (self.config.cell_width.max(1.0) * 0.14, self.config.cell_height),
            CursorStyle::Underline => (self.config.cell_width, self.config.cell_height.max(1.0) * 0.12),
        };

        let cursor_y = match self.config.cursor_style {
            CursorStyle::Underline => y + (self.config.cell_height - h).max(0.0),
            _ => y,
        };

        Some(CursorInstance {
            pos: [x, cursor_y],
            size: [w.max(1.0), h.max(1.0)],
            color: [166.0 / 255.0, 227.0 / 255.0, 161.0 / 255.0, 1.0],
            style: self.config.cursor_style,
        })
    }

    fn build_selection_instances(
        &self,
        grid: &Grid,
        damage: Option<&DamageReport>,
    ) -> Vec<SelectionInstance> {
        let Some(selection) = self.selection else {
            return Vec::new();
        };

        let mut selections = Vec::new();
        match damage {
            Some(d) if !d.full_redraw => {
                for (row, col) in &d.changed_cells {
                    if selection.contains(*row, *col) {
                        selections.push(self.selection_instance_at(*row, *col));
                    }
                }
            }
            _ => {
                for row in 0..grid.rows() {
                    for col in 0..grid.cols() {
                        if selection.contains(row, col) {
                            selections.push(self.selection_instance_at(row, col));
                        }
                    }
                }
            }
        }
        selections
    }

    fn selection_instance_at(&self, row: usize, col: usize) -> SelectionInstance {
        SelectionInstance {
            pos: [
                col as f32 * self.config.cell_width,
                self.row_y(row),
            ],
            size: [self.config.cell_width, self.config.cell_height],
            color: self.config.selection_color,
        }
    }

    fn row_y(&self, row: usize) -> f32 {
        row as f32 * self.config.cell_height - self.scroll_offset_px
    }
}

fn snapshot_grid_cells(grid: &Grid) -> Vec<CellSnapshot> {
    let mut snapshots = Vec::with_capacity(grid.cols().saturating_mul(grid.rows()));
    for row in 0..grid.rows() {
        for col in 0..grid.cols() {
            if let Some(cell) = grid.cell(row, col) {
                snapshots.push(CellSnapshot {
                    content: cell.content.clone(),
                    width: cell.width,
                    attrs: cell.attrs,
                });
            }
        }
    }
    snapshots
}

fn cell_style_flags(cell: &uterx_core::Cell) -> u32 {
    let mut flags = 0u32;
    if cell.attrs.bold {
        flags |= FLAG_BOLD;
    }
    if cell.attrs.italic {
        flags |= FLAG_ITALIC;
    }
    if cell.attrs.underline {
        flags |= FLAG_UNDERLINE;
    }
    if cell.attrs.strikethrough {
        flags |= FLAG_STRIKETHROUGH;
    }
    if cell.attrs.hidden {
        flags |= FLAG_HIDDEN;
    }
    flags
}

fn push_unique_cell(
    changed_cells: &mut Vec<(usize, usize)>,
    row: usize,
    col: usize,
    rows: usize,
    cols: usize,
) {
    if row >= rows || col >= cols {
        return;
    }
    if !changed_cells.contains(&(row, col)) {
        changed_cells.push((row, col));
    }
}

fn push_unique_row(changed_rows: &mut Vec<usize>, row: usize, rows: usize) {
    if row >= rows {
        return;
    }
    if !changed_rows.contains(&row) {
        changed_rows.push(row);
    }
}

fn globals_uniform_bytes(width: f32, height: f32) -> [u8; 16] {
    let mut bytes = [0u8; 16];
    bytes[0..4].copy_from_slice(&width.to_ne_bytes());
    bytes[4..8].copy_from_slice(&height.to_ne_bytes());
    bytes
}

fn choose_surface_format(formats: &[wgpu::TextureFormat]) -> Option<wgpu::TextureFormat> {
    formats
        .iter()
        .copied()
        .find(|f| *f == wgpu::TextureFormat::Bgra8UnormSrgb)
        .or_else(|| {
            formats
                .iter()
                .copied()
                .find(|f| *f == wgpu::TextureFormat::Rgba8UnormSrgb)
        })
        .or_else(|| formats.first().copied())
}

fn choose_present_mode(modes: &[wgpu::PresentMode]) -> Option<wgpu::PresentMode> {
    modes
        .iter()
        .copied()
        .find(|m| *m == wgpu::PresentMode::Mailbox)
        .or_else(|| modes.iter().copied().find(|m| *m == wgpu::PresentMode::Fifo))
        .or_else(|| modes.first().copied())
}

fn choose_alpha_mode(modes: &[wgpu::CompositeAlphaMode]) -> Option<wgpu::CompositeAlphaMode> {
    modes
        .iter()
        .copied()
        .find(|m| *m == wgpu::CompositeAlphaMode::Auto)
        .or_else(|| modes.iter().copied().find(|m| *m == wgpu::CompositeAlphaMode::Opaque))
        .or_else(|| modes.first().copied())
}

fn resolve_color(color: CoreColor, is_foreground: bool) -> [f32; 4] {
    match color {
        CoreColor::Rgb(r, g, b) => [
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            1.0,
        ],
        CoreColor::Indexed(idx) => indexed_color(idx),
        CoreColor::Default => {
            if is_foreground {
                [205.0 / 255.0, 214.0 / 255.0, 244.0 / 255.0, 1.0]
            } else {
                [30.0 / 255.0, 30.0 / 255.0, 46.0 / 255.0, 1.0]
            }
        }
    }
}

fn indexed_color(idx: u8) -> [f32; 4] {
    const ANSI16: [[u8; 3]; 16] = [
        [0, 0, 0],
        [205, 49, 49],
        [13, 188, 121],
        [229, 229, 16],
        [36, 114, 200],
        [188, 63, 188],
        [17, 168, 205],
        [229, 229, 229],
        [102, 102, 102],
        [241, 76, 76],
        [35, 209, 139],
        [245, 245, 67],
        [59, 142, 234],
        [214, 112, 214],
        [41, 184, 219],
        [255, 255, 255],
    ];

    if idx < 16 {
        let rgb = ANSI16[idx as usize];
        return [
            rgb[0] as f32 / 255.0,
            rgb[1] as f32 / 255.0,
            rgb[2] as f32 / 255.0,
            1.0,
        ];
    }

    if (16..=231).contains(&idx) {
        let i = idx - 16;
        let r = i / 36;
        let g = (i % 36) / 6;
        let b = i % 6;
        let to_component = |v: u8| if v == 0 { 0 } else { v * 40 + 55 };
        return [
            to_component(r) as f32 / 255.0,
            to_component(g) as f32 / 255.0,
            to_component(b) as f32 / 255.0,
            1.0,
        ];
    }

    let gray = 8 + (idx.saturating_sub(232) as u16 * 10) as u8;
    [
        gray as f32 / 255.0,
        gray as f32 / 255.0,
        gray as f32 / 255.0,
        1.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use uterx_core::cell::Color as CoreColor;

    #[test]
    fn builds_geometry_for_each_grid_cell() {
        let mut grid = Grid::new(2, 2);
        grid.write_char('A');

        let renderer = Renderer::new(RendererConfig::default());
        let geom = renderer.build_frame_geometry(&grid);

        assert_eq!(geom.cols, 2);
        assert_eq!(geom.rows, 2);
        assert_eq!(geom.instances.len(), 4);
        assert!(geom.selections.is_empty());
        assert_eq!(geom.instances[0].codepoint, 'A' as u32);
        assert_eq!(geom.instances[0].text, "A");
        assert!(geom.cursor.is_some());
    }

    #[test]
    fn inverse_swaps_colors() {
        let mut grid = Grid::new(1, 1);
        if let Some(cell) = grid.cell_mut(0, 0) {
            cell.content = "X".to_string();
            cell.attrs.fg = CoreColor::Rgb(255, 0, 0);
            cell.attrs.bg = CoreColor::Rgb(0, 0, 255);
            cell.attrs.inverse = true;
        }

        let renderer = Renderer::new(RendererConfig::default());
        let geom = renderer.build_frame_geometry(&grid);
        let inst = &geom.instances[0];

        assert_eq!(inst.fg, [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(inst.bg, [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn damage_tracking_detects_only_changed_cells() {
        let mut grid = Grid::new(2, 1);
        let mut renderer = Renderer::new(RendererConfig::default());

        let first = renderer.compute_damage(&grid);
        assert!(first.full_redraw);

        let second = renderer.compute_damage(&grid);
        assert!(!second.full_redraw);
        assert!(second.changed_cells.is_empty());

        if let Some(cell) = grid.cell_mut(0, 1) {
            cell.content = "Z".to_string();
        }

        let third = renderer.compute_damage(&grid);
        assert!(!third.full_redraw);
        assert_eq!(third.changed_cells, vec![(0, 1)]);
        assert_eq!(third.changed_rows, vec![0]);
    }

    #[test]
    fn damage_geometry_contains_only_dirty_instances() {
        let mut grid = Grid::new(3, 1);
        let mut renderer = Renderer::new(RendererConfig::default());

        let _ = renderer.compute_damage(&grid);
        if let Some(cell) = grid.cell_mut(0, 2) {
            cell.content = "Q".to_string();
        }

        let damage = renderer.compute_damage(&grid);
        let geom = renderer.build_damage_geometry(&grid, &damage);
        assert_eq!(geom.instances.len(), 1);
        assert!(geom.selections.is_empty());
        assert_eq!(geom.instances[0].codepoint, 'Q' as u32);
        assert_eq!(geom.instances[0].text, "Q");
        assert!(geom.cursor.is_some());
    }

    #[test]
    fn globals_uniform_layout_is_stable() {
        let bytes = globals_uniform_bytes(1920.0, 1080.0);
        assert_eq!(bytes.len(), 16);
        let mut x = [0u8; 4];
        let mut y = [0u8; 4];
        x.copy_from_slice(&bytes[0..4]);
        y.copy_from_slice(&bytes[4..8]);
        assert_eq!(f32::from_ne_bytes(x), 1920.0);
        assert_eq!(f32::from_ne_bytes(y), 1080.0);
    }

    #[test]
    fn surface_choice_prefers_srgb_and_mailbox() {
        let formats = [
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureFormat::Bgra8UnormSrgb,
        ];
        let modes = [wgpu::PresentMode::Fifo, wgpu::PresentMode::Mailbox];
        let alpha = [wgpu::CompositeAlphaMode::Opaque, wgpu::CompositeAlphaMode::Auto];

        assert_eq!(
            choose_surface_format(&formats),
            Some(wgpu::TextureFormat::Bgra8UnormSrgb)
        );
        assert_eq!(
            choose_present_mode(&modes),
            Some(wgpu::PresentMode::Mailbox)
        );
        assert_eq!(
            choose_alpha_mode(&alpha),
            Some(wgpu::CompositeAlphaMode::Auto)
        );
    }

    #[test]
    fn cursor_style_affects_cursor_geometry() {
        let grid = Grid::new(2, 1);
        let renderer = Renderer::new(RendererConfig {
            cursor_style: CursorStyle::Bar,
            ..RendererConfig::default()
        });
        let geom = renderer.build_frame_geometry(&grid);
        let cursor = geom.cursor.expect("cursor should exist");
        assert_eq!(cursor.style, CursorStyle::Bar);
        assert!(cursor.size[0] < renderer.config().cell_width);
    }

    #[test]
    fn selection_adds_overlay_instances() {
        let grid = Grid::new(4, 2);
        let mut renderer = Renderer::new(RendererConfig::default());
        renderer.set_selection(SelectionRange::new(0, 1, 0, 2));

        let geom = renderer.build_frame_geometry(&grid);
        assert_eq!(geom.selections.len(), 2);
        assert_eq!(geom.selections[0].pos, [renderer.config().cell_width, 0.0]);
    }

    #[test]
    fn selection_change_triggers_full_redraw_damage() {
        let grid = Grid::new(3, 1);
        let mut renderer = Renderer::new(RendererConfig::default());

        let d0 = renderer.compute_damage(&grid);
        assert!(d0.full_redraw);

        let d1 = renderer.compute_damage(&grid);
        assert!(!d1.full_redraw);

        renderer.set_selection(SelectionRange::new(0, 0, 0, 1));
        let d2 = renderer.compute_damage(&grid);
        assert!(d2.full_redraw);
    }

    #[test]
    fn ligature_run_is_grouped_into_single_instance() {
        let mut grid = Grid::new(4, 1);
        if let Some(cell) = grid.cell_mut(0, 0) {
            cell.content = "-".to_string();
        }
        if let Some(cell) = grid.cell_mut(0, 1) {
            cell.content = ">".to_string();
        }
        if let Some(cell) = grid.cell_mut(0, 2) {
            cell.content = "X".to_string();
        }

        let renderer = Renderer::new(RendererConfig::default());
        let geom = renderer.build_frame_geometry(&grid);

        assert_eq!(geom.instances.len(), 3);
        assert_eq!(geom.instances[0].text, "->");
        assert_eq!(geom.instances[0].size[0], renderer.config().cell_width * 2.0);
        assert_ne!(geom.instances[0].flags & FLAG_LIGATURE, 0);
    }

    #[test]
    fn ligature_row_damage_rebuilds_row_geometry() {
        let mut grid = Grid::new(3, 1);
        let mut renderer = Renderer::new(RendererConfig::default());

        let _ = renderer.compute_damage(&grid);
        if let Some(cell) = grid.cell_mut(0, 0) {
            cell.content = "-".to_string();
        }
        if let Some(cell) = grid.cell_mut(0, 1) {
            cell.content = ">".to_string();
        }

        let damage = renderer.compute_damage(&grid);
        let geom = renderer.build_damage_geometry(&grid, &damage);

        assert_eq!(geom.instances.len(), 2);
        assert_eq!(geom.instances[0].text, "->");
    }

    #[test]
    fn smooth_scroll_offsets_instance_positions() {
        let mut grid = Grid::new(1, 1);
        if let Some(cell) = grid.cell_mut(0, 0) {
            cell.content = "A".to_string();
        }

        let mut renderer = Renderer::new(RendererConfig::default());
        renderer.set_scroll_offset_px(6.0);

        let geom = renderer.build_frame_geometry(&grid);
        assert_eq!(geom.instances[0].pos[1], -6.0);
    }

    #[test]
    fn smooth_scroll_change_triggers_full_redraw_damage() {
        let grid = Grid::new(2, 1);
        let mut renderer = Renderer::new(RendererConfig::default());

        let d0 = renderer.compute_damage(&grid);
        assert!(d0.full_redraw);

        let d1 = renderer.compute_damage(&grid);
        assert!(!d1.full_redraw);

        renderer.set_scroll_offset_px(8.0);
        let d2 = renderer.compute_damage(&grid);
        assert!(d2.full_redraw);
    }
}
