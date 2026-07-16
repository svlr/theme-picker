use serde::Deserialize;
use std::path::PathBuf;
use gtk4::glib;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub wallpaper_dir: PathBuf,
    pub thumb_cache_dir: PathBuf,
    pub drivers: Drivers,
    pub hooks: Hooks,
}

#[derive(Deserialize, Clone)]
pub struct Drivers {
    pub image: bool,
    #[serde(default)]
    pub video: bool,
}

#[derive(Deserialize, Clone)]
pub struct Hooks {
    pub image: PathBuf,
    #[serde(default)]
    pub video: Option<PathBuf>,
}

pub fn load_config() -> Config {
    let path = glib::user_config_dir()
        .join("theme-picker")
        .join("config.toml");

    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Error: Cannot read config file at {:?}: {}", path, e);
            eprintln!("Please make sure config.toml exists in your configuration directory.");
            std::process::exit(1);
        }
    };

    match toml::from_str(&text) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Error: Invalid TOML format in {:?}: {}", path, e);
            eprintln!("Please check your config.toml for syntax errors.");
            std::process::exit(1);
        }
    }
}
