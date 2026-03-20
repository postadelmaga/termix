use alacritty_terminal::{
    event::{Event, EventListener, WindowSize},
    event_loop::{EventLoop, Msg, Notifier},
    grid::Dimensions,
    sync::FairMutex,
    term::{Config as TermConfig, Term},
    tty,
};

/// Minimal Dimensions implementation for Term::new.
struct TermDimensions {
    columns: usize,
    screen_lines: usize,
}

impl Dimensions for TermDimensions {
    fn columns(&self) -> usize { self.columns }
    fn screen_lines(&self) -> usize { self.screen_lines }
    fn total_lines(&self) -> usize { self.screen_lines }
}
use anyhow::{Context, Result};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

// ── Event handler ────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct TermEventHandler {
    pub dirty: Arc<AtomicBool>,
}

impl EventListener for TermEventHandler {
    fn send_event(&self, event: Event) {
        match event {
            Event::Wakeup | Event::Bell | Event::ResetTitle | Event::Exit => {
                self.dirty.store(true, Ordering::Relaxed);
            }
            _ => {}
        }
    }
}

// ── TerminalState ────────────────────────────────────────────────────────────

pub struct TerminalState {
    pub term: Arc<FairMutex<Term<TermEventHandler>>>,
    #[allow(dead_code)]
    pub dirty: Arc<AtomicBool>,
    #[allow(dead_code)]
    notifier: Notifier,
}

impl TerminalState {
    pub fn new(cols: u16, rows: u16, shell: &str) -> Result<Self> {
        let dirty = Arc::new(AtomicBool::new(true));

        let event_handler = TermEventHandler {
            dirty: dirty.clone(),
        };

        let pty_options = tty::Options {
            shell: Some(tty::Shell::new(shell.to_string(), vec![])),
            ..Default::default()
        };

        let window_size = WindowSize {
            num_lines: rows,
            num_cols: cols,
            cell_width: 10,
            cell_height: 20,
        };

        let pty = tty::new(&pty_options, window_size, 0)
            .context("failed to create PTY")?;

        let term_size = TermDimensions {
            columns: cols as usize,
            screen_lines: rows as usize,
        };
        let term = Arc::new(FairMutex::new(Term::new(
            TermConfig::default(),
            &term_size,
            event_handler.clone(),
        )));

        let event_loop = EventLoop::new(
            term.clone(),
            event_handler,
            pty,
            false,
            false,
        )
        .context("failed to create terminal event loop")?;

        let notifier = Notifier(event_loop.channel());
        event_loop.spawn();

        Ok(Self { term, dirty, notifier })
    }

    /// Send keyboard input bytes to the shell.
    pub fn write_input(&self, data: &[u8]) {
        let _ = self.notifier.0.send(Msg::Input(data.to_vec().into()));
    }

    /// Notify the PTY of a terminal resize.
    #[allow(dead_code)]
    pub fn resize(&self, cols: u16, rows: u16) {
        let size = TermDimensions {
            columns: cols as usize,
            screen_lines: rows as usize,
        };
        self.term.lock().resize(size);
        let _ = self.notifier.0.send(Msg::Resize(WindowSize {
            num_lines: rows,
            num_cols: cols,
            cell_width: 10,
            cell_height: 20,
        }));
    }

    /// Returns true (and clears the flag) if the terminal needs redrawing.
    #[allow(dead_code)]
    pub fn take_dirty(&self) -> bool {
        self.dirty.swap(false, Ordering::Relaxed)
    }
}
