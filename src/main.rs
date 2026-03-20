mod config;
mod terminal;
mod ui;

use anyhow::Result;
use config::Config;
use terminal::Terminal;
use ui::DropdownWindow;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let config = Config::load()?;
    tracing::info!("Config loaded: shortcut={}, backend={:?}", config.shortcut, config.terminal);

    let window = DropdownWindow::new(config.height_percent, config.opacity, config.animation_ms);
    window.init()?;

    let mut terminal = Terminal::new(&config);
    terminal.spawn()?;

    // TODO (#5): register global shortcut via KDE DBus (zbus)
    // TODO (#4): run Wayland event loop

    tracing::info!("termix running — press {} to toggle", config.shortcut);
    tokio::signal::ctrl_c().await?;

    Ok(())
}
