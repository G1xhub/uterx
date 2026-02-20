//! Terminal grid: a 2D array of cells representing the visible terminal area.

use crate::cell::{Cell, CellAttributes};
use crate::scrollback::Scrollback;

/// The terminal grid holds rows × cols of cells.
#[derive(Debug, Clone)]
pub struct Grid {
    cols: usize,
    rows: usize,
    cells: Vec<Vec<Cell>>,
    /// Cursor position (0-indexed).
    pub cursor_row: usize,
    pub cursor_col: usize,
    /// Current pen attributes applied to newly written cells.
    pub pen: CellAttributes,
    /// Scrollback buffer for lines scrolled off-screen.
    pub scrollback: Scrollback,
    /// Scroll region top (inclusive, 0-indexed).
    pub scroll_top: usize,
    /// Scroll region bottom (inclusive, 0-indexed).
    pub scroll_bottom: usize,
    /// Saved cursor position (for ESC 7 / ESC 8, DECSC/DECRC).
    pub saved_cursor: Option<(usize, usize, CellAttributes)>,
    /// Alternate screen buffer (for DECSET 1049).
    alt_cells: Option<Vec<Vec<Cell>>>,
    alt_cursor: Option<(usize, usize)>,
    /// Window title set via OSC.
    pub title: String,
}

impl Grid {
    /// Create a new grid with the given dimensions, filled with blank cells.
    pub fn new(cols: usize, rows: usize) -> Self {
        let cells = (0..rows)
            .map(|_| (0..cols).map(|_| Cell::default()).collect())
            .collect();
        Self {
            cols,
            rows,
            cells,
            cursor_row: 0,
            cursor_col: 0,
            pen: CellAttributes::default(),
            scrollback: Scrollback::default(),
            scroll_top: 0,
            scroll_bottom: rows.saturating_sub(1),
            saved_cursor: None,
            alt_cells: None,
            alt_cursor: None,
            title: String::new(),
        }
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Whether we're in the alternate screen buffer.
    pub fn is_alt_screen(&self) -> bool {
        self.alt_cells.is_some()
    }

    /// Access a cell at (row, col). Returns `None` if out of bounds.
    pub fn cell(&self, row: usize, col: usize) -> Option<&Cell> {
        self.cells.get(row)?.get(col)
    }

    /// Mutably access a cell at (row, col).
    pub fn cell_mut(&mut self, row: usize, col: usize) -> Option<&mut Cell> {
        self.cells.get_mut(row)?.get_mut(col)
    }

    /// Get an entire row.
    pub fn row(&self, row: usize) -> Option<&[Cell]> {
        self.cells.get(row).map(|r| r.as_slice())
    }

    /// Write a character at the current cursor position and advance the cursor.
    /// Applies the current pen attributes.
    pub fn write_char(&mut self, ch: char) {
        if self.cursor_col >= self.cols {
            self.cursor_col = 0;
            self.newline();
        }
        let pen = self.pen;
        if let Some(cell) = self.cell_mut(self.cursor_row, self.cursor_col) {
            cell.content.clear();
            cell.content.push(ch);
            cell.attrs = pen;
        }
        self.cursor_col += 1;
    }

    /// Move cursor to the next line, scrolling if necessary.
    pub fn newline(&mut self) {
        if self.cursor_row == self.scroll_bottom {
            self.scroll_up_region();
        } else if self.cursor_row + 1 < self.rows {
            self.cursor_row += 1;
        }
    }

    /// Scroll the scroll region up by one line.
    /// The top line of the region goes to scrollback (if in main screen
    /// and the scroll region starts at 0).
    pub fn scroll_up_region(&mut self) {
        if self.scroll_top > self.scroll_bottom {
            return;
        }
        // Evict the top row of the scroll region
        let evicted = self.cells[self.scroll_top].clone();
        // Only push to scrollback on the main screen when scrolling the full screen
        if self.alt_cells.is_none() && self.scroll_top == 0 {
            self.scrollback.push(evicted);
        }
        // Shift rows up within the scroll region
        for r in self.scroll_top..self.scroll_bottom {
            self.cells[r] = self.cells[r + 1].clone();
        }
        // New blank line at the bottom of the scroll region
        self.cells[self.scroll_bottom] = (0..self.cols).map(|_| Cell::default()).collect();
    }

    /// Scroll the scroll region down by one line.
    pub fn scroll_down_region(&mut self) {
        if self.scroll_top > self.scroll_bottom {
            return;
        }
        for r in (self.scroll_top + 1..=self.scroll_bottom).rev() {
            self.cells[r] = self.cells[r - 1].clone();
        }
        self.cells[self.scroll_top] = (0..self.cols).map(|_| Cell::default()).collect();
    }

    /// Legacy scroll_up (full screen) — kept for compatibility.
    pub fn scroll_up(&mut self) {
        self.scroll_up_region();
    }

    /// Resize the grid. Content is best-effort preserved.
    pub fn resize(&mut self, new_cols: usize, new_rows: usize) {
        // Adjust rows
        self.cells.resize_with(new_rows, || {
            (0..new_cols).map(|_| Cell::default()).collect()
        });
        // Adjust columns in each row
        for row in &mut self.cells {
            row.resize_with(new_cols, Cell::default);
        }
        self.cols = new_cols;
        self.rows = new_rows;
        // Reset scroll region to full screen
        self.scroll_top = 0;
        self.scroll_bottom = new_rows.saturating_sub(1);
        // Clamp cursor
        self.cursor_row = self.cursor_row.min(new_rows.saturating_sub(1));
        self.cursor_col = self.cursor_col.min(new_cols.saturating_sub(1));
    }

    /// Clear the entire grid.
    pub fn clear(&mut self) {
        for row in &mut self.cells {
            for cell in row {
                cell.clear();
            }
        }
        self.cursor_row = 0;
        self.cursor_col = 0;
    }

    // --- Erase operations ---

    /// ED mode 0: Erase from cursor to end of display.
    pub fn erase_below(&mut self) {
        // Clear from cursor to end of current row
        for col in self.cursor_col..self.cols {
            if let Some(cell) = self.cell_mut(self.cursor_row, col) {
                cell.clear();
            }
        }
        // Clear all rows below
        for r in (self.cursor_row + 1)..self.rows {
            for c in 0..self.cols {
                if let Some(cell) = self.cell_mut(r, c) {
                    cell.clear();
                }
            }
        }
    }

    /// ED mode 1: Erase from beginning of display to cursor.
    pub fn erase_above(&mut self) {
        // Clear all rows above
        for r in 0..self.cursor_row {
            for c in 0..self.cols {
                if let Some(cell) = self.cell_mut(r, c) {
                    cell.clear();
                }
            }
        }
        // Clear from beginning of current row to cursor (inclusive)
        for col in 0..=self.cursor_col.min(self.cols.saturating_sub(1)) {
            if let Some(cell) = self.cell_mut(self.cursor_row, col) {
                cell.clear();
            }
        }
    }

    /// EL mode 0: Erase from cursor to end of line.
    pub fn erase_line_right(&mut self) {
        for col in self.cursor_col..self.cols {
            if let Some(cell) = self.cell_mut(self.cursor_row, col) {
                cell.clear();
            }
        }
    }

    /// EL mode 1: Erase from beginning of line to cursor.
    pub fn erase_line_left(&mut self) {
        for col in 0..=self.cursor_col.min(self.cols.saturating_sub(1)) {
            if let Some(cell) = self.cell_mut(self.cursor_row, col) {
                cell.clear();
            }
        }
    }

    /// EL mode 2: Erase entire line.
    pub fn erase_line(&mut self) {
        for col in 0..self.cols {
            if let Some(cell) = self.cell_mut(self.cursor_row, col) {
                cell.clear();
            }
        }
    }

    // --- Insert / Delete Lines ---

    /// Insert `n` blank lines at cursor row (within scroll region), pushing content down.
    pub fn insert_lines(&mut self, n: usize) {
        let top = self.cursor_row.max(self.scroll_top);
        let bottom = self.scroll_bottom;
        if top > bottom {
            return;
        }
        for _ in 0..n {
            if bottom < self.cells.len() {
                self.cells.remove(bottom);
            }
            let blank_row: Vec<Cell> = (0..self.cols).map(|_| Cell::default()).collect();
            self.cells.insert(top, blank_row);
        }
    }

    /// Delete `n` lines at cursor row (within scroll region), pulling content up.
    pub fn delete_lines(&mut self, n: usize) {
        let top = self.cursor_row.max(self.scroll_top);
        let bottom = self.scroll_bottom;
        if top > bottom {
            return;
        }
        for _ in 0..n {
            if top < self.cells.len() {
                self.cells.remove(top);
            }
            let blank_row: Vec<Cell> = (0..self.cols).map(|_| Cell::default()).collect();
            if bottom < self.cells.len() {
                self.cells.insert(bottom, blank_row);
            } else {
                self.cells.push(blank_row);
            }
        }
    }

    // --- Alternate screen buffer ---

    /// Enter alternate screen buffer (DECSET 1049).
    pub fn enter_alt_screen(&mut self) {
        if self.alt_cells.is_some() {
            return; // already in alt screen
        }
        // Save main screen
        self.alt_cells = Some(std::mem::replace(
            &mut self.cells,
            (0..self.rows)
                .map(|_| (0..self.cols).map(|_| Cell::default()).collect())
                .collect(),
        ));
        self.alt_cursor = Some((self.cursor_row, self.cursor_col));
        self.cursor_row = 0;
        self.cursor_col = 0;
    }

    /// Leave alternate screen buffer (DECRST 1049).
    pub fn leave_alt_screen(&mut self) {
        if let Some(main_cells) = self.alt_cells.take() {
            self.cells = main_cells;
            if let Some((r, c)) = self.alt_cursor.take() {
                self.cursor_row = r.min(self.rows.saturating_sub(1));
                self.cursor_col = c.min(self.cols.saturating_sub(1));
            }
        }
    }

    // --- Cursor save/restore ---

    /// Save cursor position and attributes (DECSC / ESC 7).
    pub fn save_cursor(&mut self) {
        self.saved_cursor = Some((self.cursor_row, self.cursor_col, self.pen));
    }

    /// Restore cursor position and attributes (DECRC / ESC 8).
    pub fn restore_cursor(&mut self) {
        if let Some((r, c, pen)) = self.saved_cursor {
            self.cursor_row = r.min(self.rows.saturating_sub(1));
            self.cursor_col = c.min(self.cols.saturating_sub(1));
            self.pen = pen;
        }
    }

    // --- Scroll region ---

    /// Set scroll region (DECSTBM). top and bottom are 1-based (as in CSI params).
    /// Passing (0, 0) resets to full screen.
    pub fn set_scroll_region(&mut self, top: usize, bottom: usize) {
        let t = if top == 0 { 0 } else { (top - 1).min(self.rows - 1) };
        let b = if bottom == 0 {
            self.rows - 1
        } else {
            (bottom - 1).min(self.rows - 1)
        };
        if t < b {
            self.scroll_top = t;
            self.scroll_bottom = b;
        } else {
            self.scroll_top = 0;
            self.scroll_bottom = self.rows.saturating_sub(1);
        }
        // DECSTBM moves cursor to home
        self.cursor_row = self.scroll_top;
        self.cursor_col = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_grid() {
        let grid = Grid::new(80, 24);
        assert_eq!(grid.cols(), 80);
        assert_eq!(grid.rows(), 24);
        assert_eq!(grid.cursor_row, 0);
        assert_eq!(grid.cursor_col, 0);
    }

    #[test]
    fn test_write_char() {
        let mut grid = Grid::new(80, 24);
        grid.write_char('A');
        assert_eq!(grid.cell(0, 0).unwrap().content, "A");
        assert_eq!(grid.cursor_col, 1);
    }

    #[test]
    fn test_resize() {
        let mut grid = Grid::new(80, 24);
        grid.resize(40, 12);
        assert_eq!(grid.cols(), 40);
        assert_eq!(grid.rows(), 12);
    }

    #[test]
    fn test_scrollback_on_scroll() {
        let mut grid = Grid::new(80, 3);
        grid.write_char('A');
        grid.newline();
        grid.write_char('B');
        grid.newline();
        grid.write_char('C');
        grid.newline(); // This should scroll 'A' into scrollback
        assert_eq!(grid.scrollback.len(), 1);
        assert_eq!(grid.scrollback.line(0).unwrap()[0].content, "A");
    }

    #[test]
    fn test_erase_below() {
        let mut grid = Grid::new(10, 5);
        for ch in "ABCDEFGHIJ".chars() {
            grid.write_char(ch);
        }
        grid.cursor_row = 0;
        grid.cursor_col = 5;
        grid.erase_below();
        // Cells 0..5 on row 0 should still have content
        assert_eq!(grid.cell(0, 0).unwrap().content, "A");
        assert_eq!(grid.cell(0, 4).unwrap().content, "E");
        // Cell 5 onwards should be cleared
        assert_eq!(grid.cell(0, 5).unwrap().content, " ");
    }

    #[test]
    fn test_alt_screen() {
        let mut grid = Grid::new(80, 24);
        grid.write_char('X');
        grid.enter_alt_screen();
        assert!(grid.is_alt_screen());
        assert_eq!(grid.cell(0, 0).unwrap().content, " "); // alt screen is blank
        grid.write_char('Y');
        grid.leave_alt_screen();
        assert!(!grid.is_alt_screen());
        assert_eq!(grid.cell(0, 0).unwrap().content, "X"); // main screen restored
    }

    #[test]
    fn test_scroll_region() {
        let mut grid = Grid::new(10, 5);
        grid.set_scroll_region(2, 4); // rows 1..3 (0-indexed)
        assert_eq!(grid.scroll_top, 1);
        assert_eq!(grid.scroll_bottom, 3);
    }
}
