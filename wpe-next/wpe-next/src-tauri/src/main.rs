use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process;

mod overlay;
mod samples;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorConfig {
    pub name: String,
    pub enabled: bool,
    pub wallpaper: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorInfo {
    pub name: String,
    pub resolution: String,
    pub refresh_rate: String,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallpaperInfo {
    pub name: String,
    pub path: String,
    pub preview_path: Option<String>,
}

fn get_config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    PathBuf::from(home).join(".config/wpe-next/config.toml")
}

const CONFIG_SNIPPET: &str = r#"# ///////////////////////////////////////////////
# wpe-next config
# 
# Each monitor starts with [[monitor]]:
#   name = "Monitor-Name"     # Output name from wlr-randr
#   enabled = true/false       # Whether to show wallpaper on this monitor
#   wallpaper = "Folder-Name" # Name of folder in ~/.config/wpe-next/
#
# Wallpaper folders should contain:
#   - index.html (required)    # Main wallpaper HTML file
#   - preview.* (optional)    # Preview image (png, jpg, gif, webp, webm)
#
# Example:
# [[monitor]]
# name = "HDMI-A-1"
# enabled = true
# wallpaper = "My Wallpaper"
# ///////////////////////////////////////////////

"#;

fn get_config_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    PathBuf::from(home).join(".config/wpe-next")
}

#[tauri::command]
fn load_config() -> Vec<MonitorConfig> {
    let path = get_config_path();
    if !path.exists() {
        return vec![];
    }

    let content = fs::read_to_string(&path).unwrap_or_default();
    let value: toml::Value = match content.parse() {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let mut monitors = vec![];
    if let Some(monitor_list) = value.get("monitor").and_then(|v| v.as_array()) {
        for m in monitor_list {
            if let Some(m) = m.as_table() {
                let name = m
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let enabled = m.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
                let wallpaper = m
                    .get("wallpaper")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .filter(|s| !s.is_empty());
                monitors.push(MonitorConfig {
                    name,
                    enabled,
                    wallpaper,
                });
            }
        }
    }
    monitors
}

#[tauri::command]
fn save_config(monitors: Vec<MonitorConfig>) -> Result<(), String> {
    let path = get_config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let mut buf = String::new();
    buf.push_str("[general]\n\n");
    for m in monitors {
        buf.push_str("[[monitor]]\n");
        buf.push_str(&format!("name = \"{}\"\n", m.name));
        buf.push_str(&format!("enabled = {}\n", m.enabled));
        if let Some(wp) = m.wallpaper {
            buf.push_str(&format!("wallpaper = \"{}\"\n", wp));
        }
        buf.push('\n');
    }

    fs::write(&path, buf).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_monitors() -> Vec<MonitorInfo> {
    let output = match process::Command::new("wlr-randr").output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return vec![],
    };

    let mut monitors = vec![];
    let mut current_monitor: Option<String> = None;
    let mut resolution: Option<String> = None;
    let mut refresh_rate: Option<String> = None;
    let mut x = 0;
    let mut y = 0;

    for line in output.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("HDMI-")
            || trimmed.starts_with("DP-")
            || trimmed.starts_with("LVDS")
            || trimmed.starts_with("eDP")
            || trimmed.starts_with("Virtual")
        {
            if let Some(prev) = current_monitor.take() {
                monitors.push(MonitorInfo {
                    name: prev,
                    resolution: resolution.take().unwrap_or_else(|| "Unknown".to_string()),
                    refresh_rate: refresh_rate.take().unwrap_or_else(|| "Unknown".to_string()),
                    x,
                    y,
                });
            }
            current_monitor = Some(trimmed.split_whitespace().next().unwrap_or("").to_string());
            resolution = None;
            refresh_rate = None;
            x = 0;
            y = 0;
            continue;
        }

        if trimmed.starts_with("Position:") {
            if let Some(pos) = trimmed.split(':').nth(1) {
                let parts: Vec<&str> = pos.trim().split(',').collect();
                if parts.len() >= 2 {
                    x = parts[0].parse().unwrap_or(0);
                    y = parts[1].parse().unwrap_or(0);
                }
            }
        }

        if trimmed.contains("current") && trimmed.contains("px") {
            let clean = trimmed
                .replace("current", "")
                .replace("preferred", "")
                .replace("px", "")
                .replace("Hz", "")
                .replace("(", "")
                .replace(")", "")
                .replace(",", "")
                .trim()
                .to_string();

            if let Some((w, rest)) = clean.split_once('x') {
                let parts: Vec<&str> = rest.trim().split_whitespace().collect();
                if parts.len() >= 1 {
                    resolution = Some(format!("{}x{}", w, parts[0]));
                    if parts.len() >= 2 {
                        refresh_rate = Some(format!("{} Hz", parts[1]));
                    }
                }
            }
        }
    }

    if let Some(name) = current_monitor {
        monitors.push(MonitorInfo {
            name,
            resolution: resolution.unwrap_or_else(|| "Unknown".to_string()),
            refresh_rate: refresh_rate.unwrap_or_else(|| "Unknown".to_string()),
            x,
            y,
        });
    }

    monitors
}

#[tauri::command]
fn get_wallpapers() -> Vec<WallpaperInfo> {
    let config_dir = get_config_dir();
    let config_dir_str = config_dir.to_string_lossy();
    let mut wallpapers = vec![];

    if let Ok(entries) = fs::read_dir(&config_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let index_path = path.join("index.html");
                if index_path.exists() {
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    let path_str = path.to_string_lossy();
                    let relative = path_str
                        .strip_prefix(&*config_dir_str)
                        .unwrap_or(&path_str)
                        .trim_start_matches('/')
                        .to_string();

                    let mut preview_path: Option<String> = None;
                    for ext in &["webm", "gif", "webp", "png", "jpg", "jpeg", "bmp"] {
                        let preview = path.join(format!("preview.{}", ext));
                        if preview.exists() {
                            let preview_str = preview.to_string_lossy();
                            let preview_relative = preview_str
                                .strip_prefix(&*config_dir_str)
                                .unwrap_or(&preview_str)
                                .trim_start_matches('/')
                                .to_string();
                            preview_path = Some(preview_relative);
                            break;
                        }
                    }

                    wallpapers.push(WallpaperInfo {
                        name,
                        path: relative,
                        preview_path,
                    });
                }
            }
        }
    }

    wallpapers
}

#[tauri::command]
fn start_wallpapers(port: u16) -> Result<(), String> {
    let helper_path = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .map(|p| p.join("wpe-next-helper"))
        .ok_or("Could not determine helper path")?;

    if !helper_path.exists() {
        return Err("Helper not found".to_string());
    }

    process::Command::new(&helper_path)
        .arg("-g")
        .arg(port.to_string())
        .spawn()
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
fn stop_wallpapers() -> Result<(), String> {
    process::Command::new("pkill")
        .arg("-f")
        .arg("wpe-next-helper")
        .spawn()
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
fn is_helper_running() -> bool {
    process::Command::new("pgrep")
        .arg("-f")
        .arg("wpe-next-helper")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[tauri::command]
fn open_wallpaper_folder() -> Result<(), String> {
    let config_dir = get_config_dir();
    process::Command::new("xdg-open")
        .arg(&config_dir)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn init_samples() -> Result<(), String> {
    let config_dir = get_config_dir();
    let config_path = get_config_path();

    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
    }

    let entries = fs::read_dir(&config_dir).map_err(|e| e.to_string())?;
    let has_wallpaper = entries
        .flatten()
        .filter_map(|e| e.path().is_dir().then(|| e.path()))
        .any(|p| p.join("index.html").exists());

    if !config_path.exists() && has_wallpaper {
        let monitors = get_monitors();
        let mut config_content = CONFIG_SNIPPET.to_string();
        config_content.push_str("\n");
        for m in monitors {
            config_content.push_str("[[monitor]]\n");
            config_content.push_str(&format!("name = \"{}\"\n", m.name));
            config_content.push_str("enabled = false\n");
        }
        fs::write(&config_path, config_content).map_err(|e| e.to_string())?;
    }

    if config_path.exists() {
        return Ok(());
    }

    if has_wallpaper {
        return Ok(());
    }

    let sample_data = samples::get_samples();
    for (folder_name, files) in sample_data {
        let folder_path = config_dir.join(&folder_name);
        fs::create_dir_all(&folder_path).map_err(|e| e.to_string())?;
        for (file_name, data) in files {
            let file_path = folder_path.join(&file_name);
            fs::write(&file_path, data).map_err(|e| e.to_string())?;
        }
    }

    let monitors = get_monitors();
    let mut config_content = CONFIG_SNIPPET.to_string();
    config_content.push_str("\n");
    for m in monitors {
        config_content.push_str("[[monitor]]\n");
        config_content.push_str(&format!("name = \"{}\"\n", m.name));
        config_content.push_str("enabled = false\n");
    }
    fs::write(&config_path, config_content).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
fn start_host() -> Result<u16, String> {
    let helper_path = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .map(|p| p.join("wpe-next-host"))
        .ok_or("Could not determine host path")?;

    if !helper_path.exists() {
        return Err("Host not found".to_string());
    }

    let mut child = process::Command::new(&helper_path)
        .stdout(process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;

    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let reader = BufReader::new(stdout);

    let mut lines = reader.lines();
    let port_str = lines
        .next()
        .ok_or("Failed to read port")?
        .map_err(|e| e.to_string())?;

    let port: u16 = port_str.trim().parse::<u16>().map_err(|e| e.to_string())?;

    Ok(port)
}

fn main() {
    overlay::start_overlay();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            load_config,
            save_config,
            get_monitors,
            get_wallpapers,
            start_wallpapers,
            stop_wallpapers,
            is_helper_running,
            open_wallpaper_folder,
            start_host,
            init_samples,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
