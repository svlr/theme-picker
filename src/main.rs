mod config;
mod filesystem;
mod hooks;
mod state;
mod ui;
mod utils;

use gtk4::prelude::*;
use gtk4::Application;
use ui::build_ui;

fn main() {
    let _vips_app = libvips::VipsApp::new("theme-picker", false)
        .expect("failed to initialize libvips runtime");
    let app = Application::builder()
        .application_id("dev.svlr.theme-picker")
        .build();
    
    app.connect_activate(build_ui);
    
    let exit_code = app.run();
    std::process::exit(exit_code.value());
}
