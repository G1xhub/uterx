//! PTY (pseudo-terminal) abstraction using portable-pty.

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::io::Write;

/// Wrapper that provides async read via spawn_blocking.
pub struct PtyReader {
    reader: std::sync::Arc<std::sync::Mutex<Box<dyn std::io::Read + Send>>>,
}

impl PtyReader {
    /// Async read from the PTY. Runs blocking read on Tokio's blocking pool.
    pub async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let reader = self.reader.clone();
        let buf_len = buf.len();
        let result = tokio::task::spawn_blocking(move || {
            let mut tmp = vec![0u8; buf_len];
            let mut guard = reader.lock().unwrap();
            let n = guard.read(&mut tmp)?;
            Ok::<(Vec<u8>, usize), std::io::Error>((tmp, n))
        })
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))??;

        buf[..result.1].copy_from_slice(&result.0[..result.1]);
        Ok(result.1)
    }
}

/// A spawned PTY with reader/writer handles.
pub struct PtyProcess {
    master: Box<dyn MasterPty + Send>,
    reader: Option<std::sync::Arc<std::sync::Mutex<Box<dyn std::io::Read + Send>>>>,
    writer: Box<dyn Write + Send>,
    _child: Box<dyn portable_pty::Child + Send>,
}

impl PtyProcess {
    /// Spawn a new PTY running the given shell command.
    pub fn spawn(shell: &str, cols: u16, rows: u16) -> anyhow::Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd = CommandBuilder::new(shell);
        // Inherit environment
        for (key, val) in std::env::vars() {
            cmd.env(key, val);
        }

        let child = pair.slave.spawn_command(cmd)?;
        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;

        Ok(Self {
            master: pair.master,
            reader: Some(std::sync::Arc::new(std::sync::Mutex::new(reader))),
            writer,
            _child: child,
        })
    }

    /// Resize the PTY.
    pub fn resize(&self, cols: u16, rows: u16) -> anyhow::Result<()> {
        self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        Ok(())
    }

    /// Take the async PTY reader. Can only be called once.
    /// Returns a `PtyReader` that can be used with `tokio::spawn`.
    pub fn take_reader(&mut self) -> PtyReader {
        PtyReader {
            reader: self
                .reader
                .take()
                .expect("PTY reader already taken"),
        }
    }

    /// Write bytes to the PTY (sends input to the shell).
    pub fn write(&mut self, data: &[u8]) -> anyhow::Result<()> {
        self.writer.write_all(data)?;
        self.writer.flush()?;
        Ok(())
    }

    /// Get a mutable reference to the PTY writer (for sending input).
    pub fn writer(&mut self) -> &mut dyn Write {
        &mut *self.writer
    }
}
