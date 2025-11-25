mod cli;
mod config;
mod gui;
mod monitors;
mod mpvpaper;
mod profile_launcher;

use clap::Parser;
use cli::Args;
use tracing_subscriber::EnvFilter;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    if args.use_config {
        // Launch wallpapers from config.toml with -c (--config)
        profile_launcher::launch_from_profile()?;
    } else {
        // Launch the GUI
        gui::launch()?;
    }

    Ok(())
}
