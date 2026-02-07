#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellColor {
    DefaultForeground,
    DefaultBackground,
    Ansi256(u8),
    Rgb { r: u8, g: u8, b: u8 },
}

#[derive(Debug, Clone, Copy)]
pub struct ScreenCell {
    pub ch: char,
    pub fg: CellColor,
    pub bg: CellColor,
    pub selected: bool,
}

#[derive(Debug, Clone)]
pub struct ScreenSnapshot {
    pub cols: u16,
    pub rows: u16,
    pub cursor_col: u16,
    pub cursor_row: u16,
    pub cursor_visible: bool,
    pub default_fg: CellColor,
    pub default_bg: CellColor,
    pub cells: Vec<ScreenCell>,
}

impl ScreenSnapshot {
    pub fn cell(&self, col: u16, row: u16) -> ScreenCell {
        let cols = usize::from(self.cols);
        let col = usize::from(col);
        let row = usize::from(row);
        self.cells[row * cols + col]
    }
}
