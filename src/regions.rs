//! Monitor-relative cells. Tiles retain their normal size; overflowing groups scroll.
use serde::{Deserialize, Serialize};
pub const COLUMNS: u8 = 8;
pub const ROWS: u8 = 6;
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Region {
    pub left: u8,
    pub top: u8,
    pub columns: u8,
    pub rows: u8,
}
impl Region {
    pub fn from_cells(a: (u8, u8), b: (u8, u8)) -> Self {
        let left = a.0.min(b.0).min(COLUMNS - 2);
        let top = a.1.min(b.1).min(ROWS - 1);
        Self {
            left,
            top,
            columns: (a.0.abs_diff(b.0) + 1).max(2).min(COLUMNS - left),
            rows: (a.1.abs_diff(b.1) + 1).min(ROWS - top),
        }
    }
    pub fn valid(self) -> bool {
        self.columns >= 2
            && self.rows >= 1
            && self.left as u16 + self.columns as u16 <= COLUMNS as u16
            && self.top as u16 + self.rows as u16 <= ROWS as u16
    }
    pub fn overlaps(self, other: Self) -> bool {
        self.left < other.left + other.columns
            && other.left < self.left + self.columns
            && self.top < other.top + other.rows
            && other.top < self.top + self.rows
    }
    pub fn pixels(
        self,
        width: i32,
        height: i32,
        min_width: i32,
        min_height: i32,
    ) -> (i32, i32, i32, i32) {
        let w = (width * self.columns as i32 / COLUMNS as i32)
            .max(min_width)
            .min(width);
        let h = (height * self.rows as i32 / ROWS as i32)
            .max(min_height)
            .min(height);
        (
            (width * self.left as i32 / COLUMNS as i32).min(width - w),
            (height * self.top as i32 / ROWS as i32).min(height - h),
            w,
            h,
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reverse_drag_minimum_size_edge_clamping_and_monitor_resize() {
        let region = Region::from_cells((5, 0), (3, 1));
        assert!(region.valid());
        assert_eq!(region.pixels(1920, 1080, 240, 150), (720, 0, 720, 360));
        let small = Region::from_cells((7, 5), (7, 5));
        assert!(small.valid());
        let (x, y, w, h) = small.pixels(800, 480, 280, 180);
        assert!(w >= 280 && h >= 180 && x + w <= 800 && y + h <= 480);
        assert!(region.overlaps(Region::from_cells((4, 0), (5, 1))));
        assert!(!region.overlaps(Region::from_cells((0, 4), (2, 5))));
    }
}
