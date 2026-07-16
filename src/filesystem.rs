use std::path::{Path, PathBuf};
use std::io::ErrorKind;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

pub const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp"];
pub const VIDEO_EXTS: &[&str] = &["mp4", "webm", "mkv"];

pub fn scan_dir(dir: &Path) -> Vec<PathBuf> {
    if !dir.exists() {
        eprintln!("Error: Wallpaper directory does not exist: {:?}", dir);
        return Vec::new();
    }
    if !dir.is_dir() {
        eprintln!("Error: Wallpaper path is not a directory: {:?}", dir);
        return Vec::new();
    }

    let mut entries: Vec<PathBuf> = WalkDir::new(dir)
        .max_depth(1)
        .into_iter()
        .filter_map(|entry| match entry {
            Ok(e) => Some(e),
            Err(e) => {
                eprintln!("Warning: Failed to read folder entry in {:?}: {}", dir, e);
                None
            }
        })
        .filter(|e| e.path().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|s| s.to_str())
                .map(|ext| IMAGE_EXTS.contains(&ext.to_lowercase().as_str()))
                .unwrap_or(false)
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    entries.sort();
    entries
}

pub fn load_favorites(path: &Path) -> Vec<PathBuf> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(ref e) if e.kind() == ErrorKind::NotFound => {
            return Vec::new();
        }
        Err(e) => {
            eprintln!("Error: Failed to read favorites file at {:?}: {}", path, e);
            return Vec::new();
        }
    };

    let mut list: Vec<PathBuf> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(PathBuf::from)
        .filter(|p| {
            if p.is_file() {
                true
            } else {
                eprintln!("Warning: Favorite wallpaper no longer exists, skipping: {:?}", p);
                false
            }
        })
        .collect();

    list.sort();
    list
}

pub fn save_favorites(path: &Path, list: &[PathBuf]) {
    let text: String = list
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("\n");

    if let Err(e) = std::fs::write(path, text) {
        eprintln!("Error: Failed to save favorites to {:?}: {}", path, e);
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

pub fn thumbnail_cache_path(source: &Path, cache_dir: &Path) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(source.to_string_lossy().as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    cache_dir.join(format!("{}.jpg", hash))
}
