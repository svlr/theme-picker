use std::cell::RefCell;
use std::rc::Rc;
use gtk4::prelude::*;
use gtk4::Label;
use crate::state::AppState;
use crate::utils::{MAX_LABEL_CHARS, MAX_DOTS};

pub fn format_page_number(n: usize) -> String {
    let text = n.to_string();
    if text.len() > MAX_LABEL_CHARS {
        "…".to_string()
    } else {
        text
    }
}

pub fn update_page_labels(state: &Rc<RefCell<AppState>>) {
    let s = state.borrow();
    let total = s.total_pages();
    let current = s.page;
    s.current_label.set_text(&format_page_number(current + 1));
    s.total_label.set_text(&format_page_number(total));
}

pub fn rebuild_dot_indicator(state: &Rc<RefCell<AppState>>) {
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

pub fn update_view_indicator(state: &Rc<RefCell<AppState>>) {
    let s = state.borrow();
    match s.view {
        crate::state::View::All => {
            s.view_all_label.add_css_class("view-tab-active");
            s.view_favorites_label.remove_css_class("view-tab-active");
        }
        crate::state::View::Favorites => {
            s.view_all_label.remove_css_class("view-tab-active");
            s.view_favorites_label.add_css_class("view-tab-active");
        }
    }
}
