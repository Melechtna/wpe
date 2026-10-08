# wpe-next

A Wayland wallpaper engine using Tauri + WebKitGTK to render live web-based wallpapers.

## Dependencies

### Required System Packages

**Arch Linux:**
```sh
sudo pacman -S webkit2gtk-4.1 gtk-layer-shell gtk3 libappindicator-gtk3 wlr-randr
```

**Debian/Ubuntu:**
```sh
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libgtk-layer-shell0-dev wlr-randr
```

**Fedora:**
```sh
sudo dnf install webkit2gtk4.1-devel gtk3-devel gtk-layer-shell-devel libappindicator-gtk3-devel librsvg2-devel wlr-randr
```

**NixOS:**
```nix
# Add to your configuration.nix
environment.systemPackages = with pkgs; [
  webkitgtk_4_1
  gtk-layer-shell
  gtk3
  librsvg
  wlr-randr
];
```

### Runtime Requirement

- **`wlr-randr`** - Required at runtime for monitor detection. If not installed, the GUI will show no monitors.

### Important Notes

- **Wayland session required** - This tool is designed for Wayland compositors that properly implement the GTK Layer Shell protocol. Compatible compositors include: sway, river, Hyprland, GNOME, COSMIC, XFCE, and most others.

  **KDE Plasma on Wayland is NOT supported** due to its non-standard handling of layer-shell surfaces. It will not work correctly regardless of dependencies installed.

  This will NOT work on X11 sessions at all.
- **Tauri 2.x requires `libwebkit2gtk-4.1-dev`** (not 4.0). Most older guides and systems have 4.0, which will cause rendering issues.
- If the web UI isn't rendering properly (blank/white screens), you likely have the wrong webkit2gtk version.
- `gtk-layer-shell` is required for the wallpaper windows to sit properly on the background layer in Wayland.

## Building

```sh
# Install Rust if you haven't
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build all components
just build

# Output goes to dist/
```

## Architecture

- **wpe-next** - Tauri GUI for configuration
- **wpe-next-host** - HTTP server that serves wallpaper files
- **wpe-next-helper** - Spawns wallpaper windows on monitors
