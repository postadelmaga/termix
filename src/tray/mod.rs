/// System tray icon via KDE StatusNotifierItem (SNI) DBus protocol.
///
/// Protocol flow:
///   1. Claim a well-known name: org.kde.StatusNotifierItem-{pid}-1
///   2. Expose org.kde.StatusNotifierItem at /StatusNotifierItem
///   3. Register with org.kde.StatusNotifierWatcher
///   4. Handle Activate (click) → toggle dropdown
use anyhow::{Context, Result};
use zbus::{connection, interface, object_server::SignalContext, proxy};

use crate::ui::ToggleFlag;

// ── StatusNotifierWatcher proxy ──────────────────────────────────────────────

#[proxy(
    interface = "org.kde.StatusNotifierWatcher",
    default_service = "org.kde.StatusNotifierWatcher",
    default_path = "/StatusNotifierWatcher"
)]
trait StatusNotifierWatcher {
    fn register_status_notifier_item(&self, service: &str) -> zbus::Result<()>;
}

// ── StatusNotifierItem implementation ───────────────────────────────────────

struct TrayIcon {
    toggle_flag: ToggleFlag,
}

#[interface(name = "org.kde.StatusNotifierItem")]
impl TrayIcon {
    // ── Properties ──────────────────────────────────────────────────────────

    #[zbus(property)]
    fn id(&self) -> &str {
        "termix"
    }

    #[zbus(property)]
    fn title(&self) -> &str {
        "Termix"
    }

    #[zbus(property)]
    fn status(&self) -> &str {
        // "Active" keeps the icon always visible in the tray
        "Active"
    }

    #[zbus(property)]
    fn icon_name(&self) -> &str {
        "utilities-terminal"
    }

    #[zbus(property)]
    fn tooltip(&self) -> (String, Vec<(i32, i32, Vec<u8>)>, String, String) {
        (
            "utilities-terminal".to_string(),
            vec![],
            "Termix".to_string(),
            "Dropdown terminal — click to toggle".to_string(),
        )
    }

    #[zbus(property)]
    fn category(&self) -> &str {
        "ApplicationStatus"
    }

    #[zbus(property)]
    fn window_id(&self) -> u32 {
        0
    }

    // ── Methods ─────────────────────────────────────────────────────────────

    /// Left-click: toggle the dropdown.
    fn activate(&self, _x: i32, _y: i32) {
        tracing::info!("Tray: Activate — toggling dropdown");
        self.toggle_flag.trigger();
    }

    /// Middle-click: also toggles.
    fn secondary_activate(&self, _x: i32, _y: i32) {
        self.toggle_flag.trigger();
    }

    fn scroll(&self, _delta: i32, _orientation: &str) {}

    // ── Signals ─────────────────────────────────────────────────────────────

    #[zbus(signal)]
    async fn new_icon(ctxt: &SignalContext<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn new_tooltip(ctxt: &SignalContext<'_>) -> zbus::Result<()>;
}

// ── Public entry point ───────────────────────────────────────────────────────

pub async fn run(toggle_flag: ToggleFlag) -> Result<()> {
    let service_name = format!(
        "org.kde.StatusNotifierItem-{}-1",
        std::process::id()
    );

    let conn = connection::Builder::session()
        .context("DBus session builder failed")?
        .name(service_name.as_str())
        .context("failed to claim SNI service name")?
        .serve_at("/StatusNotifierItem", TrayIcon { toggle_flag })
        .context("failed to serve StatusNotifierItem")?
        .build()
        .await
        .context("failed to build DBus connection for tray")?;

    // Register with the plasma tray watcher
    let watcher = StatusNotifierWatcherProxy::new(&conn)
        .await
        .context("failed to create StatusNotifierWatcher proxy")?;

    watcher
        .register_status_notifier_item(&service_name)
        .await
        .context("failed to register with StatusNotifierWatcher")?;

    tracing::info!("Tray icon registered as {service_name}");

    // Keep the connection alive indefinitely
    std::future::pending::<()>().await;
    Ok(())
}
