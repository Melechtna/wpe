mod config;
mod host;
mod monitor;
mod monitors;
mod window;

use std::env;
use std::fs;

use monitors::get_monitors;
use window::spawn_wallpaper_window;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args: Vec<String> = env::args().collect();

    let monitor_arg = args
        .iter()
        .position(|a| a == "--monitor")
        .and_then(|i| args.get(i + 1));
    let wallpaper_arg = args
        .iter()
        .position(|a| a == "--wallpaper")
        .and_then(|i| args.get(i + 1));
    let port_arg = args
        .iter()
        .position(|a| a == "--port")
        .and_then(|i| args.get(i + 1));

    if let (Some(monitor), Some(wallpaper), Some(port_str)) = (monitor_arg, wallpaper_arg, port_arg)
    {
        let port: u16 = port_str.parse().expect("Invalid port");
        log::info!("Worker mode: {} - {}", monitor, wallpaper);
        spawn_wallpaper_window(monitor, wallpaper, Some(port));
        return;
    }

    let http_port: u16 = if args.len() >= 2 && args[1] == "-g" {
        args[2].parse().expect("Invalid port")
    } else {
        let config_dir = config::get_config_dir();
        let config_path = config::get_config_path();

        let entries = fs::read_dir(&config_dir).ok();
        let has_wallpaper = entries
            .map(|e| {
                e.flatten()
                    .filter_map(|e| e.path().is_dir().then(|| e.path()))
                    .any(|p| p.join("index.html").exists())
            })
            .unwrap_or(false);

        if !config_path.exists() && !has_wallpaper {
            eprintln!("No wallpapers found. Please add wallpaper folders to ~/.config/wpe-next/");
            eprintln!("Each folder should contain at least an index.html file.");
            eprintln!("Then create a config.toml with [[monitor]] entries.");
            std::process::exit(1);
        }

        let port = host::spawn_host().expect("Failed to spawn host");
        log::info!("Host spawned on port {}", port);
        port
    };

    let config = match host::fetch_config_from_host(http_port) {
        Some(c) => c,
        None => {
            log::error!("Failed to fetch config from host");
            std::process::exit(1);
        }
    };

    let enabled_monitors: Vec<_> = config
        .iter()
        .filter(|m| m.enabled && m.wallpaper.is_some())
        .collect();

    if enabled_monitors.is_empty() {
        log::warn!("No enabled monitors with wallpapers found.");
        std::process::exit(0);
    }

    log::info!(
        "Found {} monitors to spawn wallpapers for",
        enabled_monitors.len()
    );
    let monitors = get_monitors();
    log::info!("Detected {} monitors", monitors.len());

    let exe_path = std::env::current_exe().expect("Failed to get executable path");
    let exe_path_str = exe_path.to_string_lossy().to_string();

    for monitor_config in enabled_monitors {
        if let Some(wallpaper) = &monitor_config.wallpaper {
            let target_monitor = monitors.iter().find(|m| m.name == monitor_config.name);
            if target_monitor.is_none() {
                log::warn!(
                    "Monitor {} not found in detected monitors, skipping",
                    monitor_config.name
                );
                continue;
            }
            log::info!(
                "Spawning child process for {}: {}",
                monitor_config.name,
                wallpaper
            );

            let mut cmd = std::process::Command::new(&exe_path_str);
            cmd.arg("--monitor").arg(&monitor_config.name);
            cmd.arg("--wallpaper").arg(wallpaper);
            cmd.arg("--port").arg(http_port.to_string());
            cmd.spawn().expect("Failed to spawn wallpaper process");
        }
    }

    log::info!("All wallpaper processes spawned, helper exiting");
}
