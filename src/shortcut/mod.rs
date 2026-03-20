/// Global shortcut registration via KDE's KGlobalAccel DBus service.
///
/// KGlobalAccel exposes `org.kde.KGlobalAccel` on the session bus.
/// We register a component "termix" with action "toggle" and listen
/// for the `yourShortcutGotChanged` / `invokeAction` signals.
///
/// Protocol flow:
///   1. acquireComponent("termix")
///   2. setShortcut("termix", "toggle", [key], flags)
///   3. listen for invokeAction("termix", "toggle", timestamp)
use anyhow::{Context, Result};
use zbus::{proxy, Connection};

use crate::ui::ToggleFlag;

// ── KGlobalAccel DBus proxy ──────────────────────────────────────────────────

#[proxy(
    interface = "org.kde.KGlobalAccel",
    default_service = "org.kde.kglobalaccel",
    default_path = "/component/termix"
)]
trait KGlobalAccelComponent {
    /// Register a shortcut for this component.
    /// key_sequence: e.g. "F12"
    fn set_shortcut(
        &self,
        action_id: &[&str],         // [component, unique_name, friendly_name, action_id]
        keys: &[u32],               // Qt key codes; empty = use default
        default_keys: &[u32],
        flags: u32,
    ) -> zbus::Result<Vec<u32>>;
}

#[proxy(
    interface = "org.kde.KGlobalAccel",
    default_service = "org.kde.kglobalaccel",
    default_path = "/kglobalaccel"
)]
trait KGlobalAccel {
    fn acquire_component(&self, component_info: &[&str]) -> zbus::Result<bool>;

    #[zbus(signal)]
    fn your_shortcut_got_changed(
        &self,
        action_id: Vec<String>,
        keys: Vec<u32>,
    ) -> zbus::Result<()>;
}

// ── Qt key code helpers ──────────────────────────────────────────────────────

/// Map a key name (from config) to Qt::Key integer value.
fn qt_key(name: &str) -> Option<u32> {
    // Common keys — extend as needed
    match name {
        "F1"  => Some(0x01000030),
        "F2"  => Some(0x01000031),
        "F3"  => Some(0x01000032),
        "F4"  => Some(0x01000033),
        "F5"  => Some(0x01000034),
        "F6"  => Some(0x01000035),
        "F7"  => Some(0x01000036),
        "F8"  => Some(0x01000037),
        "F9"  => Some(0x01000038),
        "F10" => Some(0x01000039),
        "F11" => Some(0x0100003a),
        "F12" => Some(0x0100003b),
        _     => None,
    }
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Register `shortcut_name` (e.g. "F12") with KGlobalAccel and spawn a task
/// that calls `toggle_flag.trigger()` each time the key is pressed.
pub async fn register_and_listen(shortcut_name: String, toggle_flag: ToggleFlag) -> Result<()> {
    let conn = Connection::session()
        .await
        .context("failed to connect to DBus session bus")?;

    let accel = KGlobalAccelProxy::new(&conn)
        .await
        .context("failed to create KGlobalAccel proxy")?;

    // Register our component
    accel
        .acquire_component(&["termix", "Termix"])
        .await
        .context("acquire_component failed")?;

    let key_code = qt_key(&shortcut_name)
        .with_context(|| format!("unsupported shortcut key: {shortcut_name}"))?;

    let component_proxy = KGlobalAccelComponentProxy::builder(&conn)
        .path("/component/termix")
        .context("bad path")?
        .build()
        .await
        .context("failed to build component proxy")?;

    // action_id: [component_unique, action_unique, friendly_component, friendly_action]
    let action_id = ["termix", "toggle", "Termix", "Toggle dropdown"];

    component_proxy
        .set_shortcut(&action_id, &[key_code], &[key_code], 0x2) // 0x2 = SetPresent
        .await
        .context("set_shortcut failed")?;

    tracing::info!("Global shortcut registered: {shortcut_name}");

    // Listen for activations via the invokeAction signal on /component/termix
    // KGlobalAccel calls org.kde.kglobalaccel.Component.invokeAction on our path
    let rule = format!(
        "type='signal',interface='org.kde.kglobalaccel.Component',\
         member='invokeAction',path='/component/termix'"
    );
    conn.monitor_activity();

    // Use zbus message stream for the signal
    use futures_util::StreamExt;
    let mut stream = zbus::MessageStream::from(&conn);

    while let Some(msg) = stream.next().await {
        let Ok(msg) = msg else { continue };
        let hdr = msg.header();
        if hdr.member().map(|m| m.as_str()) == Some("invokeAction") {
            tracing::info!("Global shortcut activated — toggling");
            toggle_flag.trigger();
        }
    }

    Ok(())
}
