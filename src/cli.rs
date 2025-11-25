use clap::Parser;

/// CLI switches for launching wallpapers or the GUI.
#[derive(Parser, Debug)]
#[command(name = "wpe", about = "WallPaper Engine")]
pub struct Args {
    /// Launch configured wallpapers using ~/.config/wpe/config.toml.
    #[arg(short = 'c', long = "config", help = "Launch configured wallpapers")]
    pub use_config: bool,
}
