use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Button, ContentFit, CssProvider,
    FlowBox, Label, Orientation, Overflow, Picture, ScrolledWindow,
};
use gtk4::gdk::Display;
use gtk4::glib;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::mpsc;

// Размеры и отступы сетки миниатюр
const THUMB_W: i32 = 200;
const THUMB_H: i32 = 120;
const SPACING: i32 = 12;
const MARGIN: i32 = 16;
const INDICATOR_WIDTH: i32 = 40;
const MAX_DOTS: usize = 15;
const MAX_LABEL_CHARS: usize = 3;

const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp", "bmp", "gif", "tiff", "avif"];
// Пока не используется при сканировании — маршрутизация готова заранее
const VIDEO_EXTS: &[&str] = &["mp4", "webm", "mkv"];

// Конфиг из config.toml
#[derive(Deserialize)]
struct Config {
    wallpaper_dir: PathBuf,
    thumb_cache_dir: PathBuf,
    drivers: Drivers,
    hooks: Hooks,
}

// Активные типы обоев
#[derive(Deserialize)]
struct Drivers {
    image: bool,
    #[serde(default)]
    video: bool,
}

// Скрипты применения тем
#[derive(Deserialize)]
struct Hooks {
    image: PathBuf,
    #[serde(default)]
    video: Option<PathBuf>,
}

fn load_config() -> Config {
    let path = glib::user_config_dir()
        .join("theme-picker")
        .join("config.toml");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Config not found at {:?}", path));
    toml::from_str(&text).expect("Invalid config.toml")
}

// Состояние приложения
struct AppState {
    wallpapers: Vec<PathBuf>,
    page: usize,
    selected_row: usize,
    selected_col: usize,
    cols: usize,
    rows: usize,
    buttons: Vec<Button>,
    thumb_map: HashMap<PathBuf, (Picture, Button)>,
    pending_jobs: HashSet<PathBuf>,
    dot_window_start: usize,
    flow: FlowBox,
    indicator_box: GtkBox,
    current_label: Label,
    total_label: Label,
    config: Config,
    job_tx: mpsc::Sender<PathBuf>,
}

impl AppState {
    // Кол-во миниатюр на странице
    fn page_size(&self) -> usize {
        self.cols * self.rows
    }
    // Всего страниц
    fn total_pages(&self) -> usize {
        if self.wallpapers.is_empty() {
            1
        } else {
            (self.wallpapers.len() + self.page_size() - 1) / self.page_size()
        }
    }
}

fn main() {
    let app = Application::builder()
        .application_id("dev.svlr.theme-picker")
        .build();
    app.connect_activate(build_ui);
    app.run();
}

fn build_ui(app: &Application) {
    let config = load_config();
    if let Err(e) = std::fs::create_dir_all(&config.thumb_cache_dir) {
        eprintln!("failed to create thumbnail cache dir: {e}");
    }
    // Фоновый воркер генерации миниатюр
    let (job_tx, job_rx) = mpsc::channel::<PathBuf>();
    let (result_tx, result_rx) = async_channel::unbounded::<(PathBuf, PathBuf)>();
    {
        let cache_dir = config.thumb_cache_dir.clone();
        std::thread::spawn(move || {
            for source in job_rx {
                let thumb = thumbnail_cache_path(&source, &cache_dir);
                if !thumb.exists() {
                    let output = Command::new("vipsthumbnail")
                        .arg(&source)
                        .arg("-s")
                        .arg(format!("{}x{}", THUMB_W, THUMB_H))
                        .arg("--smartcrop")
                        .arg("attention")
                        .arg("-o")
                        .arg(format!("{}[Q=85]", thumb.to_string_lossy()))
                        .output();
                    match output {
                        Ok(out) if !out.status.success() => {
                            eprintln!(
                                "vipsthumbnail failed for {:?}: {}",
                                source,
                                String::from_utf8_lossy(&out.stderr)
                            );
                        }
                        Err(e) => {
                            eprintln!("failed to spawn vipsthumbnail for {:?}: {e}", source);
                        }
                        _ => {}
                    }
                }
                if let Err(e) = result_tx.send_blocking((source.clone(), thumb)) {
                    eprintln!("failed to send thumbnail result for {:?}: {e}", source);
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

    // Скан директории обоев без рекурсии, фильтр по расширениям
    let mut wallpapers: Vec<PathBuf> = walkdir::WalkDir::new(&config.wallpaper_dir)
        .max_depth(1)
        .into_iter()
        .filter_map(|entry| match entry {
            Ok(e) => Some(e),
            Err(e) => {
                eprintln!("failed to read wallpaper dir entry: {e}");
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
    wallpapers.sort();

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

    let (init_cols, init_rows) = compute_grid(700, 600);
    flow.set_min_children_per_line(1);
    flow.set_max_children_per_line(init_cols as u32);
    let state = Rc::new(RefCell::new(AppState {
        wallpapers,
        page: 0,
        selected_row: 0,
        selected_col: 0,
        cols: init_cols,
        rows: init_rows,
        buttons: Vec::new(),
        thumb_map: HashMap::new(),
        pending_jobs: HashSet::new(),
        dot_window_start: 0,
        flow: flow.clone(),
        indicator_box: indicator_box.clone(),
        current_label: current_label.clone(),
        total_label: total_label.clone(),
        config,
        job_tx,
    }));

    render_page(&state);

    let scroll = ScrolledWindow::builder()
        .child(&flow)
        .vexpand(true)
        .hexpand(true)
        .hscrollbar_policy(gtk4::PolicyType::External)
        .vscrollbar_policy(gtk4::PolicyType::External)
        .build();

    let root = GtkBox::new(Orientation::Horizontal, 0);
    root.append(&scroll);
    root.append(&indicator_wrapper);
    window.set_child(Some(&root));

    load_css();

    // Навигация клавиатурой: стрелки, Enter, Escape
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

    // Колесо мыши листает страницы
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

    // Пересчёт сетки при ресайзе окна
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

    // Отслеживание реальных изменений размера через tick callback
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

    // Приём готовых миниатюр из фонового потока
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

// Кол-во колонок/строк под заданный размер окна
fn compute_grid(width: i32, height: i32) -> (usize, usize) {
    let cell_w = THUMB_W + SPACING;
    let cell_h = THUMB_H + SPACING;
    let usable_w = (width - MARGIN * 2 - INDICATOR_WIDTH).max(cell_w);
    let usable_h = (height - MARGIN * 2).max(cell_h);
    let cols = (usable_w / cell_w).max(1) as usize;
    let rows = (usable_h / cell_h).max(1) as usize;
    (cols, rows)
}

// Номер страницы с обрезкой длинных чисел
fn format_page_number(n: usize) -> String {
    let text = n.to_string();
    if text.len() > MAX_LABEL_CHARS {
        "…".to_string()
    } else {
        text
    }
}

// Полная перерисовка страницы
fn render_page(state: &Rc<RefCell<AppState>>) {
    rebuild_thumbnail_grid(state);
    update_page_labels(state);
    rebuild_dot_indicator(state);
    clamp_and_highlight(state);
}

// Пересоздание кнопок-миниатюр для текущей страницы
fn rebuild_thumbnail_grid(state: &Rc<RefCell<AppState>>) {
    let mut s = state.borrow_mut();

    while let Some(child) = s.flow.first_child() {
        s.flow.remove(&child);
    }
    s.buttons.clear();
    s.thumb_map.clear();
    let page_size = s.page_size();
    let start = s.page * page_size;
    let end = (start + page_size).min(s.wallpapers.len());

    let flow = s.flow.clone();
    let thumb_cache_dir = s.config.thumb_cache_dir.clone();
    let job_tx = s.job_tx.clone();

    let drivers_image = s.config.drivers.image;
    let drivers_video = s.config.drivers.video;
    let hook_image = s.config.hooks.image.clone();
    let hook_video = s.config.hooks.video.clone();

    for i in start..end {
        let path = s.wallpapers[i].clone();
        let thumb_path = thumbnail_cache_path(&path, &thumb_cache_dir);

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

        if thumb_path.exists() {
            pic.set_filename(Some(&thumb_path));
        } else {
            btn.add_css_class("thumb-loading");
            if !s.pending_jobs.contains(&path) {
                s.pending_jobs.insert(path.clone());
                if let Err(e) = job_tx.send(path.clone()) {
                    eprintln!("failed to queue thumbnail job for {:?}: {e}", path);
                }
            }
        }

        btn.set_child(Some(&pic));

        let path_for_click = path.clone();
        let hook_image = hook_image.clone();
        let hook_video = hook_video.clone();
        btn.connect_clicked(move |_| {
            run_theme_hook(
                &path_for_click,
                drivers_image,
                drivers_video,
                &hook_image,
                hook_video.as_deref(),
            );
        });

        flow.append(&btn);
        s.thumb_map.insert(path.clone(), (pic, btn.clone()));
        s.buttons.push(btn);
    }
}

// Обновление лейблов страницы
fn update_page_labels(state: &Rc<RefCell<AppState>>) {
    let s = state.borrow();
    let total = s.total_pages();
    let current = s.page;
    s.current_label.set_text(&format_page_number(current + 1));
    s.total_label.set_text(&format_page_number(total));
}

// Точки-индикаторы страниц со скользящим окном
fn rebuild_dot_indicator(state: &Rc<RefCell<AppState>>) {
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

// Подсветка выделенной миниатюры с ограничением по числу кнопок
fn clamp_and_highlight(state: &Rc<RefCell<AppState>>) {
    let mut s = state.borrow_mut();
    let count = s.buttons.len();
    let cols = s.cols;

    if count == 0 {
        s.selected_row = 0;
        s.selected_col = 0;
        return;
    }

    let max_idx = count - 1;
    let mut idx = s.selected_row * cols + s.selected_col;
    if idx > max_idx {
        idx = max_idx;
        s.selected_row = idx / cols;
        s.selected_col = idx % cols;
    }
    for (i, btn) in s.buttons.iter().enumerate() {
        if i == idx {
            btn.add_css_class("selected");
        } else {
            btn.remove_css_class("selected");
        }
    }
}

// Путь к миниатюре в кэше по хэшу исходного файла
fn thumbnail_cache_path(source: &Path, cache_dir: &Path) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(source.to_string_lossy().as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    cache_dir.join(format!("{}.jpg", hash))
}

fn apply_theme(wallpaper: &Path, config: &Config) {
    run_theme_hook(
        wallpaper,
        config.drivers.image,
        config.drivers.video,
        &config.hooks.image,
        config.hooks.video.as_deref(),
    );
}

fn run_theme_hook(
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
                "video wallpaper selected but drivers.video=false in config: {:?}",
                wallpaper
            );
            return;
        }
        match hook_video {
            Some(hook) => spawn_hook(hook, wallpaper),
            None => eprintln!("drivers.video=true but hooks.video is not set"),
        }
        return;
    }

    if !drivers_image {
        eprintln!(
            "image wallpaper selected but drivers.image=false in config: {:?}",
            wallpaper
        );
        return;
    }
    spawn_hook(hook_image, wallpaper);
}

// Запуск внешнего хука без ожидания завершения
fn spawn_hook(hook: &Path, wallpaper: &Path) {
    if let Err(e) = Command::new(hook).arg(wallpaper).spawn() {
        eprintln!("failed to run hook {:?} for {:?}: {e}", hook, wallpaper);
    }
}

// CSS-стили кнопок, точек и лейблов
fn load_css() {
    let provider = CssProvider::new();
    provider.load_from_data(
        "
        .thumb-button {
            border-radius: 10px;
            border: 2px solid transparent;
            padding: 0;
        }
        .thumb-button.selected {
            border: 2px solid #ffb688;
        }
        .thumb-button.thumb-loading {
            background-color: alpha(#ffffff, 0.06);
        }
        .page-dot {
            font-size: 10px;
            opacity: 0.4;
        }
        .page-number {
            font-size: 11px;
            font-weight: bold;
            opacity: 0.6;
        }
        ",
    );
    gtk4::style_context_add_provider_for_display(
        &Display::default().expect("no display"),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
