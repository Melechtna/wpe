use futures::SinkExt;
use futures::channel::mpsc::UnboundedSender;
use std::error::Error;

use smithay_client_toolkit::{
    output::{OutputHandler, OutputState},
    reexports::client::{
        Connection, QueueHandle, globals::registry_queue_init, protocol::wl_output::WlOutput,
    },
    registry::{ProvidesRegistryState, RegistryState},
};

#[derive(Debug, Clone)]
pub struct Monitor {
    pub name: String,
    pub description: String,
    pub width: u32,
    pub height: u32,
    pub refresh_rate: u32,
}

/// Minimal app state just for querying outputs.
struct MonitorApp {
    registry_state: RegistryState,
    output_state: OutputState,
}

impl OutputHandler for MonitorApp {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _output: WlOutput) {
        // Might be a good idea to, at some point, repopulate the GUI with newly plugged outputs,
        // but you can also just relaunch the application, so *shrug*
    }

    fn update_output(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _output: WlOutput) {
        // Incase someone would like to impliment repolling resolution or refresh rate
    }

    fn output_destroyed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _output: WlOutput) {
        // Same as with new, this is for doing things for losing outputs.
    }
}

// Wire up smithay’s delegation macros so registry + outputs work.

smithay_client_toolkit::delegate_registry!(MonitorApp);
smithay_client_toolkit::delegate_output!(MonitorApp);

impl ProvidesRegistryState for MonitorApp {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    // Tell SCTK that OutputState wants registry events (wl_output / xdg-output).
    smithay_client_toolkit::registry_handlers!(OutputState);
}

pub fn list_monitors() -> Result<Vec<Monitor>, Box<dyn Error>> {
    // Connect and grab the initial global list + a queue.
    let conn = Connection::connect_to_env()?;
    let (globals, mut event_queue) = registry_queue_init::<MonitorApp>(&conn)?;

    // Create our app state and bind outputs via OutputState.
    let qh = event_queue.handle();
    let mut app = MonitorApp {
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
    };

    // Process events once so OutputState receives output info
    event_queue.blocking_dispatch(&mut app)?;

    // Read out all outputs from OutputState.
    let mut monitors = Vec::new();

    for wl_output in app.output_state.outputs() {
        if let Some(info) = app.output_state.info(&wl_output) {
            // Prefer the current mode, otherwise just pick the first mode.
            let mode = info
                .modes
                .iter()
                .find(|m| m.current)
                .or_else(|| info.modes.first());

            let (width, height, refresh_rate) = mode
                .map(|m| {
                    let (w, h) = m.dimensions;
                    // refresh_rate is in millihertz; fall back to 60 Hz if 0.
                    let hz = if m.refresh_rate > 0 {
                        (m.refresh_rate / 1000).max(1)
                    } else {
                        60
                    };
                    (w as u32, h as u32, hz as u32)
                })
                .unwrap_or((1920, 1080, 60));

            monitors.push(Monitor {
                name: info.name.clone().unwrap_or_else(|| "unknown".into()),
                description: info
                    .description
                    .clone()
                    .unwrap_or_else(|| "No description".into()),
                width,
                height,
                refresh_rate,
            });
        }
    }

    Ok(monitors)
}

/// Watch outputs and push updates to an async channel (unbounded).
pub fn watch_monitors_unbounded(
    mut tx: UnboundedSender<Vec<Monitor>>,
) -> Result<(), Box<dyn Error>> {
    let conn = Connection::connect_to_env()?;
    let (globals, mut event_queue) = registry_queue_init::<MonitorApp>(&conn)?;

    let qh = event_queue.handle();
    let mut app = MonitorApp {
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
    };

    event_queue.blocking_dispatch(&mut app)?;
    if !futures::executor::block_on(send_snapshot_async(&app.output_state, &mut tx)) {
        return Ok(());
    }

    loop {
        event_queue.blocking_dispatch(&mut app)?;
        if !futures::executor::block_on(send_snapshot_async(&app.output_state, &mut tx)) {
            return Ok(());
        }
    }
}

fn send_snapshot_async(
    output_state: &OutputState,
    tx: &mut UnboundedSender<Vec<Monitor>>,
) -> futures::future::BoxFuture<'static, bool> {
    let monitors = collect_monitors(output_state);
    let mut tx = tx.clone();
    Box::pin(async move { tx.send(monitors).await.is_ok() })
}

fn collect_monitors(output_state: &OutputState) -> Vec<Monitor> {
    let mut monitors = Vec::new();
    for wl_output in output_state.outputs() {
        if let Some(info) = output_state.info(&wl_output) {
            let mode = info
                .modes
                .iter()
                .find(|m| m.current)
                .or_else(|| info.modes.first());
            let (width, height, refresh_rate) = mode
                .map(|m| {
                    let (w, h) = m.dimensions;
                    let hz = if m.refresh_rate > 0 {
                        (m.refresh_rate / 1000).max(1)
                    } else {
                        60
                    };
                    (w as u32, h as u32, hz as u32)
                })
                .unwrap_or((1920, 1080, 60));

            monitors.push(Monitor {
                name: info.name.clone().unwrap_or_else(|| "unknown".into()),
                description: info
                    .description
                    .clone()
                    .unwrap_or_else(|| "No description".into()),
                width,
                height,
                refresh_rate,
            });
        }
    }
    monitors
}
