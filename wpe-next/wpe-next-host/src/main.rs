mod config;
mod server;
mod url;

use std::sync::Arc;
use std::time::Instant;

use server::run_server;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config_dir = config::get_config_dir();
    if !config_dir.exists() {
        eprintln!("Config directory does not exist: {:?}", config_dir);
        std::process::exit(1);
    }

    let active_connections = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let last_heartbeat = Arc::new(std::sync::Mutex::new(Instant::now()));

    *last_heartbeat.lock().unwrap() = Instant::now();

    run_server(active_connections, last_heartbeat);
}
