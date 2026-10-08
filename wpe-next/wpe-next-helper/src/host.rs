use std::io::BufRead;
use std::path::PathBuf;

use crate::monitor::MonitorConfig;

pub fn get_host_path() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("wpe-next-host")))
        .filter(|p| p.exists())
}

pub fn wait_for_host(port: u16) -> bool {
    for _ in 0..50 {
        if ureq::get(&format!("http://127.0.0.1:{}/ping", port))
            .call()
            .is_ok()
        {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    false
}

pub fn spawn_host() -> Option<u16> {
    let host_path = get_host_path()?;

    let mut child = std::process::Command::new(&host_path)
        .stdout(std::process::Stdio::piped())
        .spawn()
        .ok()?;

    let stdout = child.stdout.take()?;
    let reader: std::io::BufReader<std::process::ChildStdout> = std::io::BufReader::new(stdout);
    let mut lines = reader.lines();
    let port_str = lines.next()?.ok()?;

    let port: u16 = port_str.trim().parse().ok()?;

    if !wait_for_host(port) {
        return None;
    }

    Some(port)
}

pub fn fetch_config_from_host(port: u16) -> Option<Vec<MonitorConfig>> {
    let url = format!("http://127.0.0.1:{}/config", port);
    let response = ureq::get(&url).call().ok()?;
    let config_str = response.into_string().ok()?;

    let value: toml::Value = config_str.parse().ok()?;
    let monitors = value.get("monitor")?.as_array()?;

    let mut result = Vec::new();
    for m in monitors {
        if let Some(table) = m.as_table() {
            let name = table.get("name")?.as_str()?.to_string();
            let enabled = table
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let wallpaper = table
                .get("wallpaper")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            result.push(MonitorConfig {
                name,
                enabled,
                wallpaper,
            });
        }
    }

    Some(result)
}
