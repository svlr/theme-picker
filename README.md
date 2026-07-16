# **theme-picker**

An ultra-fast, native GTK4 wallpaper picker with a paginated thumbnail grid, keyboard/mouse navigation, and hook scripts for applying themes.  
Starting from **v2.0.0**, the application utilizes native **libvips C-bindings (FFI)** instead of spawning external CLI processes. This brings near-instant, asynchronous thumbnail generation directly in-memory, keeping the binary size under **860 KB** and memory usage minimal.

## **Supported Formats**

To ensure maximum performance and compatibility with wallpaper-setter backends, the scanner is strictly limited to:

* .png, .jpg, .jpeg, .webp

## **Dependencies**

### **Build-time (System Packages)**

To compile the FFI bindings, you need the libvips development headers and pkg-config installed on your system:

* **Arch Linux:**  
  Bash  
  sudo pacman \-S libvips pkgconf gtk4

* **Debian / Ubuntu:**  
  Bash  
  sudo apt install libvips-dev pkg-config libgtk-4-dev

* **Fedora:**  
  Bash  
  sudo dnf install vips-devel pkgconf-pkg-config gtk4-devel

### **Rust Dependencies (Cargo.toml)**

Ini, TOML  
\[dependencies\]  
gtk4 \= { version \= "0.9", features \= \["v4\_10"\] }  
glib \= "0.20"  
walkdir \= "2.5"  
sha2 \= "0.10"  
serde \= { version \= "1.0", features \= \["derive"\] }  
toml \= "0.8"  
async-channel \= "2.3"  
libvips \= "2.3.0"

### **Runtime**

* A hook script that applies the selected wallpaper (example below).

## **Build**

Clone and build:

Bash  
git clone https://github.com/svlr/theme-picker  
cd theme-picker  
cargo build \--release

The optimized, stripped binary will be located at target/release/theme-picker.  
Or install directly via Cargo (make sure you have build dependencies installed):

Bash  
cargo install \--git https://github.com/svlr/theme-picker

## **Configuration**

Config file location: \~/.config/theme-picker/config.toml

| Parameter | Description |
| :---- | :---- |
| wallpaper\_dir | Directory scanned for wallpapers (top level only) |
| thumb\_cache\_dir | Where generated thumbnails are cached |
| drivers.image | Enable/disable image wallpapers |
| drivers.video | Enable/disable video wallpapers (routing exists, unused by default) |
| hooks.image | Script run when an image wallpaper is selected |
| hooks.video | Script run when a video wallpaper is selected (optional) |

Example:

Ini, TOML  
wallpaper\_dir \= "/home/user/Pictures/Wallpapers"  
thumb\_cache\_dir \= "/home/user/.cache/theme-picker/thumbs"

\[drivers\]  
image \= true  
video \= false

\[hooks\]  
image \= "/home/user/.config/theme-picker/set-theme.sh"  
\# video \= "/home/user/.config/theme-picker/set-theme-video.sh"

## **Hook script**

A hook is any executable that receives the wallpaper's path as its first argument and applies it however you like (set the wallpaper, regenerate a color scheme, reload bars/terminals, etc). It's called on click or on Enter, without waiting for it to finish.  
Example for Hyprland \+ hyprpaper \+ matugen:

Bash  
\#\!/usr/bin/env bash  
set \-euo pipefail

IMG="${1:?Usage: set-theme \<path-to-image\>}"

if \[\[ \! \-f "$IMG" \]\]; then  
    echo "set-theme: file not found: $IMG" \>&2  
    exit 1  
fi

MONITOR="$(hyprctl activeworkspace \-j | jq \-r '.monitor')"  
if \[\[ \-z "$MONITOR" || "$MONITOR" \== "null" \]\]; then  
    echo "set-theme: failed to detect active monitor" \>&2  
    exit 1  
fi

hyprctl hyprpaper wallpaper "${MONITOR},${IMG},cover"  
matugen image "$IMG"  
"$HOME/.config/waybar/scripts/launch.sh"

## **Controls**

| Key | Action |
| :---- | :---- |
| ← → ↑ ↓ | Move selection, cross page boundaries at the edges |
| Enter | Apply selected wallpaper |
| Escape | Close window |
| Mouse wheel | Change page |
