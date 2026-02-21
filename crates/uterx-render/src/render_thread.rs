//! Dedicated render thread with channel-based grid updates.

use crate::renderer::{Renderer, RendererConfig};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use uterx_core::Grid;

enum RenderThreadCommand {
    UpdateGrid(Grid),
    Flush(mpsc::Sender<()>),
    Shutdown,
}

#[derive(Debug, Clone, Default)]
pub struct RenderThreadStats {
    pub frames_processed: u64,
    pub last_frame_instances: usize,
    pub last_changed_cells: usize,
}

pub struct RenderThreadHandle {
    tx: mpsc::Sender<RenderThreadCommand>,
    stats: Arc<Mutex<RenderThreadStats>>,
    join_handle: Option<JoinHandle<()>>,
}

impl RenderThreadHandle {
    pub fn start(config: RendererConfig) -> Self {
        let (tx, rx) = mpsc::channel::<RenderThreadCommand>();
        let stats = Arc::new(Mutex::new(RenderThreadStats::default()));
        let stats_for_thread = Arc::clone(&stats);

        let join_handle = thread::spawn(move || {
            let mut renderer = Renderer::new(config);
            while let Ok(cmd) = rx.recv() {
                match cmd {
                    RenderThreadCommand::UpdateGrid(grid) => {
                        renderer.render(&grid);
                        if let Ok(mut s) = stats_for_thread.lock() {
                            s.frames_processed += 1;
                            s.last_frame_instances = renderer
                                .last_frame()
                                .map(|f| f.instances.len())
                                .unwrap_or(0);
                            s.last_changed_cells = renderer.last_damage().changed_cells.len();
                        }
                    }
                    RenderThreadCommand::Flush(done_tx) => {
                        let _ = done_tx.send(());
                    }
                    RenderThreadCommand::Shutdown => break,
                }
            }
        });

        Self {
            tx,
            stats,
            join_handle: Some(join_handle),
        }
    }

    pub fn send_grid(&self, grid: Grid) -> anyhow::Result<()> {
        self.tx
            .send(RenderThreadCommand::UpdateGrid(grid))
            .map_err(|e| anyhow::anyhow!("failed to send grid to render thread: {}", e))
    }

    pub fn flush(&self) -> anyhow::Result<()> {
        let (done_tx, done_rx) = mpsc::channel();
        self.tx
            .send(RenderThreadCommand::Flush(done_tx))
            .map_err(|e| anyhow::anyhow!("failed to flush render thread: {}", e))?;
        done_rx
            .recv()
            .map_err(|e| anyhow::anyhow!("failed waiting for render thread flush: {}", e))?;
        Ok(())
    }

    pub fn stats(&self) -> RenderThreadStats {
        self.stats
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| RenderThreadStats::default())
    }

    pub fn shutdown(&mut self) {
        let _ = self.tx.send(RenderThreadCommand::Shutdown);
        if let Some(join) = self.join_handle.take() {
            let _ = join.join();
        }
    }
}

impl Drop for RenderThreadHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_thread_processes_grid_updates() {
        let mut handle = RenderThreadHandle::start(RendererConfig::default());
        let mut grid = Grid::new(4, 2);
        grid.write_char('A');
        let mut grid2 = grid.clone();
        if let Some(cell) = grid2.cell_mut(0, 1) {
            cell.content = "B".to_string();
        }

        handle.send_grid(grid).unwrap();
        handle.send_grid(grid2).unwrap();
        handle.flush().unwrap();

        let stats = handle.stats();
        assert!(stats.frames_processed >= 2);
        assert!(stats.last_frame_instances > 0);

        handle.shutdown();
    }
}
