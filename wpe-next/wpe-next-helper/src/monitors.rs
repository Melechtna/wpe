#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub name: String,
    pub connector: String,
    pub width: u32,
    pub height: u32,
    pub refresh_rate: f64,
    pub x: i32,
    pub y: i32,
    pub scale: f64,
}

pub fn get_monitors() -> Vec<MonitorInfo> {
    let mut monitors = Vec::new();

    if let Ok(output) = std::process::Command::new("wlr-randr").output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut current_connector = String::new();
            let mut width = 0u32;
            let mut height = 0u32;
            let mut refresh_rate = 60.0f64;
            let mut x = 0i32;
            let mut y = 0i32;
            let mut scale = 1.0f64;
            let mut in_modes = false;

            for line in stdout.lines() {
                let trimmed = line.trim();

                if trimmed.starts_with("HDMI-")
                    || trimmed.starts_with("DP-")
                    || trimmed.starts_with("LVDS")
                    || trimmed.starts_with("eDP")
                    || trimmed.starts_with("Virtual")
                {
                    if !current_connector.is_empty() && width > 0 && height > 0 {
                        monitors.push(MonitorInfo {
                            name: current_connector.clone(),
                            connector: current_connector.clone(),
                            width,
                            height,
                            refresh_rate,
                            x,
                            y,
                            scale,
                        });
                    }
                    current_connector = trimmed.split_whitespace().next().unwrap_or("").to_string();
                    width = 0;
                    height = 0;
                    refresh_rate = 60.0;
                    x = 0;
                    y = 0;
                    scale = 1.0;
                    in_modes = false;
                    continue;
                }

                if trimmed.starts_with("Modes:") {
                    in_modes = true;
                    continue;
                }

                if in_modes
                    && !trimmed.is_empty()
                    && !trimmed.starts_with("Make:")
                    && !trimmed.starts_with("Model:")
                    && !trimmed.starts_with("Serial:")
                    && !trimmed.starts_with("Physical")
                    && !trimmed.starts_with("Enabled:")
                    && !trimmed.starts_with("Position:")
                    && !trimmed.starts_with("Transform:")
                    && !trimmed.starts_with("Scale:")
                    && !trimmed.starts_with("Adaptive")
                {
                    if trimmed.contains("current") {
                        let clean_mode = trimmed
                            .replace("current", "")
                            .replace("preferred", "")
                            .replace("px", "")
                            .replace("Hz", "")
                            .replace("(", "")
                            .replace(")", "")
                            .replace(",", "")
                            .trim()
                            .to_string();

                        if let Some((w_str, rest)) = clean_mode.split_once('x') {
                            let parts: Vec<&str> = rest.trim().split_whitespace().collect();
                            if parts.len() >= 1 {
                                if let (Ok(wi), Ok(hi)) =
                                    (w_str.trim().parse::<u32>(), parts[0].parse::<u32>())
                                {
                                    width = wi;
                                    height = hi;
                                }
                                if parts.len() >= 2 {
                                    if let Ok(rate) = parts[1].parse::<f64>() {
                                        refresh_rate = rate;
                                    }
                                }
                            }
                        }
                    }
                }

                if trimmed.starts_with("Position:") {
                    in_modes = false;
                    if let Some(pos) = trimmed.split(':').nth(1) {
                        let parts: Vec<&str> = pos.trim().split(',').collect();
                        if parts.len() >= 2 {
                            if let (Ok(px), Ok(py)) =
                                (parts[0].parse::<i32>(), parts[1].parse::<i32>())
                            {
                                x = px;
                                y = py;
                            }
                        }
                    }
                }

                if trimmed.starts_with("Scale:") {
                    if let Some(s) = trimmed.split(':').nth(1) {
                        if let Ok(scaled) = s.trim().parse::<f64>() {
                            scale = scaled;
                        }
                    }
                }
            }

            if !current_connector.is_empty() && width > 0 && height > 0 {
                monitors.push(MonitorInfo {
                    name: current_connector.clone(),
                    connector: current_connector.clone(),
                    width,
                    height,
                    refresh_rate,
                    x,
                    y,
                    scale,
                });
            }
        }
    }
    monitors
}
