use anyhow::{Context, Result};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use std::sync::{Arc, Mutex};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_output, wl_shm, wl_surface},
    Connection, EventQueue, QueueHandle,
};

// ── Shared toggle flag between tokio (shortcut) and Wayland thread ──────────

#[derive(Clone, Default)]
pub struct ToggleFlag(pub Arc<Mutex<bool>>);

impl ToggleFlag {
    pub fn trigger(&self) {
        let mut v = self.0.lock().unwrap();
        *v = !*v;
    }
    pub fn get(&self) -> bool {
        *self.0.lock().unwrap()
    }
}

// ── Wayland state ────────────────────────────────────────────────────────────

pub struct DropdownSurface {
    registry_state: RegistryState,
    output_state: OutputState,
    compositor_state: CompositorState,
    shm: Shm,
    layer_shell: LayerShell,

    layer_surface: Option<LayerSurface>,
    pool: Option<SlotPool>,

    screen_width: u32,
    configured: bool,

    height_percent: u8,
    opacity: f32,
    pub visible: bool,

    pub toggle_flag: ToggleFlag,
}

impl DropdownSurface {
    pub fn new(
        height_percent: u8,
        opacity: f32,
        toggle_flag: ToggleFlag,
    ) -> Result<(Self, EventQueue<Self>)> {
        let conn = Connection::connect_to_env().context("failed to connect to Wayland display")?;
        let (globals, queue) =
            registry_queue_init::<Self>(&conn).context("failed to init Wayland registry")?;
        let qh = queue.handle();

        let compositor_state =
            CompositorState::bind(&globals, &qh).context("compositor not available")?;
        let layer_shell =
            LayerShell::bind(&globals, &qh).context("wlr-layer-shell not available — is KWin running?")?;
        let shm = Shm::bind(&globals, &qh).context("wl_shm not available")?;

        let state = Self {
            registry_state: RegistryState::new(&globals),
            output_state: OutputState::new(&globals, &qh),
            compositor_state,
            shm,
            layer_shell,
            layer_surface: None,
            pool: None,
            screen_width: 1920,
            configured: false,
            height_percent,
            opacity,
            visible: false,
            toggle_flag,
        };

        Ok((state, queue))
    }

    pub fn create_surface(&mut self, qh: &QueueHandle<Self>) {
        let surface = self.compositor_state.create_surface(qh);
        let target_height = self.target_height();

        let layer_surface = self.layer_shell.create_layer_surface(
            qh,
            surface,
            Layer::Overlay,
            Some("termix"),
            None,
        );

        // Anchor to top edge, stretch full width; height is fixed
        layer_surface.set_anchor(Anchor::TOP | Anchor::LEFT | Anchor::RIGHT);
        // width=0 means "stretch to fill anchored axis"
        layer_surface.set_size(0, target_height);
        // -1: don't push other windows down
        layer_surface.set_exclusive_zone(-1);
        // Grab keyboard when focused
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
        layer_surface.commit();

        self.layer_surface = Some(layer_surface);
    }

    fn target_height(&self) -> u32 {
        // Will be refined once we receive output dimensions; 600 is a safe fallback
        600 * self.height_percent as u32 / 100
    }

    fn draw(&mut self, qh: &QueueHandle<Self>) {
        let Some(layer_surface) = &self.layer_surface else {
            return;
        };
        if !self.configured {
            return;
        }

        let width = self.screen_width.max(1);
        let height = self.target_height().max(1);
        let stride = width * 4; // ARGB8888

        let pool = self.pool.get_or_insert_with(|| {
            SlotPool::new((stride * height) as usize, &self.shm).expect("failed to create shm pool")
        });

        let (buffer, canvas) = pool
            .create_buffer(
                width as i32,
                height as i32,
                stride as i32,
                wl_shm::Format::Argb8888,
            )
            .expect("failed to create shm buffer");

        // Catppuccin Mocha surface0 — dark, slightly purple
        let alpha = (self.opacity * 255.0) as u8;
        for pixel in canvas.chunks_exact_mut(4) {
            pixel[0] = 30;    // B
            pixel[1] = 30;    // G
            pixel[2] = 46;    // R
            pixel[3] = alpha; // A
        }

        let surface = layer_surface.wl_surface();
        buffer.attach_to(surface).expect("buffer attach failed");
        surface.damage_buffer(0, 0, width as i32, height as i32);
        surface.commit();
    }

    /// Show or hide the surface. Called when toggle_flag changes.
    pub fn apply_toggle(&mut self, qh: &QueueHandle<Self>) {
        let want_visible = self.toggle_flag.get();
        if want_visible == self.visible {
            return;
        }
        self.visible = want_visible;
        if self.visible {
            self.draw(qh);
        } else {
            // Detach buffer → blank/hidden surface
            if let Some(layer_surface) = &self.layer_surface {
                let surface = layer_surface.wl_surface();
                surface.attach(None, 0, 0);
                surface.commit();
            }
        }
    }
}

// ── sctk handler implementations ────────────────────────────────────────────

impl CompositorHandler for DropdownSurface {
    fn scale_factor_changed(
        &mut self, _: &Connection, _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface, _: i32,
    ) {}

    fn transform_changed(
        &mut self, _: &Connection, _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface, _: wl_output::Transform,
    ) {}

    fn frame(
        &mut self, _: &Connection, qh: &QueueHandle<Self>,
        _: &wl_surface::WlSurface, _: u32,
    ) {
        self.draw(qh);
    }

    fn surface_enter(
        &mut self, _: &Connection, _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface, _: &wl_output::WlOutput,
    ) {}

    fn surface_leave(
        &mut self, _: &Connection, _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface, _: &wl_output::WlOutput,
    ) {}
}

impl OutputHandler for DropdownSurface {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self, _: &Connection, _: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        if let Some(info) = self.output_state.info(&output) {
            if let Some(mode) = info.modes.iter().find(|m| m.current) {
                self.screen_width = mode.dimensions.0 as u32;
            }
        }
    }

    fn update_output(
        &mut self, _: &Connection, _: &QueueHandle<Self>,
        _: wl_output::WlOutput,
    ) {}

    fn output_destroyed(
        &mut self, _: &Connection, _: &QueueHandle<Self>,
        _: wl_output::WlOutput,
    ) {}
}

impl LayerShellHandler for DropdownSurface {
    fn closed(
        &mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface,
    ) {
        tracing::info!("layer surface closed");
    }

    fn configure(
        &mut self, _: &Connection, qh: &QueueHandle<Self>,
        _: &LayerSurface, configure: LayerSurfaceConfigure, _: u32,
    ) {
        // Compositor tells us the actual width it assigned
        if configure.new_size.0 > 0 {
            self.screen_width = configure.new_size.0;
        }
        self.configured = true;
        self.draw(qh);
    }
}

impl ShmHandler for DropdownSurface {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for DropdownSurface {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState];
}

delegate_compositor!(DropdownSurface);
delegate_output!(DropdownSurface);
delegate_layer!(DropdownSurface);
delegate_shm!(DropdownSurface);
delegate_registry!(DropdownSurface);
