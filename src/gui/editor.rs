use std::path::PathBuf;

use iced::widget::{self, Column, Row, button, checkbox, container, svg, text, text_input};
use iced::{Color, Element, Length, alignment};

use crate::{
    config::{self, DEFAULT_INTERVAL_SECS, ScaleMode, SlideshowOrder, WallpaperProfileEntry},
    monitors::Monitor,
};

use super::{
    helpers::{PathSelection, detect_path_kind, format_interval, parse_interval},
    message::Message,
    style::{load_file_icon, load_folder_icon, purple_button_style},
};

/// A tab ties monitor metadata with its editable controls.
pub(crate) struct MonitorTab {
    pub monitor: Monitor,
    pub editor: MonitorEditor,
}

/// Holds the editable fields for a single monitor entry.
#[derive(Debug)]
pub(crate) struct MonitorEditor {
    path_text: String,
    path_kind: PathKind,
    enabled: bool,
    pub scale: ScaleMode,
    pub order: SlideshowOrder,
    pub interval_seconds: u64,
    interval_text: String,
    pub interval_error: Option<String>,
    dirty: bool,
}

impl MonitorEditor {
    pub(crate) fn new(entry: Option<WallpaperProfileEntry>) -> Self {
        let (path, scale, order, interval, enabled) = entry
            .map(|entry| {
                (
                    entry
                        .path
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    entry.scale,
                    entry.order,
                    entry.interval_seconds.max(1),
                    entry.enabled,
                )
            })
            .unwrap_or_else(|| {
                (
                    String::new(),
                    ScaleMode::Fit,
                    SlideshowOrder::Sequential,
                    DEFAULT_INTERVAL_SECS,
                    false,
                )
            });

        let path_kind = detect_path_kind(&path);
        Self {
            path_text: path,
            path_kind,
            enabled,
            scale,
            order,
            interval_seconds: interval,
            interval_text: format_interval(interval),
            interval_error: None,
            dirty: false,
        }
    }

    pub(crate) fn set_path_text(&mut self, value: String) {
        self.path_text = value;
        self.path_kind = detect_path_kind(&self.path_text);
        self.dirty = true;
    }

    pub(crate) fn set_path_buf(&mut self, path: PathBuf) {
        self.path_text = path.to_string_lossy().into_owned();
        self.path_kind = detect_path_kind(&self.path_text);
        self.dirty = true;
    }

    pub(crate) fn path_buf(&self) -> Option<PathBuf> {
        config::parse_user_path(&self.path_text)
    }

    pub(crate) fn set_scale(&mut self, scale: ScaleMode) {
        if self.scale != scale {
            self.scale = scale;
            self.dirty = true;
        }
    }

    pub(crate) fn set_order(&mut self, order: SlideshowOrder) {
        if self.order != order {
            self.order = order;
            self.dirty = true;
        }
    }

    pub(crate) fn set_interval(&mut self, value: String) {
        self.interval_text = value.clone();
        match parse_interval(&value) {
            Ok(seconds) => {
                self.interval_error = None;
                self.interval_seconds = seconds.max(1);
            }
            Err(err) => {
                self.interval_error = Some(err);
            }
        }
        self.dirty = true;
    }

    pub(crate) fn mark_saved(&mut self) {
        self.dirty = false;
    }

    pub(crate) fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub(crate) fn enabled(&self) -> bool {
        self.enabled
    }

    pub(crate) fn set_enabled(&mut self, value: bool) {
        if self.enabled != value {
            self.enabled = value;
            self.dirty = true;
        }
    }
}

/// Tracks what kind of path (file/folder) the user typed or selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PathKind {
    Empty,
    File,
    Folder,
    Unknown,
}

impl PathKind {
    pub(crate) fn description(&self) -> &'static str {
        match self {
            PathKind::Empty => "No path configured.",
            PathKind::File => "Detected: file",
            PathKind::Folder => "Detected: folder",
            PathKind::Unknown => "Unable to detect path type (will try at runtime)",
        }
    }
}

impl MonitorTab {
    pub(crate) fn view(&self, index: usize, icon: Option<&svg::Handle>) -> Element<'_, Message> {
        let Monitor {
            name,
            description,
            width,
            height,
            refresh_rate,
        } = &self.monitor;
        let mut body = Column::new()
            .spacing(16)
            .push(text(name).size(28))
            .push(
                text(format!(
                    "{} — {}x{} @ {}Hz",
                    description, width, height, refresh_rate
                ))
                .size(16),
            )
            .push(
                Row::new()
                    .spacing(8)
                    .align_y(alignment::Vertical::Center)
                    .push(text("Enable:").size(16))
                    .push(
                        checkbox("", self.editor.enabled())
                            .on_toggle(move |checked| Message::EnabledToggled(index, checked)),
                    ),
            )
            .push(self.media_row(index, icon));

        body = body.push(text(self.editor.path_kind.description()).size(14));

        if self.editor.path_kind == PathKind::Folder {
            body = body
                .push(folder_controls(index, self.editor.order))
                .push(interval_row(index, &self.editor.interval_text));
            if let Some(err) = &self.editor.interval_error {
                let warn_color = Color::from_rgb(0.95, 0.56, 0.56);
                body = body.push(text(err).style(move |_| widget::text::Style {
                    color: Some(warn_color),
                    ..Default::default()
                }));
            }
        }

        body = body.push(scale_controls(index, self.editor.scale));
        container(body).into()
    }

    fn media_row(&self, index: usize, folder_icon: Option<&svg::Handle>) -> Element<'_, Message> {
        let file_icon: Element<'_, Message> = load_file_icon()
            .map(|handle| {
                svg(handle)
                    .width(Length::Fixed(24.0))
                    .height(Length::Fixed(24.0))
                    .into()
            })
            .unwrap_or_else(|| text("File").into());

        let folder_icon: Element<'_, Message> = folder_icon
            .cloned()
            .or_else(load_folder_icon)
            .map(|handle| {
                svg(handle)
                    .width(Length::Fixed(24.0))
                    .height(Length::Fixed(24.0))
                    .into()
            })
            .unwrap_or_else(|| text("Folder").into());

        Row::new()
            .spacing(12)
            .align_y(alignment::Vertical::Center)
            .push(text("Source:"))
            .push(
                text_input("/path/to/image, video, or folder", &self.editor.path_text)
                    .on_input(move |value| Message::PathChanged(index, value))
                    .width(Length::Fill),
            )
            .push(
                button(file_icon)
                    .on_press(Message::BrowsePressed(index, PathSelection::File))
                    .style(purple_button_style())
                    .padding(6),
            )
            .push(
                button(folder_icon)
                    .on_press(Message::BrowsePressed(index, PathSelection::Folder))
                    .style(purple_button_style())
                    .padding(6),
            )
            .into()
    }
}

fn folder_controls(index: usize, order: SlideshowOrder) -> Element<'static, Message> {
    let sequential = widget::radio(
        "Sequential",
        SlideshowOrder::Sequential,
        Some(order),
        move |choice| Message::OrderChanged(index, choice),
    );

    let random = widget::radio(
        "Random",
        SlideshowOrder::Random,
        Some(order),
        move |choice| Message::OrderChanged(index, choice),
    );
    Column::new()
        .spacing(8)
        .push(text("Folder playback"))
        .push(Row::new().spacing(12).push(sequential).push(random))
        .into()
}

fn interval_row<'a>(index: usize, current: &'a str) -> Element<'a, Message> {
    Row::new()
        .spacing(12)
        .align_y(alignment::Vertical::Center)
        .push(text("Timer"))
        .push(
            text_input("HH:MM:SS", current)
                .on_input(move |value| Message::IntervalChanged(index, value))
                .width(Length::Fixed(120.0)),
        )
        .into()
}

fn scale_controls(index: usize, scale: ScaleMode) -> Element<'static, Message> {
    let original = widget::radio(
        "Original",
        ScaleMode::Original,
        Some(scale),
        move |choice| Message::ScaleChanged(index, choice),
    );
    let fit = widget::radio("Fit", ScaleMode::Fit, Some(scale), move |choice| {
        Message::ScaleChanged(index, choice)
    });
    let stretch = widget::radio("Stretch", ScaleMode::Stretch, Some(scale), move |choice| {
        Message::ScaleChanged(index, choice)
    });

    Column::new()
        .spacing(8)
        .push(text("Sizing"))
        .push(
            Row::new()
                .spacing(12)
                .push(original)
                .push(fit)
                .push(stretch),
        )
        .into()
}
