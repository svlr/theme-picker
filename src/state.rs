use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc;
use gtk4::{Button, FlowBox, Label, Box as GtkBox, Picture};
use crate::config::Config;

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

pub struct AppState {
    pub favorites_path: PathBuf,
    pub config: Config,
    pub job_tx: mpsc::Sender<PathBuf>,
    pub all_wallpapers: Vec<PathBuf>,
    pub favorite_wallpapers: Vec<PathBuf>,
    pub wallpapers: Vec<PathBuf>,
    pub view: View,
    pub all_position: ViewPosition,
    pub favorites_position: ViewPosition,
    pub page: usize,
    pub selected_row: usize,
    pub selected_col: usize,
    pub cols: usize,
    pub rows: usize,
    pub buttons: Vec<Button>,
    pub heart_labels: Vec<Label>,
    pub thumb_map: HashMap<PathBuf, (Picture, Button)>,
    pub pending_jobs: HashSet<PathBuf>,
    pub dot_window_start: usize,
    pub flow: FlowBox,
    pub indicator_box: GtkBox,
    pub current_label: Label,
    pub total_label: Label,
    pub view_all_label: Label,
    pub view_favorites_label: Label,
}

impl AppState {
    pub fn page_size(&self) -> usize {
        self.cols * self.rows
    }
    
    pub fn total_pages(&self) -> usize {
        if self.wallpapers.is_empty() {
            1
        } else {
            (self.wallpapers.len() + self.page_size() - 1) / self.page_size()
        }
    }
}
