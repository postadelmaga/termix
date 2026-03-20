/// Manages the Wayland layer-shell surface used as the dropdown overlay.
///
/// The surface is anchored to the top of the screen, full width, and
/// configured as an overlay layer so it stays above all other windows.
/// Actual terminal content is rendered by the child process (foot/kitty);
/// this surface provides the dropdown chrome and animation.
use anyhow::Result;

pub struct DropdownWindow {
    pub height_percent: u8,
    pub opacity: f32,
    pub animation_ms: u64,
    pub visible: bool,
}

impl DropdownWindow {
    pub fn new(height_percent: u8, opacity: f32, animation_ms: u64) -> Self {
        Self {
            height_percent,
            opacity,
            animation_ms,
            visible: false,
        }
    }

    /// Toggle dropdown visibility. Returns new visible state.
    pub fn toggle(&mut self) -> bool {
        self.visible = !self.visible;
        self.visible
    }

    /// Initialize the Wayland layer-shell surface.
    /// Uses wlr-layer-shell protocol (supported by KWin).
    pub fn init(&self) -> Result<()> {
        // TODO (#4): connect to Wayland display, create layer-shell surface
        // Protocol: zwlr_layer_shell_v1, layer OVERLAY, anchor TOP|LEFT|RIGHT
        // Set exclusive zone to -1 (don't reserve space)
        tracing::info!(
            "DropdownWindow::init — height {}%, opacity {}, anim {}ms",
            self.height_percent,
            self.opacity,
            self.animation_ms
        );
        Ok(())
    }
}
