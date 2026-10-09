use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use gtk4::gdk::Key;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Button, ContentFit, FlowBox, Label, Orientation,
    Overflow, Overlay, Picture, ScrolledWindow,
};

use crate::backend::{
    Config, THUMB_H, THUMB_W, ThumbResult, add_favorite, apply_theme, is_video, load_config,
    load_favorites, remove_favorite, save_favorites, scan_dir, spawn_thumbnail_worker,
    thumbnail_cache_path,
};

// Константы

const SPACING: i32 = 12;
const MARGIN: i32 = 16;
const INDICATOR_WIDTH: i32 = 40;
const MAX_DOTS: usize = 15;
const MAX_LABEL_CHARS: usize = 3;
const FAVORITE_FLASH_MS: u64 = 300;
const SCROLL_PAGE_THRESHOLD: f64 = 1.0;

fn compute_grid(avail_w: i32, avail_h: i32) -> (usize, usize) {
    let cell_w = THUMB_W + SPACING;
    let cell_h = THUMB_H + SPACING;
    let usable_w = (avail_w - MARGIN * 2).max(cell_w);
    let usable_h = (avail_h - MARGIN * 2).max(cell_h);
    let cols = (usable_w / cell_w).max(1) as usize;
    let rows = (usable_h / cell_h).max(1) as usize;
    (cols, rows)
}

// Состояние

#[derive(Clone, Copy, PartialEq)]
pub enum View {
    All,
    Favorites,
}

#[derive(Clone, Copy, Default)]
pub struct ViewPosition {
    pub page: usize,
    pub row: usize,
    pub col: usize,
}

pub struct Cell {
    pub btn: Button,
    pub pic: Picture,
    pub heart: Label,
    pub video_badge: Label,
}

pub struct AppState {
    pub config: Rc<Config>,
    pub job_tx: mpsc::Sender<PathBuf>,
    pub all_wallpapers: Vec<PathBuf>,
    pub favorite_wallpapers: Vec<PathBuf>,
    pub view: View,
    pub all_position: ViewPosition,
    pub favorites_position: ViewPosition,
    pub page: usize,
    pub selected_row: usize,
    pub selected_col: usize,
    pub cols: usize,
    pub rows: usize,
    pub pending_jobs: HashSet<PathBuf>,
    pub dot_window_start: usize,

    pub cells: Vec<Cell>,
    pub thumb_map: HashMap<PathBuf, usize>,
    pub flow: FlowBox,
    pub indicator_box: GtkBox,
    pub current_label: Label,
    pub total_label: Label,
    pub view_all_label: Label,
    pub view_favorites_label: Label,
}

impl AppState {
    pub fn current_list(&self) -> &[PathBuf] {
        match self.view {
            View::All => &self.all_wallpapers,
            View::Favorites => &self.favorite_wallpapers,
        }
    }

    pub fn page_size(&self) -> usize {
        self.cols * self.rows
    }

    pub fn total_pages(&self) -> usize {
        if self.current_list().is_empty() {
            1
        } else {
            (self.current_list().len() + self.page_size() - 1) / self.page_size()
        }
    }

    pub fn local_index(&self) -> usize {
        self.selected_row * self.cols + self.selected_col
    }

    pub fn selected_index(&self) -> usize {
        self.page * self.page_size() + self.local_index()
    }

    pub fn visible_count(&self) -> usize {
        let start = self.page * self.page_size();
        self.current_list()
            .len()
            .saturating_sub(start)
            .min(self.page_size())
    }
}

pub type SharedState = Rc<RefCell<AppState>>;

// build_ui

pub fn build_ui(app: &Application) {
    if gtk4::gdk::Display::default().is_none() {
        eprintln!("Error: GDK cannot open display. Are you running in a non-GUI environment?");
        std::process::exit(1);
    }

    let config = Rc::new(load_config());

    if let Err(e) = std::fs::create_dir_all(&config.thumb_cache_dir) {
        eprintln!(
            "Warning: Failed to create thumbnail cache directory at {:?}: {}",
            config.thumb_cache_dir, e
        );
        eprintln!("Previews will not be cached between sessions.");
    }

    let (job_tx, result_rx) = spawn_thumbnail_worker(config.thumb_cache_dir.clone());

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Theme Picker")
        .default_width(700)
        .default_height(600)
        .resizable(true)
        .build();

    let flow = FlowBox::builder()
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

    let all_wallpapers = scan_dir(&config.wallpaper_dir, config.drivers.video);
    let favorite_wallpapers = load_favorites(&config.wallpaper_dir, &config.thumb_cache_dir);

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

    let state: SharedState = Rc::new(RefCell::new(AppState {
        config,
        job_tx,
        all_wallpapers,
        favorite_wallpapers,
        view: View::All,
        all_position: ViewPosition::default(),
        favorites_position: ViewPosition::default(),
        page: 0,
        selected_row: 0,
        selected_col: 0,
        cols: init_cols,
        rows: init_rows,
        pending_jobs: HashSet::new(),
        dot_window_start: 0,
        cells: Vec::new(),
        thumb_map: HashMap::new(),
        flow: flow.clone(),
        indicator_box: indicator_box.clone(),
        current_label,
        total_label,
        view_all_label,
        view_favorites_label,
    }));

    ensure_pool(&state);
    render_page(&state);
    update_view_indicator(&state);

    let scroll = ScrolledWindow::builder()
        .child(&flow)
        .vexpand(true)
        .hexpand(true)
        .hscrollbar_policy(gtk4::PolicyType::External)
        .vscrollbar_policy(gtk4::PolicyType::External)
        .build();
    scroll.add_css_class("grid-frame");

    let content_row = GtkBox::new(Orientation::Horizontal, 0);
    content_row.append(&scroll);
    content_row.append(&indicator_wrapper);

    let root = GtkBox::new(Orientation::Vertical, 0);
    root.append(&view_indicator);
    root.append(&content_row);
    window.set_child(Some(&root));

    load_css();
    attach_input(&state, &window, &scroll);
    attach_thumbnail_listener(&state, result_rx);

    window.present();
}

// Ввод

fn attach_input(state: &SharedState, window: &ApplicationWindow, scroll: &ScrolledWindow) {
    for (label, target) in [
        (state.borrow().view_all_label.clone(), View::All),
        (state.borrow().view_favorites_label.clone(), View::Favorites),
    ] {
        let state = state.clone();
        let click = gtk4::GestureClick::new();
        click.connect_released(move |_, _, _, _| {
            switch_view(&state, target);
        });
        label.add_controller(click);
    }

    let key_controller = gtk4::EventControllerKey::new();
    {
        let state = state.clone();
        let window = window.clone();
        key_controller.connect_key_pressed(move |_, key, _, _| {
            match key {
                Key::Escape => {
                    window.close();
                }
                Key::Return | Key::KP_Enter => {
                    activate_selected(&state);
                }
                Key::Tab => {
                    let target = match state.borrow().view {
                        View::All => View::Favorites,
                        View::Favorites => View::All,
                    };
                    switch_view(&state, target);
                }
                Key::f | Key::F | Key::Cyrillic_a | Key::Cyrillic_A => {
                    toggle_favorite(&state);
                }
                Key::Left | Key::Right | Key::Up | Key::Down => {
                    move_selection(&state, key);
                }
                _ => return glib::Propagation::Proceed,
            }
            glib::Propagation::Stop
        });
    }
    window.add_controller(key_controller);

    let scroll_controller =
        gtk4::EventControllerScroll::new(gtk4::EventControllerScrollFlags::VERTICAL);
    scroll_controller.set_propagation_phase(gtk4::PropagationPhase::Capture);
    {
        let state = state.clone();
        let accum = std::cell::Cell::new(0.0f64);
        scroll_controller.connect_scroll(move |_, _dx, dy| {
            let acc = accum.get() + dy;
            if acc.abs() < SCROLL_PAGE_THRESHOLD {
                accum.set(acc);
                return glib::Propagation::Stop;
            }
            accum.set(0.0);

            let mut s = state.borrow_mut();
            let paged = if acc > 0.0 && s.page + 1 < s.total_pages() {
                s.page += 1;
                true
            } else if acc < 0.0 && s.page > 0 {
                s.page -= 1;
                true
            } else {
                false
            };
            if !paged {
                return glib::Propagation::Stop;
            }
            s.selected_row = 0;
            s.selected_col = 0;
            drop(s);
            render_page(&state);
            glib::Propagation::Stop
        });
    }
    window.add_controller(scroll_controller);

    {
        let state = state.clone();
        let scroll = scroll.clone();
        let last_size = std::cell::Cell::new((0i32, 0i32));
        window.add_tick_callback(move |_win, _clock| {
            let (w, h) = (scroll.width(), scroll.height());
            if w > 0 && h > 0 && (w, h) != last_size.get() {
                last_size.set((w, h));
                handle_resize(&state, w, h);
            }
            glib::ControlFlow::Continue
        });
    }
}

fn activate_selected(state: &SharedState) {
    let s = state.borrow();
    let idx = s.selected_index();
    if idx < s.current_list().len() {
        apply_theme(&s.current_list()[idx], &s.config);
    }
}

fn move_selection(state: &SharedState, key: Key) {
    let mut s = state.borrow_mut();
    let mut page_changed = false;

    match key {
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
                page_changed = true;
            }
        }
        Key::Down => {
            if s.selected_row < s.rows - 1 {
                s.selected_row += 1;
            } else if s.page + 1 < s.total_pages() {
                s.page += 1;
                s.selected_row = 0;
                page_changed = true;
            }
        }
        _ => {}
    }

    drop(s);
    if page_changed {
        render_page(state);
    } else {
        clamp_and_highlight(state);
    }
}

fn toggle_favorite(state: &SharedState) {
    let mut s = state.borrow_mut();
    let local_idx = s.local_index();
    let global_idx = s.selected_index();

    if global_idx >= s.current_list().len() {
        return;
    }
    let path = s.current_list()[global_idx].clone();

    let mut added: Option<bool> = None;
    let mut needs_rebuild = false;

    match s.view {
        View::All => {
            if add_favorite(&path, &mut s.favorite_wallpapers) {
                save_favorites(&s.config.wallpaper_dir, &s.favorite_wallpapers);
                added = Some(true);
            }
        }
        View::Favorites => {
            if remove_favorite(&path, &mut s.favorite_wallpapers) {
                save_favorites(&s.config.wallpaper_dir, &s.favorite_wallpapers);
                added = Some(false);
                needs_rebuild = true;
            }
        }
    }

    if let Some(was_added) = added {
        if let Some(cell) = s.cells.get(local_idx) {
            flash_favorite(&cell.btn, &cell.heart, was_added);
        }
    }

    drop(s);
    if needs_rebuild {
        let state = state.clone();
        glib::timeout_add_local_once(Duration::from_millis(FAVORITE_FLASH_MS), move || {
            render_page(&state);
        });
    }
}

fn handle_resize(state: &SharedState, w: i32, h: i32) {
    let mut s = state.borrow_mut();
    let (cols, rows) = compute_grid(w, h);
    if cols == s.cols && rows == s.rows {
        return;
    }

    let old_page_size = s.page_size();
    let global_idx = if old_page_size > 0 {
        s.selected_index()
    } else {
        0
    }
    .min(s.current_list().len().saturating_sub(1));

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

    ensure_pool(state);
    render_page(state);
}

// Рендеринг

pub fn render_page(state: &SharedState) {
    populate_page(state);
    update_page_labels(state);
    rebuild_dot_indicator(state);
    clamp_and_highlight(state);
}

pub fn switch_view(state: &SharedState, target: View) {
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

    let saved = match target {
        View::All => s.all_position,
        View::Favorites => s.favorites_position,
    };
    s.page = saved.page;
    s.selected_row = saved.row;
    s.selected_col = saved.col;

    drop(s);
    render_page(state);
    update_view_indicator(state);
}

fn flash_favorite(btn: &Button, heart: &Label, added: bool) {
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

// Пул ячеек

fn ensure_pool(state: &SharedState) {
    let (flow, have, need) = {
        let s = state.borrow();
        (s.flow.clone(), s.cells.len(), s.page_size())
    };
    if have >= need {
        return;
    }

    for cell_idx in have..need {
        let btn = Button::builder().build();
        btn.add_css_class("thumb-button");
        btn.set_can_focus(false);
        btn.set_overflow(Overflow::Hidden);
        btn.set_size_request(THUMB_W, THUMB_H);

        let pic = Picture::new();
        pic.set_content_fit(ContentFit::Cover);
        pic.set_size_request(THUMB_W, THUMB_H);
        pic.set_can_shrink(true);
        pic.set_overflow(Overflow::Hidden);

        let overlay = Overlay::new();
        overlay.set_child(Some(&pic));

        let heart = Label::new(Some("♥"));
        heart.add_css_class("favorite-heart");
        heart.set_halign(gtk4::Align::Center);
        heart.set_valign(gtk4::Align::Center);
        heart.set_can_target(false);
        overlay.add_overlay(&heart);

        let video_badge = Label::new(Some("▶"));
        video_badge.add_css_class("video-badge");
        video_badge.set_halign(gtk4::Align::Start);
        video_badge.set_valign(gtk4::Align::Start);
        video_badge.set_can_target(false);
        video_badge.set_visible(false);
        overlay.add_overlay(&video_badge);

        btn.set_child(Some(&overlay));

        {
            let state = state.clone();
            btn.connect_clicked(move |_| {
                let s = state.borrow();
                let idx = s.page * s.page_size() + cell_idx;
                let list = s.current_list();
                if idx < list.len() {
                    apply_theme(&list[idx], &s.config);
                }
            });
        }

        let motion = gtk4::EventControllerMotion::new();
        {
            let state = state.clone();
            motion.connect_enter(move |_, _, _| {
                let mut s = state.borrow_mut();
                if cell_idx >= s.page_size() {
                    return;
                }
                let cols = s.cols;
                let row = cell_idx / cols;
                let col = cell_idx % cols;
                if s.selected_row != row || s.selected_col != col {
                    s.selected_row = row;
                    s.selected_col = col;
                    drop(s);
                    clamp_and_highlight(&state);
                }
            });
        }
        btn.add_controller(motion);

        flow.append(&btn);
        state.borrow_mut().cells.push(Cell {
            btn,
            pic,
            heart,
            video_badge,
        });
    }
}

fn populate_page(state: &SharedState) {
    let mut s = state.borrow_mut();

    let total = s.total_pages();
    if s.page >= total {
        s.page = total - 1;
    }

    s.thumb_map.clear();

    let page_size = s.page_size();
    let start = s.page * page_size;
    let len = s.current_list().len();
    let config = s.config.clone();
    let job_tx = s.job_tx.clone();

    for cell_idx in 0..s.cells.len() {
        let (btn, pic, video_badge) = {
            let c = &s.cells[cell_idx];
            (c.btn.clone(), c.pic.clone(), c.video_badge.clone())
        };
        let global = start + cell_idx;
        let visible = cell_idx < page_size && global < len;

        if let Some(wrapper) = btn.parent() {
            wrapper.set_visible(visible);
        }
        if !visible {
            btn.remove_css_class("thumb-loading");
            pic.set_filename(None::<&Path>);
            video_badge.set_visible(false);
            continue;
        }

        let path = s.current_list()[global].clone();
        video_badge.set_visible(is_video(&path));
        let thumb = thumbnail_cache_path(&path, &config.thumb_cache_dir);

        if thumb.exists() {
            btn.remove_css_class("thumb-loading");
            pic.set_filename(Some(&thumb));
        } else {
            pic.set_filename(None::<&Path>);
            btn.add_css_class("thumb-loading");
            if !s.pending_jobs.contains(&path) {
                s.pending_jobs.insert(path.clone());
                if let Err(e) = job_tx.send(path.clone()) {
                    eprintln!("Error: Failed to queue thumbnail job for {:?}: {}", path, e);
                }
            }
            s.thumb_map.insert(path, cell_idx);
        }
    }
}

fn clamp_and_highlight(state: &SharedState) {
    let mut s = state.borrow_mut();
    let count = s.visible_count();
    let cols = s.cols;

    if count == 0 {
        s.selected_row = 0;
        s.selected_col = 0;
        for cell in &s.cells {
            cell.btn.remove_css_class("selected");
        }
        return;
    }

    let max_idx = count - 1;
    let mut idx = s.local_index();
    if idx > max_idx {
        idx = max_idx;
        s.selected_row = idx / cols;
        s.selected_col = idx % cols;
    }
    for (i, cell) in s.cells.iter().enumerate() {
        if i == idx {
            cell.btn.add_css_class("selected");
        } else {
            cell.btn.remove_css_class("selected");
        }
    }
}

// Индикаторы

fn format_page_number(n: usize) -> String {
    let text = n.to_string();
    if text.len() > MAX_LABEL_CHARS {
        "…".to_string()
    } else {
        text
    }
}

fn update_page_labels(state: &SharedState) {
    let s = state.borrow();
    let total = s.total_pages();
    let current = s.page;
    s.current_label.set_text(&format_page_number(current + 1));
    s.total_label.set_text(&format_page_number(total));
}

fn rebuild_dot_indicator(state: &SharedState) {
    let mut s = state.borrow_mut();

    while let Some(child) = s.indicator_box.first_child() {
        s.indicator_box.remove(&child);
    }

    let total = s.total_pages();
    let current = s.page;

    if current < s.dot_window_start {
        s.dot_window_start = current;
    } else if current >= s.dot_window_start + MAX_DOTS {
        s.dot_window_start = current + 1 - MAX_DOTS;
    }
    let max_start = total.saturating_sub(MAX_DOTS);
    s.dot_window_start = s.dot_window_start.min(max_start);

    let dot_start = s.dot_window_start;
    let dot_end = (dot_start + MAX_DOTS).min(total);

    if dot_start > 0 {
        let ell = Label::new(Some("…"));
        ell.add_css_class("page-dot");
        s.indicator_box.append(&ell);
    }
    for i in dot_start..dot_end {
        let dot = Label::new(Some(if i == current { "●" } else { "○" }));
        dot.add_css_class("page-dot");
        s.indicator_box.append(&dot);
    }
    if dot_end < total {
        let ell = Label::new(Some("…"));
        ell.add_css_class("page-dot");
        s.indicator_box.append(&ell);
    }
}

fn update_view_indicator(state: &SharedState) {
    let s = state.borrow();
    match s.view {
        View::All => {
            s.view_all_label.add_css_class("view-tab-active");
            s.view_favorites_label.remove_css_class("view-tab-active");
        }
        View::Favorites => {
            s.view_all_label.remove_css_class("view-tab-active");
            s.view_favorites_label.add_css_class("view-tab-active");
        }
    }
}

// Слушатель миниатюр

fn attach_thumbnail_listener(state: &SharedState, result_rx: async_channel::Receiver<ThumbResult>) {
    let state = state.clone();
    glib::MainContext::default().spawn_local(async move {
        while let Ok((source, thumb)) = result_rx.recv().await {
            let mut s = state.borrow_mut();
            s.pending_jobs.remove(&source);
            if let Some(&cell_idx) = s.thumb_map.get(&source) {
                if let Some(cell) = s.cells.get(cell_idx) {
                    cell.pic.set_filename(Some(&thumb));
                    cell.btn.remove_css_class("thumb-loading");
                }
                s.thumb_map.remove(&source);
            }
        }
    });
}

// CSS

fn load_css() {
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(include_str!("style.css"));

    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    } else {
        eprintln!("Warning: Could not style application. No active GDK Display found.");
    }
}
