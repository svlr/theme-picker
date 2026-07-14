# theme-picker

GTK4 wallpaper picker with a paginated thumbnail grid, keyboard/mouse navigation, and hook scripts for applying themes.

## Dependencies

Runtime:
- `vipsthumbnail` (from `libvips`) — thumbnail generation
- a hook script that applies the wallpaper (example below)

Build (Rust):

```toml
[dependencies]
gtk4 = { version = "0.9", features = ["v4_10"] }
glib = "0.20"
walkdir = "2.5"
sha2 = "0.10"
serde = { version = "1.0", features = ["derive"] }
toml = "0.8"
async-channel = "2.3"
```

## Build

Clone and build:

```bash
git clone https://github.com/svlr/theme-picker
cd theme-picker
cargo build --release
```

Binary ends up at `target/release/theme-picker`.

Or install directly with cargo:

```bash
cargo install --git https://github.com/svlr/theme-picker
```

## Configuration

Config file: `~/.config/theme-picker/config.toml`

| Parameter | Description |
|---|---|
| `wallpaper_dir` | directory scanned for wallpapers (top level only) |
| `thumb_cache_dir` | where generated thumbnails are cached |
| `drivers.image` | enable/disable image wallpapers |
| `drivers.video` | enable/disable video wallpapers (routing exists, unused by default) |
| `hooks.image` | script run when an image wallpaper is selected |
| `hooks.video` | script run when a video wallpaper is selected (optional) |

Example:

```toml
wallpaper_dir = "/home/user/Pictures/Wallpapers"
thumb_cache_dir = "/home/user/.cache/theme-picker/thumbs"

[drivers]
image = true
video = false

[hooks]
image = "/home/user/.config/theme-picker/set-theme.sh"
# video = "/home/user/.config/theme-picker/set-theme-video.sh"
```

## Hook script

A hook is any executable that receives the wallpaper's path as its first argument and applies it however you like (set the wallpaper, regenerate a color scheme, reload bars/terminals, etc). It's called on click or on `Enter`, without waiting for it to finish.

Example for Hyprland + hyprpaper + matugen:

```bash
#!/usr/bin/env bash
set -euo pipefail

IMG="${1:?Usage: set-theme <path-to-image>}"

if [[ ! -f "$IMG" ]]; then
    echo "set-theme: file not found: $IMG" >&2
    exit 1
fi

MONITOR="$(hyprctl activeworkspace -j | jq -r '.monitor')"
if [[ -z "$MONITOR" || "$MONITOR" == "null" ]]; then
    echo "set-theme: failed to detect active monitor" >&2
    exit 1
fi

hyprctl hyprpaper wallpaper "${MONITOR},${IMG},cover"
matugen image "$IMG"
"$HOME/.config/waybar/scripts/launch.sh"
```

## Controls

| Key | Action |
|---|---|
| `←` `→` `↑` `↓` | move selection, cross page boundaries at the edges |
| `Enter` | apply selected wallpaper |
| `Escape` | close window |
| Mouse wheel | change page |
