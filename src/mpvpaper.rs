use std::{
    error::Error,
    process::{Child, Command, Stdio},
};

use tracing::info;

use crate::config::{MediaKind, RuntimeConfig, ScaleMode, SlideshowOrder};

/// Spawn mpvpaper
pub fn spawn_instance(config: &RuntimeConfig) -> Result<Child, Box<dyn Error>> {
    let monitor = config
        .monitor
        .as_deref()
        .ok_or_else(|| "Wallpaper entry is missing a monitor assignment".to_string())?;
    let input_path = config.media.path();

    let mut command = Command::new("mpvpaper");

    if let MediaKind::Folder(_) = &config.media {
        let seconds = config.slideshow.interval.as_secs().max(1);
        command.arg("-n").arg(seconds.to_string());
    }

    let mpv_options = build_mpv_options(config);
    if !mpv_options.is_empty() {
        let joined = mpv_options.join(" ");
        command.arg("-o").arg(joined);
    }

    command.arg(monitor);
    command.arg(input_path);
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());

    info!(
        "Launching mpvpaper for {} with source {}",
        monitor,
        input_path.display()
    );

    command
        .spawn()
        .map_err(|err| format!("Failed to launch mpvpaper for {monitor}: {err}").into())
}

fn build_mpv_options(config: &RuntimeConfig) -> Vec<String> {
    let mut options = Vec::new();
    options.push("--no-audio".into());
    options.push("--osc=no".into());
    options.push("--no-osd-bar".into());
    options.push("--hwdec=auto-safe".into());

    match config.media {
        MediaKind::Folder(_) => match config.slideshow.order {
            SlideshowOrder::Random => options.push("--shuffle".into()),
            SlideshowOrder::Sequential => options.push("--no-shuffle".into()),
        },
        _ => {
            options.push("--loop-file=inf".into());
        }
    }

    match config.scale {
        ScaleMode::Fit => options.push("--keepaspect=no".into()),
        ScaleMode::Stretch => options.push("--keepaspect=yes".into()),
        ScaleMode::Original => {
            options.push("--keepaspect=yes".into());
            options.push("--video-unscaled=downscale-big".into());
        }
    }

    options
}
