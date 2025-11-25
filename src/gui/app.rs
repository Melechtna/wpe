use std::{
    fs,
    process::{Command, Stdio},
    time::Duration,
};

use iced::{
    Color, Element, Length, Subscription, Task, Theme, alignment, application, time,
    widget::{Column, Row, button, container, scrollable, text},
    window,
};

use crate::{
    config::{self, WallpaperProfileEntry},
    monitors::Monitor,
};

use super::{
    editor::{MonitorEditor, MonitorTab},
    helpers::{
        PathSelection, detect_theme_preference, load_entries, load_monitors, monitor_events,
        select_wallpaper_source, spawn_wallpaper,
    },
    message::Message,
    overlay,
    style::{load_folder_icon, purple_button_style},
    types::ThemePreference,
};

pub fn launch() -> Result<(), Box<dyn std::error::Error>> {
    overlay::spawn_overlay();
    application("WallPaper Engine", GuiApp::update, GuiApp::view)
        .window(window::Settings {
            platform_specific: window::settings::PlatformSpecific {
                application_id: "io.melechtna.wpe".into(),
                ..Default::default()
            },
            ..window::Settings::default()
        })
        .subscription(|state| state.subscription())
        .theme(|state| state.theme())
        .window_size((860.0, 620.0))
        .run_with(GuiApp::init)
        .map_err(|err| err.into())
}

/// Aggregated GUI state and child-process tracking.
pub(crate) struct GuiApp {
    monitors: Vec<Monitor>,
    saved_entries: Vec<WallpaperProfileEntry>,
    tabs: Vec<MonitorTab>,
    active_tab: usize,
    status: Option<StatusBanner>,
    wallpaper_running: bool,
    system_theme: ThemePreference,
    picker_icon: Option<iced::widget::svg::Handle>,
}

impl GuiApp {
    pub fn init() -> (Self, Task<Message>) {
        let commands = vec![
            Task::perform(load_monitors(), Message::MonitorsLoaded),
            Task::perform(load_entries(), Message::EntriesLoaded),
            Task::perform(detect_theme_preference(), Message::ThemeDetected),
        ];

        (
            Self {
                monitors: Vec::new(),
                saved_entries: Vec::new(),
                tabs: Vec::new(),
                active_tab: 0,
                status: Some(StatusBanner::info("Gathering monitors...")),
                wallpaper_running: false,
                system_theme: ThemePreference::Dark,
                picker_icon: load_folder_icon(),
            },
            Task::batch(commands),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::MonitorsLoaded(result) => match result {
                Ok(monitors) => {
                    self.reconcile_monitors(monitors);
                    self.status = Some(StatusBanner::info("Monitors detected."));
                }
                Err(err) => {
                    self.status = Some(StatusBanner::error(format!(
                        "Failed to list monitors: {}",
                        err
                    )));
                }
            },
            Message::EntriesLoaded(result) => match result {
                Ok(entries) => {
                    self.saved_entries = entries;
                    if !self.monitors.is_empty() {
                        self.reconcile_monitors(self.monitors.clone());
                    }
                }
                Err(err) => {
                    self.status = Some(StatusBanner::error(format!(
                        "Failed to load config: {}",
                        err
                    )));
                }
            },
            Message::ThemeDetected(theme) => {
                self.system_theme = theme;
            }
            Message::MonitorsUpdated(monitors) => {
                self.reconcile_monitors(monitors);
                if self.wallpaper_running {
                    let _ = self.stop_wallpaper();
                    let _ = self.start_wallpaper();
                }
            }
            Message::SelectTab(index) => {
                if index < self.tabs.len() {
                    self.active_tab = index;
                }
            }
            Message::PathChanged(index, value) => {
                if let Some(tab) = self.tabs.get_mut(index) {
                    tab.editor.set_path_text(value);
                }
            }
            Message::BrowsePressed(index, kind) => {
                self.status = Some(StatusBanner::info(match kind {
                    PathSelection::File => "Select an image/video…",
                    PathSelection::Folder => "Select a folder…",
                }));
                return Task::perform(select_wallpaper_source(kind), move |result| {
                    Message::PathPicked(index, result)
                });
            }
            Message::PathPicked(index, result) => match result {
                Ok(Some(path)) => {
                    if let Some(tab) = self.tabs.get_mut(index) {
                        tab.editor.set_path_buf(path);
                        self.status = Some(StatusBanner::success("Updated source path."));
                    }
                }
                Ok(None) => {
                    self.status = Some(StatusBanner::info("Selection canceled."));
                }
                Err(err) => {
                    self.status = Some(StatusBanner::error(err));
                }
            },
            Message::EnabledToggled(index, value) => {
                if let Some(tab) = self.tabs.get_mut(index) {
                    tab.editor.set_enabled(value);
                }
            }
            Message::ScaleChanged(index, scale) => {
                if let Some(tab) = self.tabs.get_mut(index) {
                    tab.editor.set_scale(scale);
                }
            }
            Message::OrderChanged(index, order) => {
                if let Some(tab) = self.tabs.get_mut(index) {
                    tab.editor.set_order(order);
                }
            }
            Message::IntervalChanged(index, value) => {
                if let Some(tab) = self.tabs.get_mut(index) {
                    tab.editor.set_interval(value);
                }
            }
            Message::StartPressed => {
                if self.wallpaper_running {
                    if let Err(err) = self.stop_wallpaper() {
                        self.status = Some(StatusBanner::error(err));
                        return Task::none();
                    }
                }
                let _ = self.start_wallpaper();
            }
            Message::StopPressed => {
                if let Err(err) = self.stop_wallpaper() {
                    self.status = Some(StatusBanner::error(err));
                }
            }
            Message::Tick => {
                self.poll_wallpaper();
            }
        }

        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let mut content = Column::new().spacing(16).padding(24);

        if let Some(banner) = &self.status {
            content = content.push(self.status_banner(banner));
        }

        if self.tabs.is_empty() {
            content = content.push(text("Waiting for monitors..."));
        } else {
            content = content.push(self.tab_bar()).push(self.active_editor_view());
        }

        content = content.push(self.action_row());

        container(scrollable(content).height(Length::Fill)).into()
    }

    fn theme(&self) -> Theme {
        match self.system_theme {
            ThemePreference::Light => Theme::Light,
            ThemePreference::Dark => Theme::Dark,
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch(vec![
            time::every(Duration::from_secs(1)).map(|_| Message::Tick),
            monitor_events(),
        ])
    }

    /// Reconcile current tabs/entries against a fresh monitor list.
    fn reconcile_monitors(&mut self, new_monitors: Vec<Monitor>) {
        self.monitors = new_monitors.clone();

        // Saved entries from disk (for monitors not currently connected).
        let mut remaining_saved = self.saved_entries.clone();
        // Single fallback for entries without an assigned monitor (applied once).
        let mut fallback = remaining_saved
            .iter()
            .position(|e| e.monitor.is_none())
            .map(|idx| remaining_saved.remove(idx));

        // Take existing tabs so we can preserve unsaved edits.
        let mut existing_tabs = self.tabs.drain(..).collect::<Vec<_>>();
        let mut rebuilt_tabs = Vec::with_capacity(new_monitors.len());

        for monitor in new_monitors {
            if let Some(pos) = existing_tabs
                .iter()
                .position(|tab| tab.monitor.name == monitor.name)
            {
                let mut tab = existing_tabs.remove(pos);
                tab.monitor = monitor;
                if let Some(pos) = remaining_saved
                    .iter()
                    .position(|e| e.monitor.as_deref() == Some(&tab.monitor.name))
                {
                    let entry = remaining_saved.remove(pos);
                    // If the tab has no unsaved edits, fill it from the saved config.
                    if !tab.editor.is_dirty() {
                        tab.editor = MonitorEditor::new(Some(entry));
                    }
                }
                rebuilt_tabs.push(tab);
                continue;
            }

            // Next, look for a saved entry on disk for this monitor.
            if let Some(pos) = remaining_saved
                .iter()
                .position(|e| e.monitor.as_deref() == Some(&monitor.name))
            {
                let entry = remaining_saved.remove(pos);
                rebuilt_tabs.push(MonitorTab {
                    monitor,
                    editor: MonitorEditor::new(Some(entry)),
                });
                continue;
            }

            // Use the first unassigned entry as a one-time fallback.
            if let Some(entry) = fallback.take() {
                let mut entry = entry;
                entry.monitor = Some(monitor.name.clone());
                rebuilt_tabs.push(MonitorTab {
                    monitor,
                    editor: MonitorEditor::new(Some(entry)),
                });
                continue;
            }

            // Otherwise create a new blank entry for this monitor.
            let mut entry = WallpaperProfileEntry::default();
            entry.monitor = Some(monitor.name.clone());
            rebuilt_tabs.push(MonitorTab {
                monitor,
                editor: MonitorEditor::new(Some(entry)),
            });
        }

        // Save back disconnected monitor entries plus any tabs we didn't match.
        if let Some(entry) = fallback.take() {
            remaining_saved.push(entry);
        }
        self.saved_entries = remaining_saved;
        self.tabs = rebuilt_tabs;

        if self.tabs.is_empty() {
            self.status = Some(StatusBanner::error(
                "No displays detected. Connect a monitor and try again.",
            ));
        } else {
            self.status = Some(StatusBanner::info(
                "Ready. Configure each monitor and press Start when done.",
            ));
        }
    }

    fn tab_bar(&self) -> Element<'_, Message> {
        let mut bar = Row::new().spacing(12).push(text("Monitors:").size(18));

        for (index, tab) in self.tabs.iter().enumerate() {
            let mut label = format!("{}", tab.monitor.name);
            if tab.editor.is_dirty() {
                label.push_str(" *");
            }

            let button = button(text(label).size(16))
                .padding([8, 16])
                .style(purple_button_style());

            bar = bar.push(button.on_press(Message::SelectTab(index)));
        }

        bar.into()
    }

    fn active_editor_view(&self) -> Element<'_, Message> {
        if let Some(tab) = self.tabs.get(self.active_tab) {
            tab.view(self.active_tab, self.picker_icon.as_ref())
        } else {
            Column::new()
                .push(text("Select a monitor to configure."))
                .into()
        }
    }

    fn action_row(&self) -> Element<'_, Message> {
        let start_button = button(text("Start"))
            .on_press(Message::StartPressed)
            .style(purple_button_style())
            .padding([8, 20]);

        let stop_button = button(text("Stop"))
            .on_press(Message::StopPressed)
            .style(purple_button_style())
            .padding([8, 20]);

        Row::new()
            .spacing(16)
            .align_y(alignment::Vertical::Center)
            .push(start_button)
            .push(stop_button)
            .into()
    }

    fn status_banner(&self, banner: &StatusBanner) -> Element<'_, Message> {
        let color = banner.style();
        let content = banner.text.clone();
        text(content)
            .style(move |_| iced::widget::text::Style {
                color: Some(color),
                ..Default::default()
            })
            .into()
    }

    /// Persist current UI state, validate, and start wallpapers.
    fn start_wallpaper(&mut self) -> Result<(), ()> {
        match self.persist_entries() {
            Ok(entries) => match self.validate_entries(&entries) {
                Ok(valid_entries) if valid_entries == 0 => {
                    self.status = Some(StatusBanner::error(
                        "Enable at least one monitor and choose a valid path before starting.",
                    ));
                    Err(())
                }
                Ok(valid_entries) => match spawn_wallpaper() {
                    Ok(()) => {
                        self.wallpaper_running = true;
                        self.status = Some(StatusBanner::success(format!(
                            "Wallpaper started for {} configured entry(ies).",
                            valid_entries
                        )));
                        Ok(())
                    }
                    Err(err) => {
                        self.status = Some(StatusBanner::error(format!(
                            "Failed to launch wallpaper: {}",
                            err
                        )));
                        Err(())
                    }
                },
                Err(err) => {
                    self.status = Some(StatusBanner::error(err));
                    Err(())
                }
            },
            Err(err) => {
                self.status = Some(StatusBanner::error(err));
                Err(())
            }
        }
    }

    fn stop_wallpaper(&mut self) -> Result<(), String> {
        match Command::new("pkill")
            .arg("mpvpaper")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
        {
            Ok(status) if status.success() => {
                self.wallpaper_running = false;
                self.status = Some(StatusBanner::info("Wallpaper stopped."));
                Ok(())
            }
            Ok(_) => {
                self.wallpaper_running = false;
                Err("No running mpvpaper process found.".into())
            }
            Err(err) => Err(format!("Failed to issue pkill: {}", err)),
        }
    }

    fn poll_wallpaper(&mut self) {
        if !self.wallpaper_running {
            return;
        }

        match Command::new("pgrep")
            .arg("mpvpaper")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
        {
            Ok(status) if status.success() => {}
            Ok(_) => {
                self.wallpaper_running = false;
                self.status = Some(StatusBanner::info("Wallpaper exited."));
            }
            Err(_) => {}
        }
    }

    fn persist_entries(&mut self) -> Result<Vec<WallpaperProfileEntry>, String> {
        if self.tabs.is_empty() {
            return Err("No monitors available.".into());
        }

        if let Some(tab) = self
            .tabs
            .iter()
            .find(|tab| tab.editor.interval_error.is_some())
        {
            return Err(format!(
                "Fix the slideshow interval for {}",
                tab.monitor.name
            ));
        }

        // Start from the saved config, replace entries for connected monitors with current tab state.
        let mut entries = self.saved_entries.clone();

        for tab in &self.tabs {
            let entry = WallpaperProfileEntry {
                monitor: Some(tab.monitor.name.clone()),
                path: tab.editor.path_buf(),
                enabled: tab.editor.enabled(),
                scale: tab.editor.scale,
                order: tab.editor.order,
                interval_seconds: tab.editor.interval_seconds.max(1),
            };

            if let Some(pos) = entries
                .iter()
                .position(|e| e.monitor.as_deref() == Some(&tab.monitor.name))
            {
                entries[pos] = entry;
            } else {
                entries.push(entry);
            }
        }

        config::save_wallpaper_entries(&entries).map_err(|err| err.to_string())?;
        self.saved_entries = entries.clone();
        for tab in &mut self.tabs {
            tab.editor.mark_saved();
        }
        Ok(entries)
    }

    /// Ensure every configured path exists before launching wallpapers.
    fn validate_entries(&self, entries: &[WallpaperProfileEntry]) -> Result<usize, String> {
        let mut valid = 0usize;
        for entry in entries {
            if !entry.enabled {
                continue;
            }

            let path = entry.path.as_ref().ok_or_else(|| {
                format!(
                    "Enabled entry for {} is missing a file or folder path.",
                    entry.monitor.as_deref().unwrap_or("an unassigned monitor")
                )
            })?;

            let resolved = config::normalize_entry_path(path);
            match fs::metadata(&resolved) {
                Ok(_) => valid += 1,
                Err(_) => {
                    return Err(format!("Invalid path or file ({})", resolved.display()));
                }
            }
        }
        Ok(valid)
    }
}

/// Lightweight helper for showing info/error banners.
#[derive(Debug, Clone)]
struct StatusBanner {
    text: String,
    kind: StatusKind,
}

impl StatusBanner {
    fn info<T: Into<String>>(text: T) -> Self {
        Self {
            text: text.into(),
            kind: StatusKind::Info,
        }
    }

    fn success<T: Into<String>>(text: T) -> Self {
        Self {
            text: text.into(),
            kind: StatusKind::Success,
        }
    }

    fn error<T: Into<String>>(text: T) -> Self {
        Self {
            text: text.into(),
            kind: StatusKind::Error,
        }
    }

    fn style(&self) -> Color {
        match self.kind {
            StatusKind::Info => Color::from_rgb(0.6, 0.76, 0.9),
            StatusKind::Success => Color::from_rgb(0.6, 0.9, 0.6),
            StatusKind::Error => Color::from_rgb(0.95, 0.56, 0.56),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum StatusKind {
    Info,
    Success,
    Error,
}
