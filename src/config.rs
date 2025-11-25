use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use std::env;

use crate::monitors::Monitor;

const CONFIG_HEADER: &str = "\
# ///////////////////////////////////////////////
# This config powers WallPaper Engine (wpe).
# Each display starts with [[wallpapers]] and is
# auto-populated either by the GUI or by
# running wpe -c on first run. monitor is
# the output we're targeting. path is the
# image, video, or folder. scale controls how
# mpvpaper scales the source: fit fills the
# monitor, stretch preserves aspect ratio, and
# original uses the source resolution. Set enabled
# to false to leave a display unconfigured without
# clearing the path. order is for folders:
# sequential (A-Z) or random.
# interval_seconds is the amount of time (in
# seconds) before folder content swaps to the
# next image or video.
# ///////////////////////////////////////////////
";

pub const PLACEHOLDER_PATH: &str = "your/image/or/folder/here";

/// Scaling choices exposed to both CLI and config file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScaleMode {
    /// Non-uniform scaling to fill the entire output.
    Fit,
    /// Uniform scaling that preserves aspect ratio (letterboxed/pillarboxed).
    Stretch,
    /// No scaling (render at the source centered as is).
    Original,
}

#[derive(Debug, Clone)]
pub enum MediaKind {
    Image(PathBuf),
    Folder(PathBuf),
    Video(PathBuf),
}

impl MediaKind {
    pub fn path(&self) -> &Path {
        match self {
            MediaKind::Image(path) | MediaKind::Folder(path) | MediaKind::Video(path) => path,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub monitor: Option<String>,
    pub media: MediaKind,
    pub slideshow: SlideshowSettings,
    pub scale: ScaleMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SlideshowOrder {
    Sequential,
    Random,
}

#[derive(Debug, Clone, Copy)]
pub struct SlideshowSettings {
    pub order: SlideshowOrder,
    pub interval: Duration,
}

impl RuntimeConfig {
    /// Build runtime settings from ~/.config/wpe/config.toml
    pub fn from_entry(index: usize) -> Result<Self, Box<dyn Error>> {
        let mut profile = load_or_create_profile()?;
        if profile.wallpapers.is_empty() {
            profile.wallpapers.push(WallpaperEntry::default());
            save_profile(&profile)?;
        }

        let entry = profile
            .wallpapers
            .get(index)
            .ok_or_else(|| format!("No wallpaper entry found at index {}", index))?;

        let path = entry
            .path
            .as_ref()
            .ok_or_else(|| "Configured entry is missing a file or folder path".to_string())?;

        let resolved_path = normalize_entry_path(path);
        let media = detect_media_kind(&resolved_path)?;
        let slideshow = SlideshowSettings {
            order: entry.order,
            interval: Duration::from_secs(entry.interval_seconds.max(1)),
        };

        Ok(RuntimeConfig {
            monitor: entry.monitor.clone(),
            media,
            slideshow,
            scale: entry.scale,
        })
    }
}

/// Inspect a path and convert it into a MediaKind for renderer usage.
fn detect_media_kind(path: &Path) -> Result<MediaKind, Box<dyn Error>> {
    let metadata = fs::metadata(path)
        .map_err(|err| format!("Unable to access {}: {}", path.display(), err))?;
    if metadata.is_dir() {
        return Ok(MediaKind::Folder(path.to_path_buf()));
    }

    if metadata.is_file() {
        if is_probably_video(path) {
            return Ok(MediaKind::Video(path.to_path_buf()));
        }
        return Ok(MediaKind::Image(path.to_path_buf()));
    }

    Err(format!("{} is neither a file nor a folder", path.display()).into())
}

/// Top-level config file layout written/read by the GUI/CLI.
#[derive(Debug, Serialize, Deserialize)]
struct Profile {
    #[serde(default)]
    wallpapers: Vec<WallpaperEntry>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            wallpapers: vec![WallpaperEntry::default()],
        }
    }
}

/// Per-monitor wallpaper entry persisted to the config file.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WallpaperEntry {
    monitor: Option<String>,
    path: Option<PathBuf>,
    #[serde(default = "default_enabled_false")]
    enabled: bool,
    #[serde(default)]
    scale: ScaleMode,
    #[serde(default)]
    order: SlideshowOrder,
    #[serde(default = "default_interval_secs")]
    interval_seconds: u64,
}

impl Default for WallpaperEntry {
    fn default() -> Self {
        Self {
            monitor: None,
            path: Some(PathBuf::from(PLACEHOLDER_PATH)),
            enabled: false,
            scale: ScaleMode::Fit,
            order: SlideshowOrder::Sequential,
            interval_seconds: DEFAULT_INTERVAL_SECS,
        }
    }
}

pub const DEFAULT_INTERVAL_SECS: u64 = 300;

fn default_interval_secs() -> u64 {
    DEFAULT_INTERVAL_SECS
}

fn default_enabled_false() -> bool {
    false
}

/// Simplified entry structure exposed to the GUI layer.
#[derive(Debug, Clone)]
pub struct WallpaperProfileEntry {
    pub monitor: Option<String>,
    pub path: Option<PathBuf>,
    pub enabled: bool,
    pub scale: ScaleMode,
    pub order: SlideshowOrder,
    pub interval_seconds: u64,
}

impl Default for WallpaperProfileEntry {
    fn default() -> Self {
        Self {
            monitor: None,
            path: Some(PathBuf::from(PLACEHOLDER_PATH)),
            enabled: false,
            scale: ScaleMode::Fit,
            order: SlideshowOrder::Sequential,
            interval_seconds: DEFAULT_INTERVAL_SECS,
        }
    }
}

pub fn load_wallpaper_entries() -> Result<Vec<WallpaperProfileEntry>, Box<dyn Error>> {
    let profile = load_or_create_profile()?;
    let entries = profile
        .wallpapers
        .into_iter()
        .map(|entry| WallpaperProfileEntry {
            monitor: entry.monitor,
            path: entry.path,
            enabled: entry.enabled,
            scale: entry.scale,
            order: entry.order,
            interval_seconds: entry.interval_seconds.max(1),
        })
        .collect();
    Ok(entries)
}

pub fn save_wallpaper_entries(entries: &[WallpaperProfileEntry]) -> Result<(), Box<dyn Error>> {
    let profile = Profile {
        wallpapers: entries
            .iter()
            .map(|entry| WallpaperEntry {
                monitor: entry.monitor.clone(),
                path: entry.path.clone(),
                enabled: entry.enabled,
                scale: entry.scale,
                order: entry.order,
                interval_seconds: entry.interval_seconds.max(1),
            })
            .collect(),
    };
    save_profile(&profile)
}

/// Ensure the config file exists with one entry per monitor, returning entries and creation flag.
pub fn ensure_profile_for_monitors(
    monitors: &[Monitor],
) -> Result<(Vec<WallpaperProfileEntry>, bool, PathBuf), Box<dyn Error>> {
    let path = config_file_path()?;
    if path.exists() {
        let entries = load_wallpaper_entries()?;
        return Ok((entries, false, path));
    }

    let entries: Vec<WallpaperProfileEntry> = if monitors.is_empty() {
        vec![WallpaperProfileEntry {
            enabled: false,
            ..WallpaperProfileEntry::default()
        }]
    } else {
        monitors
            .iter()
            .map(|monitor| WallpaperProfileEntry {
                monitor: Some(monitor.name.clone()),
                path: Some(PathBuf::from(PLACEHOLDER_PATH)),
                enabled: false,
                scale: ScaleMode::Fit,
                order: SlideshowOrder::Sequential,
                interval_seconds: DEFAULT_INTERVAL_SECS,
            })
            .collect()
    };

    save_wallpaper_entries(&entries)?;
    Ok((entries, true, path))
}

/// Resolve ~/.config/wpe/config.toml or create it alongside the directory.
fn config_file_path() -> Result<PathBuf, Box<dyn Error>> {
    let base = if let Ok(custom) = env::var("XDG_CONFIG_HOME") {
        PathBuf::from(custom)
    } else {
        let home = env::var("HOME").map_err(|_| "HOME environment variable not set")?;
        PathBuf::from(home).join(".config")
    };
    let dir = base.join("wpe");
    fs::create_dir_all(&dir)?;
    Ok(dir.join("config.toml"))
}

/// Read the TOML profile from disk (creating a default file if missing).
fn load_or_create_profile() -> Result<Profile, Box<dyn Error>> {
    let path = config_file_path()?;
    if !path.exists() {
        let profile = Profile::default();
        save_profile_to_path(&profile, &path)?;
        return Ok(profile);
    }

    let data = fs::read_to_string(&path)?;
    let profile: Profile = toml::from_str(&data)?;
    Ok(profile)
}

fn save_profile(profile: &Profile) -> Result<(), Box<dyn Error>> {
    let path = config_file_path()?;
    save_profile_to_path(profile, &path)
}

fn save_profile_to_path(profile: &Profile, path: &Path) -> Result<(), Box<dyn Error>> {
    let data = toml::to_string_pretty(profile)?;
    let mut content = String::new();
    content.push_str(CONFIG_HEADER);
    if !CONFIG_HEADER.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(&data);
    fs::write(path, content)?;
    Ok(())
}

/// Convert a GUI text field into a PathBuf, expanding leading ~ and env vars.
pub fn parse_user_path(input: &str) -> Option<PathBuf> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(PathBuf::from(expand_leading_tokens(trimmed)))
}

/// Normalize a config path when launching wallpapers (handles ~, env vars, relatives).
pub fn normalize_entry_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return canonicalize_best_effort(path.to_path_buf());
    }

    let raw = path
        .to_str()
        .map(expand_leading_tokens)
        .unwrap_or_else(|| path.to_string_lossy().into_owned());

    let candidate = PathBuf::from(raw);

    let absolute = if candidate.is_absolute() {
        candidate
    } else if let Ok(home) = env::var("HOME") {
        PathBuf::from(home).join(candidate)
    } else if let Ok(cwd) = env::current_dir() {
        cwd.join(candidate)
    } else {
        candidate
    };

    canonicalize_best_effort(absolute)
}

fn expand_leading_tokens(value: &str) -> String {
    let mut current = value.to_string();

    if let Some(expanded) = expand_home_prefix(&current) {
        current = expanded;
    }

    if let Some(expanded) = expand_env_prefix(&current) {
        current = expanded;
    }

    current
}

fn expand_home_prefix(value: &str) -> Option<String> {
    if value == "~" {
        let home = env::var("HOME").ok()?;
        return Some(home);
    }

    if let Some(rest) = value.strip_prefix("~/") {
        let home = env::var("HOME").ok()?;
        let mut expanded = PathBuf::from(home);
        expanded.push(rest);
        return Some(expanded.to_string_lossy().into_owned());
    }

    None
}

fn expand_env_prefix(value: &str) -> Option<String> {
    if let Some(rest) = value.strip_prefix("${") {
        let end = rest.find('}')?;
        let var = &rest[..end];
        if var.is_empty() {
            return None;
        }
        let remainder = &rest[end + 1..];
        let val = env::var(var).ok()?;
        return Some(format!("{}{}", val, remainder));
    }

    if let Some(rest) = value.strip_prefix('$') {
        let mut len = 0;
        for ch in rest.chars() {
            if ch == '_' || ch.is_ascii_alphanumeric() {
                len += ch.len_utf8();
            } else {
                break;
            }
        }

        if len == 0 {
            return None;
        }

        let (var, remainder) = rest.split_at(len);
        let val = env::var(var).ok()?;
        return Some(format!("{}{}", val, remainder));
    }

    None
}

fn canonicalize_best_effort(path: PathBuf) -> PathBuf {
    fs::canonicalize(&path).unwrap_or(path)
}

impl Default for ScaleMode {
    fn default() -> Self {
        ScaleMode::Fit
    }
}

impl Default for SlideshowOrder {
    fn default() -> Self {
        SlideshowOrder::Sequential
    }
}

fn is_probably_video(path: &Path) -> bool {
    const VIDEO_EXTENSIONS: &[&str] = &[
        "mp4", "mkv", "webm", "mov", "avi", "flv", "wmv", "m4v", "mpg", "mpeg", "ogv", "ts",
        "m2ts", "mxf", "3gp", "m4p",
    ];

    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            let lower = ext.to_ascii_lowercase();
            VIDEO_EXTENSIONS.contains(&lower.as_str())
        })
        .unwrap_or(false)
}
