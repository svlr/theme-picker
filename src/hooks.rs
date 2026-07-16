use std::path::Path;
use std::process::Command;
use crate::config::Config;
use crate::filesystem::VIDEO_EXTS;

pub fn apply_theme(wallpaper: &Path, config: &Config) {
    run_theme_hook(
        wallpaper,
        config.drivers.image,
        config.drivers.video,
        &config.hooks.image,
        config.hooks.video.as_deref(),
    );
}

pub fn run_theme_hook(
    wallpaper: &Path,
    drivers_image: bool,
    drivers_video: bool,
    hook_image: &Path,
    hook_video: Option<&Path>,
) {
    let ext = wallpaper
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    if VIDEO_EXTS.contains(&ext.as_str()) {
        if !drivers_video {
            eprintln!(
                "Warning: Video wallpaper selected, but drivers.video is set to false in config: {:?}",
                wallpaper
            );
            return;
        }
        match hook_video {
            Some(hook) => spawn_hook(hook, wallpaper),
            None => {
                eprintln!("Warning: drivers.video=true but hooks.video path is not configured");
            }
        }
        return;
    }

    if !drivers_image {
        eprintln!(
            "Warning: Image wallpaper selected, but drivers.image is set to false in config: {:?}",
            wallpaper
        );
        return;
    }
    spawn_hook(hook_image, wallpaper);
}

fn spawn_hook(hook: &Path, wallpaper: &Path) {
    if let Err(e) = Command::new(hook).arg(wallpaper).spawn() {
        eprintln!("Error: Failed to run hook {:?} for {:?}: {}", hook, wallpaper, e);
    }
}
