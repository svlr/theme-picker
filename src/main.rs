mod backend;
mod ui;

use gtk4::Application;
use gtk4::prelude::*;
use ui::build_ui;

fn print_help() {
    println!("theme-picker {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("A fast GTK4 wallpaper and theme picker.");
    println!();
    println!("Usage:");
    println!("  theme-picker            Launch the picker");
    println!("  theme-picker --paths    Print resolved file locations and exit");
    println!("  theme-picker --help     Show this help and exit");
    println!("  theme-picker --version  Show version and exit");
}

fn print_paths() {
    println!("{:<11} {}", "config:", backend::config_path().display());
    match backend::try_load_config() {
        Ok(cfg) => println!("{:<11} {}", "cache:", cfg.thumb_cache_dir.display()),
        Err(e) => println!("{:<11} (unknown — {})", "cache:", e),
    }
    println!(
        "{:<11} {}",
        "favorites:",
        backend::favorites_file_path().display()
    );
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("-h") | Some("--help") => {
            print_help();
            return;
        }
        Some("-V") | Some("--version") => {
            println!("theme-picker {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        Some("--paths") => {
            print_paths();
            return;
        }
        _ => {}
    }

    let _vips_app =
        libvips::VipsApp::new("theme-picker", false).expect("failed to initialize libvips runtime");

    if let Err(e) = ffmpeg_next::init() {
        eprintln!(
            "Warning: failed to initialize ffmpeg runtime: {e}. \
             Video wallpaper previews will be unavailable."
        );
    }

    let app = Application::builder()
        .application_id("dev.svlr.theme-picker")
        .build();

    app.connect_activate(build_ui);

    let exit_code = app.run();
    std::process::exit(exit_code.value());
}
