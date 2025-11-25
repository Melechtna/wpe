use std::error::Error;

use tracing::info;

use crate::{
    config::{self, RuntimeConfig, WallpaperProfileEntry},
    monitors, mpvpaper,
};

/// Launch a wallpaper instance for each configured entry in config.toml.
/// mpvpaper processes are spawned directly and left running so they can be
/// stopped later with a simple `pkill mpvpaper`.
pub fn launch_from_profile() -> Result<(), Box<dyn Error>> {
    let monitors = monitors::list_monitors()?;
    let (entries, created, path) = config::ensure_profile_for_monitors(&monitors)?;

    if created {
        println!("Created default config at {}.", path.display());
        println!("Edit this file to choose wallpapers, then rerun `wpe -c`.");
        return Ok(());
    }

    let targets = select_targets(&entries);
    if targets.is_empty() {
        println!(
            "No enabled wallpaper entries in {} have a configured path.",
            path.display()
        );
        println!("Set `enabled = true` and provide a valid path, then rerun `wpe -c`.");
        return Ok(());
    }

    for index in &targets {
        let runtime = match RuntimeConfig::from_entry(*index) {
            Ok(runtime) => runtime,
            Err(err) => return Err(err),
        };

        mpvpaper::spawn_instance(&runtime)?;
    }

    info!(
        "Launched {} wallpaper instance(s) based on config entries.",
        targets.len()
    );
    println!(
        "Started {} mpvpaper instance(s). Stop them with `pkill mpvpaper`.",
        targets.len()
    );
    Ok(())
}

fn select_targets(entries: &[WallpaperProfileEntry]) -> Vec<usize> {
    entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.enabled && entry.path.is_some())
        .map(|(index, _)| index)
        .collect()
}
