mod config;
mod shortcut;
mod terminal;
mod ui;

use anyhow::Result;
use config::Config;
use terminal::Terminal;
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

    // Shared flag: toggled by the global shortcut (tokio side),
    // consumed by the Wayland event loop (dedicated thread).
    let toggle_flag = ToggleFlag::default();

    // ── Wayland thread ──────────────────────────────────────────────────────
    let (mut surface, mut queue) =
        DropdownSurface::new(config.height_percent, config.opacity, toggle_flag.clone())?;

    let qh = queue.handle();
    surface.create_surface(&qh);

    std::thread::spawn(move || {
        loop {
            // Check toggle from shortcut side
            surface.apply_toggle(&qh);

            // Dispatch Wayland events (blocking up to 16ms)
            if let Err(e) = queue.blocking_dispatch(&mut surface) {
                tracing::error!("Wayland dispatch error: {e}");
                break;
            }
        }
    });

    // ── Terminal process ────────────────────────────────────────────────────
    let mut terminal = Terminal::new(&config);
    terminal.spawn()?;

    // ── Main loop ───────────────────────────────────────────────────────────
    // ── Global shortcut ─────────────────────────────────────────────────────
    let shortcut_key = config.shortcut.clone();
    let toggle_for_shortcut = toggle_flag.clone();
    tokio::spawn(async move {
        if let Err(e) = shortcut::register_and_listen(shortcut_key, toggle_for_shortcut).await {
            tracing::error!("Global shortcut error: {e}");
        }
    });

    tracing::info!("termix running — press {} to toggle (Ctrl-C to quit)", config.shortcut);
    tokio::signal::ctrl_c().await?;
    tracing::info!("shutting down");
    Ok(())
}
