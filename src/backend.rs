use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::time::UNIX_EPOCH;

use gtk4::glib;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

// Константы

pub const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp"];
pub const VIDEO_EXTS: &[&str] = &["mp4", "webm", "mkv"];

pub const THUMB_W: i32 = 200;
pub const THUMB_H: i32 = 120;

// Конфигурация

#[derive(Deserialize)]
pub struct Config {
    pub wallpaper_dir: PathBuf,
    pub thumb_cache_dir: PathBuf,
    pub drivers: Drivers,
    pub hooks: Hooks,
}

#[derive(Deserialize)]
pub struct Drivers {
    pub image: bool,
    #[serde(default)]
    pub video: bool,
}

#[derive(Deserialize)]
pub struct Hooks {
    pub image: PathBuf,
    #[serde(default)]
    pub video: Option<PathBuf>,
}

pub fn config_path() -> PathBuf {
    glib::user_config_dir()
        .join("theme-picker")
        .join("config.toml")
}

pub fn try_load_config() -> Result<Config, String> {
    let path = config_path();
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read config file at {:?}: {}", path, e))?;
    toml::from_str(&text).map_err(|e| format!("invalid TOML in {:?}: {}", path, e))
}

pub fn load_config() -> Config {
    match try_load_config() {
        Ok(cfg) => cfg,
        Err(e) => {
            crate::elog!("Error: {}", e);
            crate::elog!(
                "Please make sure config.toml exists and is valid in your config directory."
            );
            std::process::exit(1);
        }
    }
}

// Сканирование

pub fn scan_dir(dir: &Path, include_video: bool) -> Vec<PathBuf> {
    if !dir.exists() {
        crate::elog!("Error: Wallpaper directory does not exist: {:?}", dir);
        return Vec::new();
    }
    if !dir.is_dir() {
        crate::elog!("Error: Wallpaper path is not a directory: {:?}", dir);
        return Vec::new();
    }

    let mut entries: Vec<PathBuf> = WalkDir::new(dir)
        .max_depth(1)
        .into_iter()
        .filter_map(|entry| match entry {
            Ok(e) => Some(e),
            Err(e) => {
                crate::elog!("Warning: Failed to read folder entry in {:?}: {}", dir, e);
                None
            }
        })
        .filter(|e| e.path().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|s| s.to_str())
                .map(|ext| {
                    let ext = ext.to_lowercase();
                    IMAGE_EXTS.contains(&ext.as_str())
                        || (include_video && VIDEO_EXTS.contains(&ext.as_str()))
                })
                .unwrap_or(false)
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    entries.sort();
    entries
}

// Избранное

#[derive(Serialize, Deserialize, Default)]
struct FavoritesFile {
    #[serde(default)]
    wallpapers: Vec<String>,
}

pub fn favorites_file_path() -> PathBuf {
    glib::user_data_dir()
        .join("theme-picker")
        .join("favorites.toml")
}

pub fn load_favorites(wallpaper_dir: &Path, legacy_cache_dir: &Path) -> Vec<PathBuf> {
    let toml_path = favorites_file_path();

    let raw: Vec<String> = if toml_path.is_file() {
        match std::fs::read_to_string(&toml_path) {
            Ok(text) => match toml::from_str::<FavoritesFile>(&text) {
                Ok(f) => f.wallpapers,
                Err(e) => {
                    crate::elog!(
                        "Error: Invalid TOML in favorites file {:?}: {}",
                        toml_path,
                        e
                    );
                    crate::elog!("Favorites will appear empty; the file is left untouched.");
                    return Vec::new();
                }
            },
            Err(e) => {
                crate::elog!(
                    "Error: Failed to read favorites file {:?}: {}",
                    toml_path,
                    e
                );
                return Vec::new();
            }
        }
    } else {
        migrate_legacy_favorites(&legacy_cache_dir.join("favorites.txt"), wallpaper_dir)
    };

    let mut list: Vec<PathBuf> = raw
        .iter()
        .map(|entry| {
            let p = PathBuf::from(entry);
            if p.is_absolute() {
                p
            } else {
                wallpaper_dir.join(p)
            }
        })
        .filter(|p| {
            if p.is_file() {
                true
            } else {
                crate::elog!(
                    "Warning: Favorite wallpaper no longer exists, skipping: {:?}",
                    p
                );
                false
            }
        })
        .collect();

    list.sort();
    list.dedup();
    list
}

fn migrate_legacy_favorites(legacy_path: &Path, wallpaper_dir: &Path) -> Vec<String> {
    let text = match std::fs::read_to_string(legacy_path) {
        Ok(t) => t,
        Err(ref e) if e.kind() == ErrorKind::NotFound => return Vec::new(),
        Err(e) => {
            crate::elog!(
                "Warning: Failed to read legacy favorites {:?}: {}",
                legacy_path,
                e
            );
            return Vec::new();
        }
    };

    let paths: Vec<PathBuf> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(PathBuf::from)
        .collect();

    if paths.is_empty() {
        return Vec::new();
    }

    crate::elog!(
        "Info: Migrating {} favorite(s) from {:?} to {:?}",
        paths.len(),
        legacy_path,
        favorites_file_path()
    );

    save_favorites(wallpaper_dir, &paths);

    let backup = legacy_path.with_extension("txt.bak");
    if let Err(e) = std::fs::rename(legacy_path, &backup) {
        crate::elog!(
            "Warning: Failed to rename legacy favorites to {:?}: {}",
            backup,
            e
        );
    }

    paths
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

pub fn save_favorites(wallpaper_dir: &Path, list: &[PathBuf]) {
    let path = favorites_file_path();

    if let Some(dir) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(dir) {
            crate::elog!("Error: Failed to create data directory {:?}: {}", dir, e);
            return;
        }
    }

    let file = FavoritesFile {
        wallpapers: list
            .iter()
            .map(|p| match p.strip_prefix(wallpaper_dir) {
                Ok(rel) => rel.to_string_lossy().into_owned(),
                Err(_) => p.to_string_lossy().into_owned(),
            })
            .collect(),
    };

    let text = match toml::to_string_pretty(&file) {
        Ok(t) => t,
        Err(e) => {
            crate::elog!("Error: Failed to serialize favorites: {}", e);
            return;
        }
    };

    if let Err(e) = std::fs::write(&path, text) {
        crate::elog!("Error: Failed to save favorites to {:?}: {}", path, e);
    }
}

pub fn add_favorite(path: &Path, list: &mut Vec<PathBuf>) -> bool {
    if list.iter().any(|p| p == path) {
        return false;
    }
    list.push(path.to_path_buf());
    list.sort();
    true
}

pub fn remove_favorite(path: &Path, list: &mut Vec<PathBuf>) -> bool {
    let before = list.len();
    list.retain(|p| p != path);
    list.len() != before
}

// Кэш миниатюр

fn cache_hash(source: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source.to_string_lossy().as_bytes());

    if let Ok(meta) = std::fs::metadata(source) {
        hasher.update(meta.len().to_le_bytes());
        if let Ok(mtime) = meta.modified() {
            if let Ok(d) = mtime.duration_since(UNIX_EPOCH) {
                hasher.update(d.as_secs().to_le_bytes());
                hasher.update(d.subsec_nanos().to_le_bytes());
            }
        }
    }

    format!("{:x}", hasher.finalize())
}

pub fn thumbnail_cache_path(source: &Path, cache_dir: &Path) -> PathBuf {
    cache_dir.join(format!("{}.jpg", cache_hash(source)))
}

pub fn poster_cache_path(source: &Path, cache_dir: &Path) -> PathBuf {
    cache_dir.join(format!("{}.poster.jpg", cache_hash(source)))
}

pub fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|ext| VIDEO_EXTS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

// Стоп-кадры видео

fn decode_video_frame(source: &Path) -> Result<(Vec<u8>, i32, i32), String> {
    use ffmpeg::format::Pixel;
    use ffmpeg::media::Type;
    use ffmpeg::software::scaling::{context::Context, flag::Flags};
    use ffmpeg::util::frame::video::Video;
    use ffmpeg_next as ffmpeg;

    let mut ictx = ffmpeg::format::input(&source).map_err(|e| format!("cannot open video: {e}"))?;

    let input = ictx
        .streams()
        .best(Type::Video)
        .ok_or_else(|| "no video stream in file".to_string())?;
    let stream_index = input.index();

    let decoder_ctx = ffmpeg::codec::context::Context::from_parameters(input.parameters())
        .map_err(|e| format!("codec setup failed: {e}"))?;
    let mut decoder = decoder_ctx
        .decoder()
        .video()
        .map_err(|e| format!("not a video decoder: {e}"))?;

    let target: i64 = 1_000_000;
    let _ = ictx.seek(target, ..target);

    let mut scaler = Context::get(
        decoder.format(),
        decoder.width(),
        decoder.height(),
        Pixel::RGB24,
        decoder.width(),
        decoder.height(),
        Flags::BILINEAR,
    )
    .map_err(|e| format!("scaler init failed: {e}"))?;

    let mut take_frame = |decoder: &mut ffmpeg::decoder::Video| -> Option<(Vec<u8>, i32, i32)> {
        let mut decoded = Video::empty();
        if decoder.receive_frame(&mut decoded).is_err() {
            return None;
        }
        let mut rgb = Video::empty();
        if scaler.run(&decoded, &mut rgb).is_err() {
            return None;
        }
        let w = rgb.width() as usize;
        let h = rgb.height() as usize;
        let stride = rgb.stride(0);
        let data = rgb.data(0);
        let row = w * 3;
        let mut packed = Vec::with_capacity(row * h);
        for y in 0..h {
            let start = y * stride;
            packed.extend_from_slice(&data[start..start + row]);
        }
        Some((packed, w as i32, h as i32))
    };

    for (stream, packet) in ictx.packets() {
        if stream.index() == stream_index && decoder.send_packet(&packet).is_ok() {
            if let Some(frame) = take_frame(&mut decoder) {
                return Ok(frame);
            }
        }
    }

    let _ = decoder.send_eof();
    if let Some(frame) = take_frame(&mut decoder) {
        return Ok(frame);
    }

    Err("no decodable frame".to_string())
}

fn generate_poster(source: &Path, poster: &Path) -> Result<(), String> {
    use libvips::ops;

    let (pixels, w, h) = decode_video_frame(source)?;

    let img = libvips::VipsImage::new_from_memory_copy(&pixels, w, h, 3, ops::BandFormat::Uchar)
        .map_err(|e| format!("vips image from frame failed: {e}"))?;

    let save_opts = ops::JpegsaveOptions {
        q: 90,
        ..Default::default()
    };

    let tmp = poster.with_extension(format!("{}.tmp", std::process::id()));
    ops::jpegsave_with_opts(&img, &tmp.to_string_lossy(), &save_opts)
        .map_err(|e| format!("vips jpegsave failed: {e}"))?;
    if let Err(e) = std::fs::rename(&tmp, poster) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("finalize poster failed: {e}"));
    }
    Ok(())
}

pub fn ensure_poster(source: &Path, cache_dir: &Path) -> Result<PathBuf, String> {
    let poster = poster_cache_path(source, cache_dir);
    if !poster.exists() {
        generate_poster(source, &poster)?;
    }
    Ok(poster)
}

// Хуки

pub fn apply_theme(wallpaper: &Path, config: &Config) {
    let ext = wallpaper
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    if VIDEO_EXTS.contains(&ext.as_str()) {
        if !config.drivers.video {
            crate::elog!(
                "Warning: Video wallpaper selected, but drivers.video is set to false in config: {:?}",
                wallpaper
            );
            return;
        }
        let poster = ensure_poster(wallpaper, &config.thumb_cache_dir);
        match &poster {
            Ok(p) => {
                let _ = writeln!(
                    io::stdout(),
                    "theme-picker: video poster for {:?} -> {:?}",
                    wallpaper,
                    p
                );
            }
            Err(e) => {
                let _ = writeln!(
                    io::stderr(),
                    "Warning: could not generate poster for {:?}: {} (hook gets video path only)",
                    wallpaper,
                    e
                );
            }
        }

        match config.hooks.video.as_deref() {
            Some(hook) => spawn_hook(hook, wallpaper, poster.as_deref().ok()),
            Option::None => {
                crate::elog!("Warning: drivers.video=true but hooks.video path is not configured");
            }
        }
        return;
    }

    if !config.drivers.image {
        crate::elog!(
            "Warning: Image wallpaper selected, but drivers.image is set to false in config: {:?}",
            wallpaper
        );
        return;
    }
    spawn_hook(&config.hooks.image, wallpaper, None);
}

fn spawn_hook(hook: &Path, wallpaper: &Path, extra: Option<&Path>) {
    let wallpaper = wallpaper
        .canonicalize()
        .unwrap_or_else(|_| wallpaper.to_path_buf());
    let mut cmd = Command::new(hook);
    cmd.arg(&wallpaper);
    if let Some(extra) = extra {
        cmd.arg(extra);
    }
    if let Err(e) = cmd.spawn() {
        crate::elog!(
            "Error: Failed to run hook {:?} for {:?}: {}",
            hook,
            wallpaper,
            e
        );
    }
}

// Воркер миниатюр

pub type ThumbResult = (PathBuf, PathBuf);

pub fn spawn_thumbnail_worker(
    cache_dir: PathBuf,
) -> (mpsc::Sender<PathBuf>, async_channel::Receiver<ThumbResult>) {
    let (job_tx, job_rx) = mpsc::channel::<PathBuf>();
    let (result_tx, result_rx) = async_channel::unbounded::<ThumbResult>();

    std::thread::spawn(move || {
        use libvips::ops;

        for source in job_rx {
            let thumb = thumbnail_cache_path(&source, &cache_dir);
            if !thumb.exists() {
                let thumb_src: Option<PathBuf> = if is_video(&source) {
                    match ensure_poster(&source, &cache_dir) {
                        Ok(poster) => Some(poster),
                        Err(e) => {
                            crate::elog!("Error: failed to extract poster for {:?}: {}", source, e);
                            None
                        }
                    }
                } else {
                    Some(source.clone())
                };

                if let Some(thumb_src) = thumb_src {
                    let source_str = thumb_src.to_string_lossy();

                    let options = ops::ThumbnailOptions {
                        height: THUMB_H,
                        crop: ops::Interesting::Attention,
                        ..Default::default()
                    };

                    match ops::thumbnail_with_opts(&source_str, THUMB_W, &options) {
                        Ok(resized) => {
                            let save_opts = ops::JpegsaveOptions {
                                q: 85,
                                ..Default::default()
                            };

                            let tmp = thumb.with_extension(format!("{}.tmp", std::process::id()));
                            let tmp_str = tmp.to_string_lossy();

                            match ops::jpegsave_with_opts(&resized, &tmp_str, &save_opts) {
                                Ok(()) => {
                                    if let Err(e) = std::fs::rename(&tmp, &thumb) {
                                        crate::elog!(
                                            "Error: failed to finalize thumbnail {:?}: {}",
                                            thumb,
                                            e
                                        );
                                        let _ = std::fs::remove_file(&tmp);
                                    }
                                }
                                Err(e) => {
                                    crate::elog!(
                                        "Error: libvips failed to save thumbnail for {:?}: {}",
                                        source,
                                        e
                                    );
                                    let _ = std::fs::remove_file(&tmp);
                                }
                            }
                        }
                        Err(e) => {
                            crate::elog!(
                                "Error: libvips failed to generate thumbnail for {:?}: {}",
                                source,
                                e
                            );
                        }
                    }
                }
            }
            if let Err(e) = result_tx.send_blocking((source.clone(), thumb)) {
                crate::elog!(
                    "Error: Failed to send thumbnail result for {:?}: {}",
                    source,
                    e
                );
            }
        }
    });

    (job_tx, result_rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires ffmpeg CLI; generates a test video and writes files"]
    fn poster_from_video_is_valid_jpeg() {
        let _vips = libvips::VipsApp::new("tp-test", false).expect("vips init");
        ffmpeg_next::init().expect("ffmpeg init");

        let dir = std::env::temp_dir().join(format!("tp-poster-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let video = dir.join("clip.mp4");

        let status = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=2:size=320x240:rate=10",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&video)
            .status()
            .expect("run ffmpeg");
        assert!(status.success(), "ffmpeg failed to make test clip");

        assert!(is_video(&video));

        let poster = ensure_poster(&video, &dir).expect("poster generated");
        assert!(poster.exists(), "poster file missing");
        assert_eq!(poster, poster_cache_path(&video, &dir));

        let img = libvips::VipsImage::new_from_file(poster.to_string_lossy().as_ref())
            .expect("poster is a loadable image");
        assert!(img.get_width() > 0 && img.get_height() > 0);

        let again = ensure_poster(&video, &dir).expect("poster cached");
        assert_eq!(again, poster);

        std::fs::remove_dir_all(&dir).ok();
    }
}
