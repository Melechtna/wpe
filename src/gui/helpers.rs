use std::{
    env, fs,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
};

use ashpd::desktop::file_chooser::SelectedFiles;
use zbus::{Connection, Proxy, zvariant::OwnedValue};

use crate::{
    config::{self, WallpaperProfileEntry},
    monitors::{self, Monitor},
};

use super::{editor::PathKind, message::Message, types::ThemePreference};
use futures::stream::{BoxStream, StreamExt};
use iced::Subscription;
use iced::advanced::subscription::{self as advanced_subscription, EventStream, Hasher, Recipe};

/// Kind of source the user wants to pick.
#[derive(Debug, Clone, Copy)]
pub enum PathSelection {
    File,
    Folder,
}

/// Detect whether the input path points to a file, folder, or nothing.
pub(crate) fn detect_path_kind(input: &str) -> PathKind {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return PathKind::Empty;
    }

    let path = match config::parse_user_path(trimmed) {
        Some(path) => path,
        None => return PathKind::Unknown,
    };

    match fs::metadata(&path) {
        Ok(metadata) => {
            if metadata.is_dir() {
                PathKind::Folder
            } else if metadata.is_file() {
                PathKind::File
            } else {
                PathKind::Unknown
            }
        }
        Err(_) => PathKind::Unknown,
    }
}

/// Convert a slideshow interval to HH:MM:SS for display.
pub(crate) fn format_interval(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    format!("{:02}:{:02}:{:02}", hours.min(99), minutes, secs)
}

/// Parse HH:MM:SS input into seconds, returning user-friendly errors.
pub(crate) fn parse_interval(value: &str) -> Result<u64, String> {
    let parts: Vec<_> = value.split(':').collect();
    if parts.len() != 3 {
        return Err("Use HH:MM:SS".into());
    }

    let mut total = 0u64;
    for (idx, part) in parts.iter().enumerate() {
        if part.len() != 2 {
            return Err("Use two-digit fields".into());
        }
        let number = part
            .parse::<u64>()
            .map_err(|_| "Interval fields must be numeric".to_string())?;
        if idx > 0 && number > 59 {
            return Err("Minutes/seconds must be <= 59".into());
        }
        total = match idx {
            0 => number * 3600,
            1 => total + number * 60,
            _ => total + number,
        };
    }
    Ok(total.max(1))
}

/// Query wl_output and convert them into our `Monitor` struct.
pub(crate) async fn load_monitors() -> Result<Vec<Monitor>, String> {
    monitors::list_monitors().map_err(|err| err.to_string())
}

/// Read the config profile from disk, creating defaults if needed.
pub(crate) async fn load_entries() -> Result<Vec<WallpaperProfileEntry>, String> {
    config::load_wallpaper_entries().map_err(|err| err.to_string())
}

/// Launch the CLI version in the background using `-c`.
pub(crate) fn spawn_wallpaper() -> Result<(), String> {
    // Prevent duplicates: kill any running mpvpaper first.
    let _ = Command::new("pkill")
        .arg("mpvpaper")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let exe = std::env::current_exe().map_err(|err| err.to_string())?;
    let status = Command::new(exe)
        .arg("-c")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| err.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "wpe -c exited with status {}",
            status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "signal".into())
        ))
    }
}

/// Use xdg-desktop-portal to pick a local file/folder.
pub(crate) async fn select_wallpaper_source(
    kind: PathSelection,
) -> Result<Option<PathBuf>, String> {
    let mut request = SelectedFiles::open_file()
        .title("Select wallpaper source")
        .accept_label("Select")
        .modal(true);

    if matches!(kind, PathSelection::Folder) {
        request = request.directory(true);
    }

    let request = request.send().await.map_err(|err| err.to_string())?;

    let response = request.response().map_err(|err| err.to_string())?;

    if let Some(uri) = response.uris().first() {
        if uri.scheme() == "file" {
            uri.to_file_path()
                .map_err(|_| "Only local files or folders are supported.".to_string())
                .map(Some)
        } else {
            Err("Only local files or folders are supported.".into())
        }
    } else {
        Ok(None)
    }
}

/// Pick a theme by querying the portal or falling back to env vars.
pub(crate) async fn detect_theme_preference() -> ThemePreference {
    if let Some(pref) = query_portal_theme().await {
        return pref;
    }
    if let Some(pref) = guess_theme_from_env() {
        return pref;
    }
    ThemePreference::Dark
}

/// Subscription that pushes monitor updates reactively (Wayland events).
pub(crate) fn monitor_events() -> Subscription<Message> {
    advanced_subscription::from_recipe(MonitorEventRecipe)
}

#[derive(Debug, Clone)]
struct MonitorEventRecipe;

impl Recipe for MonitorEventRecipe {
    type Output = Message;

    fn hash(&self, state: &mut Hasher) {
        use std::hash::Hash;
        "monitor-events".hash(state);
    }

    fn stream(self: Box<Self>, _input: EventStream) -> BoxStream<'static, Message> {
        let (tx, rx) = futures::channel::mpsc::unbounded();
        thread::spawn(move || {
            let _ = monitors::watch_monitors_unbounded(tx);
        });
        rx.map(Message::MonitorsUpdated).boxed()
    }
}

async fn query_portal_theme() -> Option<ThemePreference> {
    let connection = Connection::session().await.ok()?;
    let proxy = Proxy::new(
        &connection,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.Settings",
    )
    .await
    .ok()?;

    let value: OwnedValue = proxy
        .call("Read", &("org.freedesktop.appearance", "color-scheme"))
        .await
        .ok()?;
    let code: u32 = u32::try_from(value).ok()?;
    match code {
        1 => Some(ThemePreference::Dark),
        2 => Some(ThemePreference::Light),
        _ => None,
    }
}

fn guess_theme_from_env() -> Option<ThemePreference> {
    if let Ok(theme) = env::var("GTK_THEME") {
        return classify_theme_hint(theme);
    }
    if let Ok(theme) = env::var("XCURSOR_THEME") {
        return classify_theme_hint(theme);
    }
    None
}

fn classify_theme_hint(value: String) -> Option<ThemePreference> {
    let lower = value.to_lowercase();
    if lower.contains("dark") {
        Some(ThemePreference::Dark)
    } else if lower.contains("light") {
        Some(ThemePreference::Light)
    } else {
        None
    }
}
