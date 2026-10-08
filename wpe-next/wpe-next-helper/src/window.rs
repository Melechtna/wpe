use gtk::prelude::*;
use libc::c_int;
use webkit2gtk::WebViewExt;

use crate::monitors::get_monitors;

#[repr(i32)]
#[derive(Clone, Copy)]
pub enum LayerShellLayer {
    Background = 0,
    Bottom = 1,
    Top = 2,
    Overlay = 3,
}

extern "C" {
    fn gtk_layer_init_for_window(window: *mut std::ffi::c_void) -> i32;
    fn gtk_layer_set_layer(window: *mut std::ffi::c_void, layer: c_int) -> i32;
    fn gtk_layer_set_exclusive_zone(window: *mut std::ffi::c_void, zone: c_int) -> i32;
    fn gtk_layer_set_monitor(window: *mut std::ffi::c_void, monitor: *mut std::ffi::c_void) -> i32;

    fn gdk_display_get_default() -> *mut std::ffi::c_void;
    fn gdk_display_get_monitor_at_point(
        display: *mut std::ffi::c_void,
        x: i32,
        y: i32,
    ) -> *mut std::ffi::c_void;
}

fn init_layer_shell_for_monitor(
    window: &gtk::Window,
    monitor_name: &str,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) {
    unsafe {
        let ptr = window.as_ptr() as *mut std::ffi::c_void;

        if gtk_layer_init_for_window(ptr) != 0 {
            log::info!("Layer shell initialized for {}", monitor_name);
        } else {
            log::warn!("Layer shell init failed");
            return;
        }

        gtk_layer_set_layer(ptr, LayerShellLayer::Background as c_int);

        let display = gdk_display_get_default();
        if !display.is_null() {
            let monitor = gdk_display_get_monitor_at_point(display, x, y);
            if !monitor.is_null() {
                gtk_layer_set_monitor(ptr, monitor);
                log::info!("Set layer monitor at {},{}", x, y);
            } else {
                log::warn!("Could not get monitor at {},{}", x, y);
            }
        } else {
            log::warn!("Could not get default display");
        }

        gtk_layer_set_exclusive_zone(ptr, height);
        log::info!(
            "Layer shell configured for {} at {}x{}+{}+{}",
            monitor_name,
            width,
            height,
            x,
            y
        );
    }
}

pub fn urlencoding_encode(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => result.push(c),
            ' ' => result.push_str("%20"),
            _ => {
                for byte in c.to_string().as_bytes() {
                    result.push_str(&format!("%{:02X}", byte));
                }
            }
        }
    }
    result
}

pub fn spawn_wallpaper_window(monitor_name: &str, wallpaper_path: &str, port: Option<u16>) {
    log::info!("Worker mode: {} - {}", monitor_name, wallpaper_path);

    let monitors = get_monitors();
    log::info!("Detected {} monitors", monitors.len());

    let target_monitor = monitors
        .iter()
        .find(|m| m.name == monitor_name || m.connector == monitor_name);

    let (width, height, x, y) = if let Some(monitor) = target_monitor {
        log::info!("Found matching monitor: {}", monitor.name);
        let logical_width = (monitor.width as f64 / monitor.scale) as u32;
        let logical_height = (monitor.height as f64 / monitor.scale) as u32;
        log::info!(
            "Monitor {} scale: {}, size: {}x{} at {},{}",
            monitor.name,
            monitor.scale,
            logical_width,
            logical_height,
            monitor.x,
            monitor.y
        );
        (logical_width, logical_height, monitor.x, monitor.y)
    } else {
        log::warn!("No target monitor found for {}!", monitor_name);
        (1920, 1080, 0, 0)
    };

    gtk::init().expect("Failed to initialize GTK");

    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title("wpe-next-wallpaper");
    window.set_default_size(width as i32, height as i32);
    window.set_decorated(false);
    window.set_skip_taskbar_hint(true);
    window.set_skip_pager_hint(true);
    window.set_resizable(false);
    window.set_size_request(width as i32, height as i32);

    init_layer_shell_for_monitor(&window, monitor_name, x, y, width as i32, height as i32);

    let webview = webkit2gtk::WebView::new();
    webview.set_size_request(width as i32, height as i32);

    let uri = if let Some(p) = port {
        let relative = wallpaper_path.trim_start_matches('/');
        let encoded = urlencoding_encode(relative);
        log::info!("Relative path: {} -> encoded: {}", relative, encoded);
        format!("http://127.0.0.1:{}/{}/index.html", p, encoded)
    } else {
        format!("file://{}/index.html", wallpaper_path)
    };
    log::info!("Loading URI: {}", uri);
    webview.load_uri(&uri);

    window.add(&webview);
    window.show_all();
    window.move_(x, y);

    if let Some(p) = port {
        let heartbeat_port = p;
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let _ = ureq::get(&format!("http://127.0.0.1:{}/heartbeat", heartbeat_port)).call();
        });
    }

    log::info!(
        "Wallpaper window created for {} at {},{}",
        monitor_name,
        x,
        y
    );
    gtk::main();
}
