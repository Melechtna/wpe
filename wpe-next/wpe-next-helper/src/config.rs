use std::env;
use std::path::PathBuf;

pub fn get_config_path() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    PathBuf::from(home).join(".config/wpe-next/config.toml")
}

pub fn get_config_dir() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    PathBuf::from(home).join(".config/wpe-next")
}
