mod config;
mod renderer;
mod shortcut;
mod terminal;
mod tray;
mod ui;
mod vte;

use anyhow::Result;
use config::Config;
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

    let toggle_flag = ToggleFlag::default();

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

    std::thread::spawn(move || loop {
        surface.apply_toggle(&qh);
        if let Err(e) = queue.blocking_dispatch(&mut surface) {
            tracing::error!("Wayland dispatch error: {e}");
            break;
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
