# WallPaper Engine (WPE)

WPE is a lightweight GUI + CLI wrapper around [mpvpaper](https://github.com/GhostNaN/mpvpaper).  It is an Iced based frontend, provides TOML configuration, display detection, takes per-monitor settings and translates them into mpvpaper invocations.  

TLDR Frontend handles the configuration, mpvpaper does the rest.

## Features

- Per-monitor wallpaper configuration with scale/folder/shuffle with available timer
- Toggle each monitor to use as preferred
- Launches one mpvpaper instance per display while letting mpv automatically manage hardware accel via `--hwdec=auto-safe`
- `fit`/`stretch`/`original` scale options
- GUI front end or CLI allowing for easy autostart (.desktop file included)

## Dependencies

- Rust 1.78+
- [mpvpaper](https://github.com/GhostNaN/mpvpaper) plus its runtime prerequisites (mpv, wlroots compositor, etc.) installed on the system

## Installation

Clone the repo, ensure `mpvpaper` is installed, then run:

```bash
just build
just install
```

The `install` target copies the binary, desktop entries, icon, and metainfo file. `just uninstall` removes them.

## Usage

### CLI

```bash
wpe -c
```

On the first run the CLI creates `~/.config/wpe/config.toml` and exits so you can edit the file. Subsequent runs spawn one mpvpaper instance per configured `[[wallpapers]]` entry.

WallPaper Engine always launches mpvpaper with `--hwdec=auto-safe`, letting mpv fall back to software decode whenever the hardware path is unavailable. The CLI only starts entries whose `enabled` flag is `true`, so you can leave placeholders around without needing to configure. Similarly, folder specific options `order` and `interval_seconds`, can be ignored if the `path` is not a folder.

### GUI

The GUI lists every detected monitor, displays a per-monitor editor, and starts/stops the background mpvpaper instances via the Start/Stop buttons. A purple overlay will appear on each display so you can immediately tell which monitor you are editing.

## Configuration

Interactive edits from the GUI are stored in `~/.config/wpe/config.toml`.  The file is annotated with a banner that explains every field, and new configs are seeded with placeholder paths so you can see how to configure everything after first run if using CLI:

```toml
[[wallpapers]]
monitor = "DP-1"
enabled = true                      # set to false to skip launching this entry
path = "/your/image/or/folder/here" # The path to the image/video/folder
scale = "fit"                       # fit (the whole display), stretch (uniformly), or original (resolution, centered to screen)
order = "sequential"                # sequential or random (folders only)
interval_seconds = 300              # slideshow delay (folders only)
```

Every entry becomes an mpvpaper invocation, so folders are treated as playlists and the Start button launches as many mpvpaper processes as you have configured/enabled monitors.

## Contributing

Ultimately this should be fairly feature complete, but any improvements or added features are welcome!


## Packaging

I am not likely to attempt to do so myself, the justfile is simple enough to work with, as is compiling rust projects in general. As such, if anyone wishes to take the project and add it to a distributions repositories, you are of course, free to do so.
