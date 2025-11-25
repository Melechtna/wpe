use std::{env, path::PathBuf};

use iced::{
    Background, Color, Theme,
    border::{self, Border},
    widget,
};
use walkdir::WalkDir;

const BUTTON_COLOR: Color = Color {
    r: 0x4B as f32 / 255.0,
    g: 0x00 as f32 / 255.0,
    b: 0x6E as f32 / 255.0,
    a: 1.0,
};

const FOLDER_ICON_NAMES: &[&str] = &[
    "folder-open-symbolic",
    "folder-open",
    "document-open-folder",
    "folder-symbolic",
    "folder",
];

const FILE_ICON_NAMES: &[&str] = &[
    "text-x-generic-symbolic",
    "text-x-generic",
    "document-open-symbolic",
    "document-open",
    "document-new",
];

/// Create a pill-shaped button style based on the WPE accent color.
pub(crate) fn purple_button_style<'a>()
-> impl Fn(&Theme, widget::button::Status) -> widget::button::Style + Clone {
    move |_, status| {
        let mut base = BUTTON_COLOR;
        if matches!(status, widget::button::Status::Hovered) {
            base = lighten(base, 0.08);
        } else if matches!(status, widget::button::Status::Pressed) {
            base = lighten(base, -0.05);
        }

        widget::button::Style {
            background: Some(Background::Color(base)),
            text_color: Color::WHITE,
            border: Border {
                radius: border::Radius::default().left(999.0).right(999.0),
                ..Default::default()
            },
            shadow: Default::default(),
        }
    }
}

/// Return the first matching folder icon from standard icon search paths.
pub(crate) fn load_folder_icon() -> Option<widget::svg::Handle> {
    find_icon_path(FOLDER_ICON_NAMES).map(widget::svg::Handle::from_path)
}

pub(crate) fn load_file_icon() -> Option<widget::svg::Handle> {
    find_icon_path(FILE_ICON_NAMES).map(widget::svg::Handle::from_path)
}

fn lighten(color: Color, delta: f32) -> Color {
    let adjust = |component: f32| (component + delta).clamp(0.0, 1.0);
    Color {
        r: adjust(color.r),
        g: adjust(color.g),
        b: adjust(color.b),
        a: color.a,
    }
}

fn find_icon_path(names: &[&str]) -> Option<PathBuf> {
    for root in icon_search_roots() {
        if !root.exists() {
            continue;
        }
        for entry in WalkDir::new(root)
            .max_depth(5)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let extension = path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("svg"))
                .unwrap_or(false);
            if !extension {
                continue;
            }
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                if names.iter().any(|name| stem == *name) {
                    return Some(path.to_path_buf());
                }
            }
        }
    }
    None
}

fn icon_search_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(xdg_data_home) = env::var("XDG_DATA_HOME") {
        roots.push(PathBuf::from(xdg_data_home).join("icons"));
    } else if let Ok(home) = env::var("HOME") {
        roots.push(PathBuf::from(&home).join(".local/share/icons"));
    }
    if let Ok(home) = env::var("HOME") {
        roots.push(PathBuf::from(home).join(".icons"));
    }
    let data_dirs =
        env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    for dir in data_dirs.split(':') {
        if dir.is_empty() {
            continue;
        }
        roots.push(PathBuf::from(dir).join("icons"));
    }
    roots
}
