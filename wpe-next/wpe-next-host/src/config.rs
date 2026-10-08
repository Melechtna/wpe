use std::path::PathBuf;

pub fn get_config_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    PathBuf::from(home).join(".config/wpe-next")
}
