mod config;
mod renderer;
mod shortcut;
mod terminal;
mod tray;
mod ui;
mod vte;

use anyhow::Result;
use config::Config;
use std::io::Read;
use std::os::fd::{AsFd, AsRawFd};
use ui::{DropdownSurface, ToggleFlag};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let config = Config::load()?;
    tracing::info!(
        "Config loaded: shortcut={}, backend={:?}",
        config.shortcut,
        config.terminal
    );

    let (toggle_flag, wakeup_rx) = ToggleFlag::new();

    // ── Wayland surface ─────────────────────────────────────────────────────
    let (mut surface, mut queue) =
        DropdownSurface::new(config.height_percent, config.opacity, toggle_flag.clone())?;

    let qh = queue.handle();
    surface.create_surface(&qh);

    // ── Terminal (VTE) ──────────────────────────────────────────────────────
    let term_state = vte::TerminalState::new(220, 50, "zellij")?;
    let renderer = renderer::Renderer::new(16.0)?;
    surface.set_terminal(term_state, renderer);
    tracing::info!("Terminal spawned");

    // ── Wayland event loop with wakeup pipe ─────────────────────────────────
    // We poll both the Wayland socket and a wakeup pipe so toggle events from
    // the tokio side (shortcut / tray click) wake us up immediately.
    std::thread::spawn(move || {
        let wayland_fd = queue.as_fd().as_raw_fd();
        let _ = &queue; // ensure borrow is live
        let wakeup_fd = wakeup_rx.as_raw_fd();
        let mut drain = [0u8; 64];

        loop {
            surface.apply_toggle(&qh);
            queue.dispatch_pending(&mut surface).expect("dispatch_pending");
            queue.flush().expect("flush");

            // Poll both fds — block until either has data
            let mut fds = [
                libc::pollfd { fd: wayland_fd, events: libc::POLLIN, revents: 0 },
                libc::pollfd { fd: wakeup_fd,  events: libc::POLLIN, revents: 0 },
            ];
            unsafe { libc::poll(fds.as_mut_ptr(), 2, -1) };

            // Drain the wakeup pipe
            if fds[1].revents & libc::POLLIN != 0 {
                let _ = (&wakeup_rx).read(&mut drain);
            }

            // Read Wayland events if available
            if fds[0].revents & libc::POLLIN != 0 {
                if let Some(guard) = queue.prepare_read() {
                    let _ = guard.read();
                }
            }
        }
    });

    // ── Global shortcut ─────────────────────────────────────────────────────
    let shortcut_key = config.shortcut.clone();
    let toggle_for_shortcut = toggle_flag.clone();
    tokio::spawn(async move {
        if let Err(e) = shortcut::register_and_listen(shortcut_key, toggle_for_shortcut).await {
            tracing::error!("Global shortcut error: {e}");
        }
    });

    // ── System tray ─────────────────────────────────────────────────────────
    let toggle_for_tray = toggle_flag.clone();
    tokio::spawn(async move {
        if let Err(e) = tray::run(toggle_for_tray).await {
            tracing::error!("Tray error: {e}");
        }
    });

    tracing::info!("termix running — press {} to toggle (Ctrl-C to quit)", config.shortcut);
    tokio::signal::ctrl_c().await?;

    Ok(())
}
