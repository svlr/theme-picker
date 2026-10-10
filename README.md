# theme-picker

A fast, native GTK4 wallpaper and theme picker. Pick a wallpaper from a
thumbnail grid; everything that happens next is up to you.

<img src="https://github.com/user-attachments/assets/9e36a84d-26bb-4657-b6a3-bcdf6e440a61" width="820" alt="theme-picker" />

## Why another one

Most wallpaper pickers decide for you how the wallpaper gets set — they
hardcode a backend, a compositor, a colour-scheme generator, and you live
with those choices or you look for a different tool.

theme-picker doesn't. It knows how to show you images fast and how to
remember which ones you like. Applying them is a one-line shell script that
you write. That means:

- **Any compositor** — Hyprland, Sway, river, labwc, or an X11 WM
- **Any wallpaper backend** — hyprpaper, swww, swaybg, feh, mpvpaper
- **Any theming pipeline** — matugen, pywal, wallust, or nothing at all
- **Anything else you can script**

That last one is not a figure of speech. The hook receives a path and runs
as you; what it does with the resulting palette is entirely your business.
Push the dominant colour to a smart bulb over the Hue or Tasmota API. Drive
an LED strip behind the monitor so the room matches the desktop. Repaint
your terminal, your editor, your bar, your lock screen. Commit the change to
your dotfiles. Set your chat avatar to the wallpaper.

The program never needs an update because your setup changed. The hook is
the integration layer, and it's yours.

## Features

- Paginated thumbnail grid that reflows live on window resize and fullscreen
- Keyboard and mouse navigation, mouse-wheel paging
- Favorites, stored as a separate view
- Image wallpapers: png, jpg, jpeg, webp
- Video wallpapers: mp4, webm, mkv (optional, off by default)
- Thumbnails generated asynchronously in-process via libvips (FFI)
- Video posters extracted in-process via libav — no shelling out to ffmpeg
- Theme application delegated entirely to a user hook script

<img src="https://github.com/user-attachments/assets/4d1a479b-6b37-4c1a-901e-b6e46ca653b9" width="820" alt="grid reflowing on window resize" />

## Dependencies

Build and runtime require libvips, GTK4 and ffmpeg development libraries
plus pkg-config. They are linked into the binary like any FFI dependency;
there is no external command to configure.

**Arch Linux**

```
sudo pacman -S libvips gtk4 ffmpeg pkgconf
```

**Debian / Ubuntu**

```
sudo apt install libvips-dev libgtk-4-dev libavcodec-dev libavformat-dev \
    libavutil-dev libswscale-dev pkg-config
```

**Fedora**

```
sudo dnf install vips-devel gtk4-devel ffmpeg-devel pkgconf-pkg-config
```

## Install

```
cargo install --git https://github.com/svlr/theme-picker
```

Or build from a clone:

```
git clone https://github.com/svlr/theme-picker
cd theme-picker
cargo build --release
```

The binary is at `target/release/theme-picker`.

## Usage

Run with no arguments to launch the picker.

```
theme-picker --paths      Print resolved config, cache and favorites locations
theme-picker --help       Show help
theme-picker --version    Show version
```

## Configuration

Config file: `~/.config/theme-picker/config.toml`

| Key             | Description                                             |
| --------------- | ------------------------------------------------------- |
| wallpaper_dir   | Directory scanned for wallpapers (top level only)       |
| thumb_cache_dir | Where generated thumbnails and video posters are cached |
| drivers.image   | Enable image wallpapers                                 |
| drivers.video   | Enable video wallpapers (off by default)                |
| hooks.image     | Script run when an image wallpaper is applied           |
| hooks.video     | Script run when a video wallpaper is applied            |

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

## Hooks

A hook is any executable that applies a wallpaper. It is spawned on apply
and not waited on. The program itself knows nothing about your compositor
or wallpaper backend; that logic lives entirely in the hook.

<img src="https://github.com/user-attachments/assets/de1052b5-e223-4543-b470-71fd5becff63" width="720" alt="palette applied across the desktop" />

### Image hooks

One argument:

```
$1   wallpaper path
```

Hyprland + hyprpaper + matugen:

```bash
#!/usr/bin/env bash
set -euo pipefail
IMG="$1"
MONITOR="$(hyprctl activeworkspace -j | jq -r '.monitor')"
hyprctl hyprpaper wallpaper "${MONITOR},${IMG},cover"
matugen image "$IMG"
```

Sway + swaybg + pywal:

```bash
#!/usr/bin/env bash
set -euo pipefail
IMG="$1"
pkill swaybg || true
swaybg -i "$IMG" -m fill &
wal -i "$IMG" -n
```

swww, any compositor:

```bash
#!/usr/bin/env bash
set -euo pipefail
swww img "$1" --transition-type grow --transition-fps 60
```

Going further — push the generated palette to a smart bulb:

```bash
#!/usr/bin/env bash
set -euo pipefail
IMG="$1"
swww img "$IMG"
matugen image "$IMG"

# matugen can write any format you template; here it has produced
# ~/.cache/matugen/colors.json with a "primary" hex colour
HEX="$(jq -r '.colors.dark.primary' ~/.cache/matugen/colors.json | tr -d '#')"
curl -s -X POST "http://$BULB_HOST/cm?cmnd=Color%20$HEX" >/dev/null || true
```

### Video hooks

Two arguments:

```
$1   video path
$2   extracted poster (a JPEG still frame)
```

Use `$2` as the palette source — video files are not valid input for image
tools. The poster path is also printed to the terminal. The player
(mpvpaper, swww, and so on) and stopping a previous instance are the
script's responsibility:

```bash
#!/usr/bin/env bash
set -euo pipefail
VIDEO="$1"
POSTER="$2"
pkill mpvpaper || true
mpvpaper -o "no-audio --loop-playlist" '*' "$VIDEO" &
matugen image "$POSTER"
```

An image hook should also stop any running video player, otherwise it keeps
playing under the new static wallpaper.

## Controls

| Key         | Action                                             |
| ----------- | -------------------------------------------------- |
| Arrow keys  | Move selection, cross page boundaries at the edges |
| Enter       | Apply selected wallpaper                           |
| F           | Toggle favorite                                    |
| Tab         | Switch between All and Favorites                   |
| Mouse wheel | Change page                                        |
| Escape      | Close window                                       |

## License

GPL-3.0-only.