use log::info;
use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crate::config::get_config_dir;
use crate::url::urlencoding_decode;

const PORTS: [u16; 4] = [19761, 19762, 19763, 19764];

pub fn run_server(
    active_connections: Arc<AtomicUsize>,
    last_heartbeat: Arc<std::sync::Mutex<Instant>>,
) {
    let config_dir = get_config_dir();

    info!("Serving files from: {:?}", config_dir);

    for port in PORTS {
        let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();

        match tiny_http::Server::http(addr) {
            Ok(server) => {
                info!("Successfully bound to 127.0.0.1:{}", port);
                println!("{}", port);

                loop {
                    match server.recv_timeout(std::time::Duration::from_millis(500)) {
                        Ok(Some(request)) => {
                            active_connections.fetch_add(1, Ordering::SeqCst);
                            *last_heartbeat.lock().unwrap() = Instant::now();

                            handle_request(request, &config_dir, &active_connections);
                        }
                        Ok(None) => {
                            if last_heartbeat.lock().unwrap().elapsed()
                                > std::time::Duration::from_secs(1)
                            {
                                info!("No heartbeat for 1 second, shutting down");
                                return;
                            }
                        }
                        Err(e) => {
                            info!("Server error: {}", e);
                            return;
                        }
                    }
                }
            }
            Err(e) => {
                info!("Failed to bind to 127.0.0.1:{}: {}", port, e);
                continue;
            }
        }
    }

    eprintln!("Could not bind to any port");
    std::process::exit(1);
}

fn handle_request(
    request: tiny_http::Request,
    config_dir: &PathBuf,
    active_connections: &Arc<AtomicUsize>,
) {
    let url = request.url().to_string();

    if url == "/ping" || url == "/ping/" {
        send_response(request, "ok", "text/plain", active_connections);
        return;
    }

    if url == "/heartbeat" || url == "/heartbeat/" {
        send_response(request, "ok", "text/plain", active_connections);
        return;
    }

    if url == "/config" || url == "/config/" {
        let config_path = config_dir.join("config.toml");
        if config_path.exists() {
            if let Ok(data) = fs::read(&config_path) {
                send_data_response(request, data, "application/toml", active_connections);
            } else {
                send_error(request, 500, "Internal Server Error", active_connections);
            }
        } else {
            send_response(request, "[]", "application/json", active_connections);
        }
        return;
    }

    let path = match urlencoding_decode(url.trim_start_matches('/')) {
        Ok(p) => p,
        Err(_) => {
            send_error(request, 400, "Bad Request", active_connections);
            return;
        }
    };

    let full_path = config_dir.join(&path);

    let canonical = match full_path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            send_error(request, 404, "Not Found", active_connections);
            return;
        }
    };

    let config_dir_canonical = match config_dir.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            send_error(request, 500, "Internal Error", active_connections);
            return;
        }
    };

    if !canonical.starts_with(&config_dir_canonical) {
        send_error(request, 403, "Forbidden", active_connections);
        return;
    }

    if full_path.exists() {
        if full_path.is_dir() {
            let index = full_path.join("index.html");
            if index.exists() {
                if let Ok(data) = fs::read(&index) {
                    send_data_response(request, data, "text/html", active_connections);
                } else {
                    send_error(request, 500, "Internal Server Error", active_connections);
                }
            } else {
                send_error(
                    request,
                    403,
                    "Directory listing not allowed",
                    active_connections,
                );
            }
        } else {
            serve_file(request, &full_path, active_connections);
        }
    } else {
        send_error(request, 404, "Not Found", active_connections);
    }
}

fn serve_file(
    request: tiny_http::Request,
    full_path: &PathBuf,
    active_connections: &Arc<AtomicUsize>,
) {
    let path_str = full_path.to_string_lossy();
    let mime = if path_str.ends_with(".html") || path_str.ends_with(".htm") {
        "text/html"
    } else if path_str.ends_with(".js") {
        "application/javascript"
    } else if path_str.ends_with(".css") {
        "text/css"
    } else if path_str.ends_with(".png") {
        "image/png"
    } else if path_str.ends_with(".jpg") || path_str.ends_with(".jpeg") {
        "image/jpeg"
    } else if path_str.ends_with(".gif") {
        "image/gif"
    } else if path_str.ends_with(".webp") {
        "image/webp"
    } else if path_str.ends_with(".webm") {
        "video/webm"
    } else if path_str.ends_with(".svg") {
        "image/svg+xml"
    } else if path_str.ends_with(".json") {
        "application/json"
    } else if path_str.ends_with(".toml") {
        "application/toml"
    } else {
        "application/octet-stream"
    };

    match fs::read(full_path) {
        Ok(data) => send_data_response(request, data, mime, active_connections),
        Err(_) => send_error(request, 403, "Forbidden", active_connections),
    }
}

fn send_response(
    request: tiny_http::Request,
    body: &str,
    mime: &str,
    active_connections: &Arc<AtomicUsize>,
) {
    let response = tiny_http::Response::from_string(body)
        .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], mime.as_bytes()).unwrap());
    let _ = request.respond(response);
    active_connections.fetch_sub(1, Ordering::SeqCst);
}

fn send_data_response(
    request: tiny_http::Request,
    data: Vec<u8>,
    mime: &str,
    active_connections: &Arc<AtomicUsize>,
) {
    let response = tiny_http::Response::from_data(data)
        .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], mime.as_bytes()).unwrap());
    let _ = request.respond(response);
    active_connections.fetch_sub(1, Ordering::SeqCst);
}

fn send_error(
    request: tiny_http::Request,
    code: u16,
    msg: &str,
    active_connections: &Arc<AtomicUsize>,
) {
    let response = tiny_http::Response::from_string(msg).with_status_code(code);
    let _ = request.respond(response);
    active_connections.fetch_sub(1, Ordering::SeqCst);
}
