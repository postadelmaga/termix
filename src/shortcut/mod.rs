/// Global shortcut via KDE KGlobalAccel D-Bus (kglobalaccel6).
///
/// KGlobalAccel6 API:
///   service: org.kde.kglobalaccel
///   path:    /kglobalaccel
///   method:  setShortcut(as actionId, au keys, au defaultKeys, u flags) -> au
///
/// actionId = [component_unique, action_unique, friendly_component, friendly_action]
///
/// When the shortcut fires, kglobalaccel calls invokeShortcut(name) on the
/// component object at /component/{unique_name}. We serve that path.
use anyhow::{Context, Result};
use zbus::{connection, interface, proxy};

use crate::ui::ToggleFlag;

// ── KGlobalAccel root proxy ──────────────────────────────────────────────────

#[proxy(
    interface = "org.kde.KGlobalAccel",
    default_service = "org.kde.kglobalaccel",
    default_path = "/kglobalaccel"
)]
trait KGlobalAccel {
    fn set_shortcut(
        &self,
        action_id: &[&str],      // [component, action, friendlyComponent, friendlyAction]
        keys: &[u32],            // active Qt key codes
        default_keys: &[u32],
        flags: u32,
    ) -> zbus::Result<Vec<u32>>;
}

// ── Component object served at /component/termix ─────────────────────────────

struct TermixComponent {
    toggle_flag: ToggleFlag,
}

#[interface(name = "org.kde.kglobalaccel.Component")]
impl TermixComponent {
    /// Called by kglobalaccel when our registered shortcut is pressed.
    fn invoke_shortcut(&self, name: &str) {
        tracing::info!("invoke_shortcut: {name}");
        if name == "toggle" {
            self.toggle_flag.trigger();
        }
    }

    fn friendly_name(&self) -> &str { "Termix" }
    fn unique_name(&self) -> &str  { "termix" }
    fn shortcut_names(&self) -> Vec<String> { vec!["toggle".to_string()] }

    #[zbus(signal)]
    async fn global_shortcut_accepted(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        action_id: std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
        timestamp: u32,
    ) -> zbus::Result<()>;
}

// ── Qt key code helpers ──────────────────────────────────────────────────────

fn qt_key(name: &str) -> Option<u32> {
    match name {
        "F1"  => Some(0x01000030), "F2"  => Some(0x01000031),
        "F3"  => Some(0x01000032), "F4"  => Some(0x01000033),
        "F5"  => Some(0x01000034), "F6"  => Some(0x01000035),
        "F7"  => Some(0x01000036), "F8"  => Some(0x01000037),
        "F9"  => Some(0x01000038), "F10" => Some(0x01000039),
        "F11" => Some(0x0100003a), "F12" => Some(0x0100003b),
        _     => None,
    }
}

// ── Public entry point ───────────────────────────────────────────────────────

pub async fn register_and_listen(shortcut_name: String, toggle_flag: ToggleFlag) -> Result<()> {
    let key_code = qt_key(&shortcut_name)
        .with_context(|| format!("unsupported shortcut key: {shortcut_name}"))?;

    // Build a connection that serves /component/termix
    let conn = connection::Builder::session()
        .context("DBus session builder")?
        .serve_at("/component/termix", TermixComponent { toggle_flag })
        .context("serve component")?
        .build()
        .await
        .context("build DBus connection for shortcut")?;

    // Register the shortcut with kglobalaccel
    let accel = KGlobalAccelProxy::new(&conn)
        .await
        .context("KGlobalAccel proxy")?;

    let action_id = ["termix", "toggle", "Termix", "Toggle dropdown"];
    match accel.set_shortcut(&action_id, &[key_code], &[key_code], 0x2).await {
        Ok(assigned) => tracing::info!("Global shortcut registered: {shortcut_name} (assigned keys: {assigned:?})"),
        Err(e)       => tracing::warn!("set_shortcut failed ({e}) — configure F12 manually in KDE System Settings → Shortcuts"),
    }

    // Keep the connection alive so /component/termix stays served
    std::future::pending::<()>().await;
    Ok(())
}
