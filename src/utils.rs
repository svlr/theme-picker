pub const THUMB_W: i32 = 200;
pub const THUMB_H: i32 = 120;
pub const SPACING: i32 = 12;
pub const MARGIN: i32 = 16;
pub const INDICATOR_WIDTH: i32 = 40;
pub const MAX_DOTS: usize = 15;
pub const MAX_LABEL_CHARS: usize = 3;
pub const FAVORITE_FLASH_MS: u64 = 300;

/// Вычисление размера сетки на основе размеров окна
pub fn compute_grid(width: i32, height: i32) -> (usize, usize) {
    let cell_w = THUMB_W + SPACING;
    let cell_h = THUMB_H + SPACING;
    let usable_w = (width - MARGIN * 2 - INDICATOR_WIDTH).max(cell_w);
    let usable_h = (height - MARGIN * 2).max(cell_h);
    let cols = (usable_w / cell_w).max(1) as usize;
    let rows = (usable_h / cell_h).max(1) as usize;
    (cols, rows)
}
