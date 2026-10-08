#[derive(Debug, Clone)]
pub struct MonitorConfig {
    pub name: String,
    pub enabled: bool,
    pub wallpaper: Option<String>,
}
