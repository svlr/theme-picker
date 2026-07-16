use std::cell::RefCell;
use std::rc::Rc;
use gtk4::prelude::*;
use gtk4::{Button, ContentFit, Overflow, Overlay, Picture, Label};
use crate::state::AppState;
use crate::utils::{THUMB_W, THUMB_H};
use crate::filesystem::thumbnail_cache_path;
use crate::hooks::run_theme_hook;

pub fn rebuild_thumbnail_grid(state: &Rc<RefCell<AppState>>) {
    let mut s = state.borrow_mut();

    while let Some(child) = s.flow.first_child() {
        s.flow.remove(&child);
    }
    s.buttons.clear();
    s.heart_labels.clear();
    s.thumb_map.clear();
    
    let page_size = s.page_size();
    let start = s.page * page_size;
    let end = (start + page_size).min(s.wallpapers.len());
    let cols = s.cols;

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
                    eprintln!("Error: Failed to queue thumbnail job for {:?}: {}", path, e);
                }
            }
        }

        let overlay = Overlay::new();
        overlay.set_child(Some(&pic));

        let heart = Label::new(Some("♥"));
        heart.add_css_class("favorite-heart");
        heart.set_halign(gtk4::Align::Center);
        heart.set_valign(gtk4::Align::Center);
        heart.set_visible(true);
        heart.set_can_target(false);
        overlay.add_overlay(&heart);

        btn.set_child(Some(&overlay));

        let path_for_click = path.clone();
        let hook_image_c = hook_image.clone();
        let hook_video_c = hook_video.clone();
        btn.connect_clicked(move |_| {
            run_theme_hook(
                &path_for_click,
                drivers_image,
                drivers_video,
                &hook_image_c,
                hook_video_c.as_deref(),
            );
        });

        let row = (i - start) / cols;
        let col = (i - start) % cols;
        let motion = gtk4::EventControllerMotion::new();
        {
            let state = state.clone();
            motion.connect_enter(move |_, _, _| {
                let mut s = state.borrow_mut();
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
        s.thumb_map.insert(path.clone(), (pic, btn.clone()));
        s.buttons.push(btn);
        s.heart_labels.push(heart);
    }
}

pub fn clamp_and_highlight(state: &Rc<RefCell<AppState>>) {
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
