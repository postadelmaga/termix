use anyhow::{Context, Result};
use portable_pty::{Child, CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

/// A PTY session running a shell (zellij by default).
///
/// - `write_input` sends bytes (keyboard) to the process stdin
/// - `read_output` returns pending bytes from stdout (call in a loop/thread)
/// - `resize` sends SIGWINCH with new dimensions
pub struct TerminalPty {
    master: Box<dyn MasterPty + Send>,
    _child: Box<dyn Child + Send + Sync>,
    writer: Box<dyn Write + Send>,
    reader: Arc<Mutex<Box<dyn Read + Send>>>,
}

impl TerminalPty {
    pub fn spawn(cols: u16, rows: u16, shell: &str) -> Result<Self> {
        let pty_system = NativePtySystem::default();

        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("failed to open PTY")?;

        let mut cmd = CommandBuilder::new(shell);
        // TERM must be set so ncurses/zellij work correctly
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");

        let child = pair
            .slave
            .spawn_command(cmd)
            .context("failed to spawn shell in PTY")?;

        let writer = pair.master.take_writer().context("failed to get PTY writer")?;
        let reader = pair.master.try_clone_reader().context("failed to clone PTY reader")?;

        Ok(Self {
            master: pair.master,
            _child: child,
            writer,
            reader: Arc::new(Mutex::new(reader)),
        })
    }

    /// Send bytes to the shell (keyboard input).
    pub fn write_input(&mut self, data: &[u8]) -> Result<()> {
        self.writer.write_all(data).context("PTY write failed")?;
        self.writer.flush().context("PTY flush failed")?;
        Ok(())
    }

    /// Returns a cloned reader handle for use in a read thread.
    pub fn reader(&self) -> Arc<Mutex<Box<dyn Read + Send>>> {
        self.reader.clone()
    }

    /// Notify the shell of a terminal resize.
    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("PTY resize failed")?;
        Ok(())
    }
}
