pub mod css;
pub mod grid;
pub mod indicators;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Button, Label, Orientation, ScrolledWindow,
};
use gtk4::glib;

use crate::config::load_config;
use crate::filesystem::{
    load_favorites, scan_dir, thumbnail_cache_path, save_favorites, add_favorite, remove_favorite
};
use crate::hooks::apply_theme;
use crate::state::{AppState, View, ViewPosition};
use crate::utils::*;

use self::grid::{rebuild_thumbnail_grid, clamp_and_highlight};
use self::indicators::{update_page_labels, rebuild_dot_indicator, update_view_indicator};
use self::css::load_css;

pub fn render_page(state: &Rc<RefCell<AppState>>) {
    rebuild_thumbnail_grid(state);
    update_page_labels(state);
    rebuild_dot_indicator(state);
    clamp_and_highlight(state);
}

pub fn switch_view(state: &Rc<RefCell<AppState>>, target: View) {
    let mut s = state.borrow_mut();
    if s.view == target {
        return;
    }

    let current_pos = ViewPosition {
        page: s.page,
        row: s.selected_row,
        col: s.selected_col,
    };
    match s.view {
        View::All => s.all_position = current_pos,
        View::Favorites => s.favorites_position = current_pos,
    }

    s.view = target;
    s.wallpapers = match target {
        View::All => s.all_wallpapers.clone(),
        View::Favorites => s.favorite_wallpapers.clone(),
    };

    let saved = match target {
        View::All => s.all_position,
        View::Favorites => s.favorites_position,
    };
    s.page = saved.page;
    s.selected_row = saved.row;
    s.selected_col = saved.col;

    let total = s.total_pages();
    if s.page >= total {
        s.page = total - 1;
    }

    drop(s);
    render_page(state);
    update_view_indicator(state);
}

// В src/ui/mod.rs
pub fn flash_favorite(btn: &Button, heart: &Label, added: bool) {
    btn.add_css_class("favorite-flash");

    heart.set_text(if added { "♥" } else { "♡" });
    heart.remove_css_class("favorite-heart-add");
    heart.remove_css_class("favorite-heart-remove");
    heart.add_css_class(if added {
        "favorite-heart-add"
    } else {
        "favorite-heart-remove"
    });

    let btn = btn.clone();
    let heart = heart.clone();
    glib::timeout_add_local_once(Duration::from_millis(FAVORITE_FLASH_MS), move || {
        btn.remove_css_class("favorite-flash");
        heart.remove_css_class("favorite-heart-add");
        heart.remove_css_class("favorite-heart-remove");
    });
}

pub fn build_ui(app: &Application) {
    if gtk4::gdk::Display::default().is_none() {
        eprintln!("Error: GDK cannot open display. Are you running in a non-GUI environment?");
        std::process::exit(1);
    }

    let config = load_config();
    
    if let Err(e) = std::fs::create_dir_all(&config.thumb_cache_dir) {
        eprintln!("Warning: Failed to create thumbnail cache directory at {:?}: {}", config.thumb_cache_dir, e);
        eprintln!("Previews will not be cached between sessions.");
    }

    let favorites_path = config.thumb_cache_dir.join("favorites.txt");

    let (job_tx, job_rx) = mpsc::channel::<std::path::PathBuf>();
    let (result_tx, result_rx) = async_channel::unbounded::<(std::path::PathBuf, std::path::PathBuf)>();

    {
        let cache_dir = config.thumb_cache_dir.clone();
        std::thread::spawn(move || {
            use libvips::ops;

            for source in job_rx {
                let thumb = thumbnail_cache_path(&source, &cache_dir);
                if !thumb.exists() {
                    let source_str = source.to_string_lossy();
                    let thumb_str = thumb.to_string_lossy();

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
                            
                            if let Err(e) = ops::jpegsave_with_opts(&resized, &thumb_str, &save_opts) {
                                eprintln!("Error: libvips failed to save thumbnail for {:?}: {}", source, e);
                            }
                        }
                        Err(e) => {
                            eprintln!("Error: libvips failed to generate thumbnail for {:?}: {}", source, e);
                        }
                    }
                }
                if let Err(e) = result_tx.send_blocking((source.clone(), thumb)) {
                    eprintln!("Error: Failed to send thumbnail result for {:?}: {}", source, e);
                }
            }
        });
    }


    let window = ApplicationWindow::builder()
        .application(app)
        .title("Theme Picker")
        .default_width(700)
        .default_height(600)
        .resizable(true)
        .build();

    let flow = gtk4::FlowBox::builder()
        .valign(gtk4::Align::Center)
        .halign(gtk4::Align::Center)
        .selection_mode(gtk4::SelectionMode::None)
        .row_spacing(SPACING as u32)
        .column_spacing(SPACING as u32)
        .margin_top(MARGIN)
        .margin_bottom(MARGIN)
        .margin_start(MARGIN)
        .margin_end(MARGIN)
        .build();
    flow.set_can_focus(false);

    let all_wallpapers = scan_dir(&config.wallpaper_dir);
    let favorite_wallpapers = load_favorites(&favorites_path);
    let wallpapers = all_wallpapers.clone();

    let current_label = Label::new(Some("1"));
    current_label.add_css_class("page-number");
    let total_label = Label::new(Some("1"));
    total_label.add_css_class("page-number");

    let indicator_box = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(6)
        .halign(gtk4::Align::Center)
        .valign(gtk4::Align::Center)
        .build();

    let indicator_wrapper = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(8)
        .halign(gtk4::Align::Center)
        .valign(gtk4::Align::Center)
        .width_request(INDICATOR_WIDTH)
        .build();
    indicator_wrapper.append(&current_label);
    indicator_wrapper.append(&indicator_box);
    indicator_wrapper.append(&total_label);

    let view_all_label = Label::new(Some("All"));
    view_all_label.add_css_class("view-tab");
    view_all_label.set_cursor_from_name(Some("pointer"));
    let view_favorites_label = Label::new(Some("Favorites"));
    view_favorites_label.add_css_class("view-tab");
    view_favorites_label.set_cursor_from_name(Some("pointer"));

    let view_indicator = GtkBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(16)
        .halign(gtk4::Align::Center)
        .valign(gtk4::Align::Start)
        .margin_top(MARGIN)
        .build();
    view_indicator.append(&view_all_label);
    view_indicator.append(&view_favorites_label);

    let (init_cols, init_rows) = compute_grid(700, 600);
    flow.set_min_children_per_line(1);
    flow.set_max_children_per_line(init_cols as u32);
    
    let state = Rc::new(RefCell::new(AppState {
        favorites_path: favorites_path.clone(),
        config,
        job_tx,
        all_wallpapers,
        favorite_wallpapers,
        wallpapers,
        view: View::All,
        all_position: ViewPosition::default(),
        favorites_position: ViewPosition::default(),
        page: 0,
        selected_row: 0,
        selected_col: 0,
        cols: init_cols,
        rows: init_rows,
        buttons: Vec::new(),
        heart_labels: Vec::new(),
        thumb_map: HashMap::new(),
        pending_jobs: HashSet::new(),
        dot_window_start: 0,
        flow: flow.clone(),
        indicator_box: indicator_box.clone(),
        current_label: current_label.clone(),
        total_label: total_label.clone(),
        view_all_label: view_all_label.clone(),
        view_favorites_label: view_favorites_label.clone(),
    }));

    render_page(&state);
    update_view_indicator(&state);

    let scroll = ScrolledWindow::builder()
        .child(&flow)
        .vexpand(true)
        .hexpand(true)
        .hscrollbar_policy(gtk4::PolicyType::External)
        .vscrollbar_policy(gtk4::PolicyType::External)
        .build();

    let content_row = GtkBox::new(Orientation::Horizontal, 0);
    content_row.append(&scroll);
    content_row.append(&indicator_wrapper);

    let root = GtkBox::new(Orientation::Vertical, 0);
    root.append(&view_indicator);
    root.append(&content_row);
    window.set_child(Some(&root));

    load_css();

    {
        let state = state.clone();
        let click_all = gtk4::GestureClick::new();
        click_all.connect_released(move |_, _, _, _| {
            switch_view(&state, View::All);
        });
        view_all_label.add_controller(click_all);
    }

    {
        let state = state.clone();
        let click_favorites = gtk4::GestureClick::new();
        click_favorites.connect_released(move |_, _, _, _| {
            switch_view(&state, View::Favorites);
        });
        view_favorites_label.add_controller(click_favorites);
    }

    let key_controller = gtk4::EventControllerKey::new();
    {
        let state = state.clone();
        let window = window.clone();
        key_controller.connect_key_pressed(move |_, key, _, _| {
            use gtk4::gdk::Key;
            let mut s = state.borrow_mut();
            match key {
                Key::Escape => {
                    drop(s);
                    window.close();
                    return glib::Propagation::Stop;
                }
                Key::Return | Key::KP_Enter => {
                    let idx = s.page * s.page_size() + s.selected_row * s.cols + s.selected_col;
                    if idx < s.wallpapers.len() {
                        let path = s.wallpapers[idx].clone();
                        apply_theme(&path, &s.config);
                    }
                    return glib::Propagation::Stop;
                }
                Key::Tab => {
                    let target = match s.view {
                        View::All => View::Favorites,
                        View::Favorites => View::All,
                    };
                    drop(s);
                    switch_view(&state, target);
                    return glib::Propagation::Stop;
                }
                Key::f | Key::F => {
                    let local_idx = s.selected_row * s.cols + s.selected_col;
                    let global_idx = s.page * s.page_size() + local_idx;
                    
                    if global_idx < s.wallpapers.len() {
                        let path = s.wallpapers[global_idx].clone();
                        let mut added: Option<bool> = None;
                        let mut needs_rebuild = false;

                        match s.view {
                            View::All => {
                                if add_favorite(&path, &mut s.favorite_wallpapers) {
                                    let favorites_path = s.favorites_path.clone();
                                    save_favorites(&favorites_path, &s.favorite_wallpapers);
                                    added = Some(true);
                                }
                            }
                            View::Favorites => {
                                if remove_favorite(&path, &mut s.favorite_wallpapers) {
                                    let favorites_path = s.favorites_path.clone();
                                    save_favorites(&favorites_path, &s.favorite_wallpapers);
                                    s.wallpapers = s.favorite_wallpapers.clone();
                                    added = Some(false);
                                    needs_rebuild = true;
                                }
                            }
                        }

                        if let Some(was_added) = added {
                            let btn = s.buttons.get(local_idx).cloned();
                            let heart = s.heart_labels.get(local_idx).cloned();
                            if let (Some(btn), Some(heart)) = (btn, heart) {
                                flash_favorite(&btn, &heart, was_added);
                            }
                        }

                        if needs_rebuild {
                            let state_c = state.clone();
                            glib::timeout_add_local_once(Duration::from_millis(FAVORITE_FLASH_MS), move || {
                                render_page(&state_c);
                            });
                        }
                    }
                    drop(s);
                    return glib::Propagation::Stop;
                }
                Key::Left => {
                    if s.selected_col > 0 {
                        s.selected_col -= 1;
                    }
                }
                Key::Right => {
                    if s.selected_col < s.cols - 1 {
                        s.selected_col += 1;
                    }
                }
                Key::Up => {
                    if s.selected_row > 0 {
                        s.selected_row -= 1;
                    } else if s.page > 0 {
                        s.page -= 1;
                        s.selected_row = s.rows - 1;
                        drop(s);
                        render_page(&state);
                        return glib::Propagation::Stop;
                    }
                }
                Key::Down => {
                    if s.selected_row < s.rows - 1 {
                        s.selected_row += 1;
                    } else if s.page + 1 < s.total_pages() {
                        s.page += 1;
                        s.selected_row = 0;
                        drop(s);
                        render_page(&state);
                        return glib::Propagation::Stop;
                    }
                }
                _ => return glib::Propagation::Proceed,
            }
            drop(s);
            clamp_and_highlight(&state);
            glib::Propagation::Stop
        });
    }
    window.add_controller(key_controller);

    let scroll_controller =
        gtk4::EventControllerScroll::new(gtk4::EventControllerScrollFlags::VERTICAL);
    {
        let state = state.clone();
        scroll_controller.connect_scroll(move |_, _dx, dy| {
            let mut s = state.borrow_mut();
            if dy > 0.0 && s.page + 1 < s.total_pages() {
                s.page += 1;
            } else if dy < 0.0 && s.page > 0 {
                s.page -= 1;
            } else {
                return glib::Propagation::Proceed;
            }
            s.selected_row = 0;
            s.selected_col = 0;
            drop(s);
            render_page(&state);
            glib::Propagation::Stop
        });
    }
    window.add_controller(scroll_controller);

    let handle_resize = {
        let state = state.clone();
        move |w: i32, h: i32| {
            let mut s = state.borrow_mut();
            let (cols, rows) = compute_grid(w, h);
            if cols == s.cols && rows == s.rows {
                return;
            }

            let old_page_size = s.page_size();
            let global_idx = if old_page_size > 0 {
                s.page * old_page_size + s.selected_row * s.cols + s.selected_col
            } else {
                0
            }
            .min(s.wallpapers.len().saturating_sub(1));

            s.cols = cols;
            s.rows = rows;
            s.flow.set_min_children_per_line(1);
            s.flow.set_max_children_per_line(cols as u32);

            let new_page_size = s.page_size();
            if new_page_size > 0 {
                s.page = global_idx / new_page_size;
                let offset = global_idx % new_page_size;
                s.selected_row = offset / cols;
                s.selected_col = offset % cols;
            }
            drop(s);
            render_page(&state);
        }
    };

    window.present();

    {
        let handle_resize = handle_resize.clone();
        let last_size = Rc::new(RefCell::new((0i32, 0i32)));
        window.add_tick_callback(move |win, _clock| {
            let w = win.width();
            let h = win.height();
            let mut last = last_size.borrow_mut();
            if (w, h) != *last {
                *last = (w, h);
                drop(last);
                handle_resize(w, h);
            }
            glib::ControlFlow::Continue
        });
    }

    {
        let state = state.clone();
        glib::MainContext::default().spawn_local(async move {
            while let Ok((source, thumb)) = result_rx.recv().await {
                let mut s = state.borrow_mut();
                s.pending_jobs.remove(&source);
                if let Some((pic, btn)) = s.thumb_map.get(&source) {
                    pic.set_filename(Some(&thumb));
                    btn.remove_css_class("thumb-loading");
                }
            }
        });
    }
}
