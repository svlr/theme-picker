# theme-picker

A fast, native GTK4 wallpaper and theme picker. It shows a paginated grid of
thumbnails, lets you navigate by keyboard or mouse, and runs a hook script to
apply the selected wallpaper however you like.

Thumbnails are generated asynchronously in-process via libvips (FFI). Video
wallpapers are supported by extracting a still frame in-process via libav
(ffmpeg FFI), which is used both for the grid preview and as the color-palette
source for your hook.

## Features

- Paginated thumbnail grid that reflows live on window resize and fullscreen
- Keyboard and mouse navigation, mouse-wheel paging
- Favorites, stored as a separate view
- Image wallpapers: png, jpg, jpeg, webp
- Video wallpapers: mp4, webm, mkv (optional, off by default)
- Theme application delegated entirely to a user hook script

## Dependencies

Build and runtime require libvips, GTK4 and ffmpeg development libraries plus
pkg-config. They are linked into the binary like any FFI dependency; there is
no external command to configure.

Arch Linux

    sudo pacman -S libvips gtk4 ffmpeg pkgconf

Debian / Ubuntu

    sudo apt install libvips-dev libgtk-4-dev libavcodec-dev libavformat-dev \
        libavutil-dev libswscale-dev pkg-config

Fedora

    sudo dnf install vips-devel gtk4-devel ffmpeg-devel pkgconf-pkg-config

## Install

    cargo install --git https://github.com/SvlR/theme-picker

Or build from a clone:

    git clone https://github.com/SvlR/theme-picker
    cd theme-picker
    cargo build --release

The binary is at `target/release/theme-picker`.

## Usage

Run with no arguments to launch the picker. A few flags are available:

    theme-picker --paths      Print resolved config, cache and favorites locations
    theme-picker --help       Show help
    theme-picker --version    Show version

## Configuration

Config file: `~/.config/theme-picker/config.toml`

| Key | Description |
| --- | --- |
| wallpaper_dir | Directory scanned for wallpapers (top level only) |
| thumb_cache_dir | Where generated thumbnails and video posters are cached |
| drivers.image | Enable image wallpapers |
| drivers.video | Enable video wallpapers (off by default) |
| hooks.image | Script run when an image wallpaper is applied |
| hooks.video | Script run when a video wallpaper is applied |

Example:

    wallpaper_dir = "/home/user/Pictures/Wallpapers"
    thumb_cache_dir = "/home/user/.cache/theme-picker/thumbs"

    [drivers]
    image = true
    video = false

    [hooks]
    image = "/home/user/.config/theme-picker/set-theme.sh"
    # video = "/home/user/.config/theme-picker/set-theme-video.sh"

## Hooks

A hook is any executable that applies a wallpaper. It is spawned on apply and
not waited on. The program itself knows nothing about your compositor or
wallpaper backend; that logic lives entirely in the hook.

Image hooks receive one argument:

    $1   wallpaper path

Example for Hyprland with hyprpaper and matugen:

    #!/usr/bin/env bash
    set -euo pipefail
    IMG="$1"
    MONITOR="$(hyprctl activeworkspace -j | jq -r '.monitor')"
    hyprctl hyprpaper wallpaper "${MONITOR},${IMG},cover"
    matugen image "$IMG"

Video hooks receive two arguments:

    $1   video path
    $2   extracted poster (a JPEG still frame)

The poster path is also printed to the terminal. Use `$2` as the palette
source, since video files are not valid input for image tools. The player
(mpvpaper, swww, and so on) and stopping a previous instance are the script's
responsibility:

    #!/usr/bin/env bash
    set -euo pipefail
    VIDEO="$1"
    POSTER="$2"
    # stop any previous player, then start your chosen one with "$VIDEO"
    matugen image "$POSTER"

An image hook should also stop any running video player, otherwise it keeps
playing under the new static wallpaper.

## Controls

| Key | Action |
| --- | --- |
| Arrow keys | Move selection, cross page boundaries at the edges |
| Enter | Apply selected wallpaper |
| F | Toggle favorite |
| Tab | Switch between All and Favorites |
| Mouse wheel | Change page |
| Escape | Close window |

## License

GPL-3.0-only.
