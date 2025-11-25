/// Draw a compositor-level overlay that labels every detected monitor.

use std::{collections::HashMap, thread};

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
    },
    shm::{Shm, ShmHandler, slot::SlotPool},
};
use wayland_client::{
    Connection, Proxy, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_output, wl_shm, wl_surface},
};

const OVERLAY_WIDTH: u32 = 260;
const OVERLAY_HEIGHT: u32 = 88;
const GLYPH_WIDTH: u32 = 5;
const GLYPH_SCALE: u32 = 4;
const OVERLAY_BG: [u8; 4] = [0x6E, 0x00, 0x4B, 0xFF];
const TEXT_COLOR: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xFF];

/// Spawn a detached thread that paints overlays for every Wayland output.
pub fn spawn_overlay() {
    let _ = thread::Builder::new().name("wpe-overlay".into()).spawn(|| {
        if let Err(err) = overlay_main() {
            eprintln!("overlay error: {err}");
        }
    });
}

/// Connect to Wayland and drive the layer-shell event loop.
fn overlay_main() -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::connect_to_env()?;
    let (globals, mut event_queue) = registry_queue_init(&conn)?;
    let qh = event_queue.handle();

    let compositor = CompositorState::bind(&globals, &qh)?;
    let layer_shell = LayerShell::bind(&globals, &qh)?;
    let shm = Shm::bind(&globals, &qh)?;

    let mut state = OverlayState::new(&globals, compositor, layer_shell, shm, &qh);
    state.bootstrap_overlays(&qh);

    loop {
        event_queue.blocking_dispatch(&mut state)?;
    }
}

/// Tracks compositor globals plus the overlay surfaces we created.
struct OverlayState {
    registry_state: RegistryState,
    output_state: OutputState,
    compositor_state: CompositorState,
    layer_shell: LayerShell,
    shm: Shm,
    overlays: HashMap<u32, OverlaySurface>,
}

impl OverlayState {
    fn new(
        globals: &smithay_client_toolkit::reexports::client::globals::GlobalList,
        compositor_state: CompositorState,
        layer_shell: LayerShell,
        shm: Shm,
        qh: &QueueHandle<Self>,
    ) -> Self {
        Self {
            registry_state: RegistryState::new(globals),
            output_state: OutputState::new(globals, qh),
            compositor_state,
            layer_shell,
            shm,
            overlays: HashMap::new(),
        }
    }

    /// Create overlays for outputs that already existed before we connected.
    fn bootstrap_overlays(&mut self, qh: &QueueHandle<Self>) {
        let outputs: Vec<_> = self.output_state.outputs().collect();
        for output in outputs {
            if let Some(info) = self.output_state.info(&output) {
                let name = info.name.clone().unwrap_or_else(|| {
                    info.description.clone().unwrap_or_else(|| "Display".into())
                });
                self.create_overlay(output, name, qh);
            }
        }
    }

    /// Create a purple badge for the provided output name.
    fn create_overlay(
        &mut self,
        output: wl_output::WlOutput,
        name: String,
        qh: &QueueHandle<Self>,
    ) {
        let surface = self.compositor_state.create_surface(qh);
        let layer = self.layer_shell.create_layer_surface(
            qh,
            surface,
            Layer::Overlay,
            Some("wpe-overlay"),
            Some(&output),
        );
        layer.set_size(OVERLAY_WIDTH, OVERLAY_HEIGHT);
        layer.set_anchor(Anchor::TOP | Anchor::LEFT);
        layer.set_exclusive_zone(0);
        layer.set_margin(10, 0, 0, 10);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer.commit();

        let pool = SlotPool::new((OVERLAY_WIDTH * OVERLAY_HEIGHT * 4) as usize, &self.shm)
            .expect("slot pool");

        let id = layer.wl_surface().id().protocol_id();
        self.overlays.insert(
            id,
            OverlaySurface {
                output,
                layer,
                pool,
                width: OVERLAY_WIDTH,
                height: OVERLAY_HEIGHT,
                name,
            },
        );
    }

    /// Remove overlays when an output disappears.
    fn remove_overlay(&mut self, output: &wl_output::WlOutput) {
        self.overlays.retain(|_, surf| &surf.output != output);
    }

    /// Redraw a surface when the compositor asks us to reconfigure it.
    fn draw_for_layer(&mut self, layer: &LayerSurface, qh: &QueueHandle<Self>) {
        if let Some(surface) = self
            .overlays
            .get_mut(&layer.wl_surface().id().protocol_id())
        {
            surface.draw(qh);
        }
    }
}

/// Small helper that owns the GPU resources for a single badge.
struct OverlaySurface {
    output: wl_output::WlOutput,
    layer: LayerSurface,
    pool: SlotPool,
    width: u32,
    height: u32,
    name: String,
}

impl OverlaySurface {
    fn draw(&mut self, qh: &QueueHandle<OverlayState>) {
        let width = self.width.max(1);
        let height = self.height.max(1);
        let stride = width as i32 * 4;

        let (buffer, canvas) = self
            .pool
            .create_buffer(
                width as i32,
                height as i32,
                stride,
                wl_shm::Format::Argb8888,
            )
            .expect("buffer");

        {
            let data = canvas.as_mut();
            fill_capsule(data, width, height);
            draw_text(data, width, height, &self.name);
        }

        self.layer
            .wl_surface()
            .damage_buffer(0, 0, width as i32, height as i32);
        self.layer
            .wl_surface()
            .frame(qh, self.layer.wl_surface().clone());
        buffer
            .attach_to(self.layer.wl_surface())
            .expect("attach overlay");
        self.layer.commit();
    }
}

/// Rasterise the monitor name using the tiny bitmap font.
fn draw_text(buffer: &mut [u8], width: u32, height: u32, text: &str) {
    let uppercase = text.to_uppercase();
    let glyph_height = (7 * GLYPH_SCALE) as i32;
    let text_width = text_pixel_width(&uppercase) as i32;
    let start_x = ((width as i32 - text_width) / 2).max(8);
    let start_y = ((height as i32 - glyph_height) / 2).max(4);
    let mut cursor_x = start_x;
    for ch in uppercase.chars() {
        if cursor_x + (GLYPH_WIDTH * GLYPH_SCALE) as i32 >= width as i32 {
            break;
        }
        if let Some(rows) = glyph_rows(ch) {
            for (row, bits) in rows.iter().enumerate() {
                for col in 0..GLYPH_WIDTH {
                    if bits & (1 << (GLYPH_WIDTH - 1 - col)) != 0 {
                        for sy in 0..GLYPH_SCALE {
                            for sx in 0..GLYPH_SCALE {
                                let px = cursor_x + (col * GLYPH_SCALE + sx) as i32;
                                let py = start_y + (row as u32 * GLYPH_SCALE + sy) as i32;
                                if px >= 0 && py >= 0 && px < width as i32 && py < height as i32 {
                                    let offset = (py as u32 * width + px as u32) as usize * 4;
                                    buffer[offset..offset + 4].copy_from_slice(&TEXT_COLOR);
                                }
                            }
                        }
                    }
                }
            }
        }
        cursor_x += (GLYPH_WIDTH * GLYPH_SCALE + GLYPH_SCALE) as i32;
    }
}

/// Compute the rendered pixel width for a string so we can center it.
fn text_pixel_width(text: &str) -> u32 {
    let mut width = 0u32;
    let mut first = true;
    for ch in text.chars() {
        if glyph_rows(ch).is_some() {
            if !first {
                width += GLYPH_SCALE;
            }
            width += GLYPH_WIDTH * GLYPH_SCALE;
            first = false;
        }
    }
    width
}

/// Return the bitmap rows for the limited glyph set we support.
fn glyph_rows(ch: char) -> Option<[u8; 7]> {
    Some(match ch {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b11110, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
        ],
        'D' => [
            0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100,
        ],
        'E' => [
            0b11111, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111,
        ],
        'H' => [
            0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        'J' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01110, 0b10001, 0b10000, 0b01110, 0b00001, 0b10001, 0b01110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        '0' => [
            0b01110, 0b10011, 0b10101, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        '4' => [
            0b10010, 0b10010, 0b10010, 0b11111, 0b00010, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ],
        '6' => [
            0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
        ],
        '-' => [
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ],
        ' ' => [0; 7],
        _ => return None,
    })
}

/// Paint the purple squircle while masking out pixels outside the rounded ends.
fn fill_capsule(buffer: &mut [u8], width: u32, height: u32) {
    let radius = (height as i32) / 2;
    let center_y = height as i32 / 2;
    let right_center = width as i32 - radius;
    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let offset = (y as u32 * width + x as u32) as usize * 4;
            let inside = if x < radius {
                let dx = radius - x;
                let dy = center_y - y;
                dx * dx + dy * dy <= radius * radius
            } else if x >= right_center {
                let dx = x - right_center;
                let dy = center_y - y;
                dx * dx + dy * dy <= radius * radius
            } else {
                true
            };
            if inside {
                buffer[offset..offset + 4].copy_from_slice(&OVERLAY_BG);
            } else {
                buffer[offset + 3] = 0;
            }
        }
    }
}

impl CompositorHandler for OverlayState {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for OverlayState {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        if let Some(info) = self.output_state.info(&output) {
            let name = info
                .name
                .clone()
                .unwrap_or_else(|| info.description.clone().unwrap_or_else(|| "Display".into()));
            self.create_overlay(output, name, qh);
        }
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        if let Some(info) = self.output_state.info(&output) {
            let name = info
                .name
                .clone()
                .unwrap_or_else(|| info.description.clone().unwrap_or_else(|| "Display".into()));
            self.remove_overlay(&output);
            self.create_overlay(output, name, qh);
        }
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        self.remove_overlay(&output);
    }
}

impl LayerShellHandler for OverlayState {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, layer: &LayerSurface) {
        self.overlays.remove(&layer.wl_surface().id().protocol_id());
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if let Some(surface) = self
            .overlays
            .get_mut(&layer.wl_surface().id().protocol_id())
        {
            let (w, h) = configure.new_size;
            if w > 0 && h > 0 {
                surface.width = w;
                surface.height = h;
            }
        }
        self.draw_for_layer(layer, qh);
    }
}

impl ShmHandler for OverlayState {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

delegate_compositor!(OverlayState);
delegate_output!(OverlayState);
delegate_shm!(OverlayState);
delegate_layer!(OverlayState);
delegate_registry!(OverlayState);

impl ProvidesRegistryState for OverlayState {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState];
}
