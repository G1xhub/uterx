//! VTE/ANSI terminal sequence parser.
//!
//! Wraps the `vte` crate to parse incoming byte streams from the PTY
//! and dispatches actions to the terminal grid.

use crate::grid::Grid;

/// Actions that the parser produces for the terminal to execute.
#[derive(Debug, Clone, PartialEq)]
pub enum TerminalAction {
    /// Print a character at the current cursor position.
    Print(char),
    /// Execute a C0/C1 control code (e.g., \n, \r, \t, bell).
    Execute(u8),
    /// CSI sequence (e.g., cursor movement, erase, SGR).
    CsiDispatch {
        params: Vec<u16>,
        intermediates: Vec<u8>,
        final_byte: u8,
    },
    /// ESC sequence.
    EscDispatch {
        intermediates: Vec<u8>,
        final_byte: u8,
    },
    /// OSC sequence (e.g., set title).
    OscDispatch(Vec<Vec<u8>>),
}

/// The terminal parser. Wraps `vte::Parser` and collects actions.
pub struct Parser {
    vte_parser: vte::Parser,
    performer: Performer,
}

/// Internal performer that receives VTE callbacks.
struct Performer {
    actions: Vec<TerminalAction>,
}

impl vte::Perform for Performer {
    fn print(&mut self, c: char) {
        self.actions.push(TerminalAction::Print(c));
    }

    fn execute(&mut self, byte: u8) {
        self.actions.push(TerminalAction::Execute(byte));
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        let param_vec: Vec<u16> = params.iter().flat_map(|p| p.iter().copied()).collect();
        self.actions.push(TerminalAction::CsiDispatch {
            params: param_vec,
            intermediates: intermediates.to_vec(),
            final_byte: action as u8,
        });
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], _ignore: bool, byte: u8) {
        self.actions.push(TerminalAction::EscDispatch {
            intermediates: intermediates.to_vec(),
            final_byte: byte,
        });
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        self.actions
            .push(TerminalAction::OscDispatch(params.iter().map(|p| p.to_vec()).collect()));
    }

    fn hook(&mut self, _params: &vte::Params, _intermediates: &[u8], _ignore: bool, _action: char) {
        // DCS hook — TODO
    }

    fn unhook(&mut self) {
        // DCS unhook — TODO
    }

    fn put(&mut self, _byte: u8) {
        // DCS put — TODO
    }
}

impl Parser {
    pub fn new() -> Self {
        Self {
            vte_parser: vte::Parser::new(),
            performer: Performer {
                actions: Vec::new(),
            },
        }
    }

    /// Feed raw bytes from the PTY into the parser.
    /// Returns the actions to be applied to the grid.
    pub fn advance(&mut self, bytes: &[u8]) -> Vec<TerminalAction> {
        self.performer.actions.clear();
        for &byte in bytes {
            self.vte_parser.advance(&mut self.performer, byte);
        }
        std::mem::take(&mut self.performer.actions)
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

/// Apply parsed terminal actions to a grid.
pub fn apply_actions(grid: &mut Grid, actions: &[TerminalAction]) {
    for action in actions {
        match action {
            TerminalAction::Print(ch) => {
                grid.write_char(*ch);
            }
            TerminalAction::Execute(byte) => match byte {
                // Line feed (LF), Vertical tab (VT), Form feed (FF)
                0x0A | 0x0B | 0x0C => grid.newline(),
                // Carriage return (CR)
                0x0D => grid.cursor_col = 0,
                // Backspace
                0x08 => {
                    grid.cursor_col = grid.cursor_col.saturating_sub(1);
                }
                // Tab
                0x09 => {
                    let next_tab = (grid.cursor_col + 8) & !7;
                    grid.cursor_col = next_tab.min(grid.cols() - 1);
                }
                // Bell — ignore for now
                0x07 => {}
                // Shift Out / Shift In (charset switching) — ignore
                0x0E | 0x0F => {}
                _ => {
                    tracing::trace!("unhandled execute byte: 0x{:02x}", byte);
                }
            },
            TerminalAction::CsiDispatch {
                params,
                intermediates,
                final_byte,
            } => {
                handle_csi(grid, params, intermediates, *final_byte);
            }
            TerminalAction::EscDispatch {
                intermediates,
                final_byte,
            } => {
                handle_esc(grid, intermediates, *final_byte);
            }
            TerminalAction::OscDispatch(params) => {
                handle_osc(grid, params);
            }
        }
    }
}

/// Handle ESC sequences.
fn handle_esc(grid: &mut Grid, intermediates: &[u8], final_byte: u8) {
    match (intermediates, final_byte) {
        // ESC 7 — Save Cursor (DECSC)
        ([], b'7') => grid.save_cursor(),
        // ESC 8 — Restore Cursor (DECRC)
        ([], b'8') => grid.restore_cursor(),
        // ESC M — Reverse Index (move cursor up, scroll down if at top)
        ([], b'M') => {
            if grid.cursor_row == grid.scroll_top {
                grid.scroll_down_region();
            } else if grid.cursor_row > 0 {
                grid.cursor_row -= 1;
            }
        }
        // ESC D — Index (move cursor down, scroll up if at bottom)
        ([], b'D') => {
            if grid.cursor_row == grid.scroll_bottom {
                grid.scroll_up_region();
            } else if grid.cursor_row + 1 < grid.rows() {
                grid.cursor_row += 1;
            }
        }
        // ESC E — Next Line
        ([], b'E') => {
            grid.cursor_col = 0;
            grid.newline();
        }
        // ESC c — Full Reset (RIS)
        ([], b'c') => {
            grid.clear();
            grid.pen = crate::cell::CellAttributes::default();
            grid.scroll_top = 0;
            grid.scroll_bottom = grid.rows().saturating_sub(1);
        }
        _ => {
            tracing::trace!(
                "unhandled ESC: intermediates={:?}, final=0x{:02x} ({:?})",
                intermediates,
                final_byte,
                final_byte as char
            );
        }
    }
}

/// Handle OSC sequences (Operating System Commands).
fn handle_osc(grid: &mut Grid, params: &[Vec<u8>]) {
    if params.is_empty() {
        return;
    }
    let cmd = std::str::from_utf8(&params[0]).unwrap_or("");
    match cmd {
        // OSC 0 ; title ST — Set icon name and window title
        // OSC 2 ; title ST — Set window title
        "0" | "2" => {
            if params.len() > 1 {
                if let Ok(title) = std::str::from_utf8(&params[1]) {
                    grid.title = title.to_string();
                    tracing::debug!("window title: {}", grid.title);
                }
            }
        }
        _ => {
            tracing::trace!("unhandled OSC command: {:?}", cmd);
        }
    }
}

/// Handle a CSI sequence.
fn handle_csi(grid: &mut Grid, params: &[u16], intermediates: &[u8], final_byte: u8) {
    let p = |idx: usize, default: u16| -> u16 {
        params.get(idx).copied().filter(|&v| v != 0).unwrap_or(default)
    };

    // Check for DEC private mode sequences (CSI ? ...)
    if intermediates == [b'?'] {
        handle_dec_mode(grid, params, final_byte);
        return;
    }

    match final_byte {
        // CUU — Cursor Up
        b'A' => {
            let n = p(0, 1) as usize;
            grid.cursor_row = grid.cursor_row.saturating_sub(n);
        }
        // CUD — Cursor Down
        b'B' => {
            let n = p(0, 1) as usize;
            grid.cursor_row = (grid.cursor_row + n).min(grid.rows() - 1);
        }
        // CUF — Cursor Forward
        b'C' => {
            let n = p(0, 1) as usize;
            grid.cursor_col = (grid.cursor_col + n).min(grid.cols() - 1);
        }
        // CUB — Cursor Back
        b'D' => {
            let n = p(0, 1) as usize;
            grid.cursor_col = grid.cursor_col.saturating_sub(n);
        }
        // CNL — Cursor Next Line
        b'E' => {
            let n = p(0, 1) as usize;
            grid.cursor_row = (grid.cursor_row + n).min(grid.rows() - 1);
            grid.cursor_col = 0;
        }
        // CPL — Cursor Previous Line
        b'F' => {
            let n = p(0, 1) as usize;
            grid.cursor_row = grid.cursor_row.saturating_sub(n);
            grid.cursor_col = 0;
        }
        // CHA — Cursor Horizontal Absolute (1-based)
        b'G' => {
            let col = p(0, 1) as usize;
            grid.cursor_col = (col - 1).min(grid.cols() - 1);
        }
        // CUP — Cursor Position (row;col, 1-based)
        b'H' | b'f' => {
            let row = p(0, 1) as usize;
            let col = p(1, 1) as usize;
            grid.cursor_row = (row - 1).min(grid.rows() - 1);
            grid.cursor_col = (col - 1).min(grid.cols() - 1);
        }
        // ED — Erase in Display
        b'J' => {
            let mode = p(0, 0);
            match mode {
                0 => grid.erase_below(),
                1 => grid.erase_above(),
                2 | 3 => grid.clear(),
                _ => {}
            }
        }
        // EL — Erase in Line
        b'K' => {
            let mode = p(0, 0);
            match mode {
                0 => grid.erase_line_right(),
                1 => grid.erase_line_left(),
                2 => grid.erase_line(),
                _ => {}
            }
        }
        // IL — Insert Lines
        b'L' => {
            let n = p(0, 1) as usize;
            grid.insert_lines(n);
        }
        // DL — Delete Lines
        b'M' => {
            let n = p(0, 1) as usize;
            grid.delete_lines(n);
        }
        // DCH — Delete Characters
        b'P' => {
            let n = p(0, 1) as usize;
            let row = grid.cursor_row;
            let col = grid.cursor_col;
            let cols = grid.cols();
            for c in col..(cols - n).min(cols) {
                if c + n < cols {
                    if let (Some(src), Some(_)) = (grid.cell(row, c + n).cloned(), grid.cell(row, c)) {
                        if let Some(dest) = grid.cell_mut(row, c) {
                            *dest = src;
                        }
                    }
                }
            }
            for c in (cols.saturating_sub(n))..cols {
                if let Some(cell) = grid.cell_mut(row, c) {
                    cell.clear();
                }
            }
        }
        // SGR — Select Graphic Rendition
        b'm' => {
            handle_sgr(grid, params);
        }
        // VPA — Vertical Line Position Absolute (1-based row)
        b'd' => {
            let row = p(0, 1) as usize;
            grid.cursor_row = (row - 1).min(grid.rows() - 1);
        }
        // DECSTBM — Set Scrolling Region (top;bottom, 1-based)
        b'r' => {
            let top = p(0, 0) as usize;
            let bottom = p(1, 0) as usize;
            grid.set_scroll_region(top, bottom);
        }
        // ICH — Insert Characters
        b'@' => {
            let n = p(0, 1) as usize;
            let row = grid.cursor_row;
            let col = grid.cursor_col;
            let cols = grid.cols();
            // Shift characters right
            for c in (col..cols.saturating_sub(n)).rev() {
                if let Some(src) = grid.cell(row, c).cloned() {
                    if let Some(dest) = grid.cell_mut(row, c + n) {
                        *dest = src;
                    }
                }
            }
            // Blank the inserted cells
            for c in col..(col + n).min(cols) {
                if let Some(cell) = grid.cell_mut(row, c) {
                    cell.clear();
                }
            }
        }
        // ECH — Erase Characters
        b'X' => {
            let n = p(0, 1) as usize;
            for c in grid.cursor_col..(grid.cursor_col + n).min(grid.cols()) {
                if let Some(cell) = grid.cell_mut(grid.cursor_row, c) {
                    cell.clear();
                }
            }
        }
        // SU — Scroll Up
        b'S' => {
            let n = p(0, 1) as usize;
            for _ in 0..n {
                grid.scroll_up_region();
            }
        }
        // SD — Scroll Down
        b'T' => {
            let n = p(0, 1) as usize;
            for _ in 0..n {
                grid.scroll_down_region();
            }
        }
        // DSR — Device Status Report
        b'n' => {
            // We don't have a writer here, so just log
            tracing::trace!("DSR request: {:?}", params);
        }
        // DECSC / DECRC (via CSI s / CSI u)
        b's' => grid.save_cursor(),
        b'u' => grid.restore_cursor(),
        _ => {
            tracing::trace!(
                "unhandled CSI: intermediates={:?}, params={:?}, final=0x{:02x} ({:?})",
                intermediates,
                params,
                final_byte,
                final_byte as char
            );
        }
    }
}

/// Handle DEC private mode set/reset (CSI ? Ps h/l).
fn handle_dec_mode(grid: &mut Grid, params: &[u16], final_byte: u8) {
    for &param in params {
        match (param, final_byte) {
            // DECSET 1049 — Enter alternate screen buffer
            (1049, b'h') => grid.enter_alt_screen(),
            // DECRST 1049 — Leave alternate screen buffer
            (1049, b'l') => grid.leave_alt_screen(),
            // DECSET 1048 — Save cursor
            (1048, b'h') => grid.save_cursor(),
            // DECRST 1048 — Restore cursor
            (1048, b'l') => grid.restore_cursor(),
            // DECSET 1 — Application cursor keys (we send the same sequences for now)
            (1, b'h' | b'l') => {}
            // DECSET 12 — Start/stop blinking cursor (ignore)
            (12, b'h' | b'l') => {}
            // DECSET 25 — Show/hide cursor (we always show — TODO)
            (25, b'h' | b'l') => {}
            // DECSET 7 — Auto wrap mode
            (7, b'h' | b'l') => {
                // TODO: track auto wrap state
            }
            // DECSET 47 / 1047 — alternate screen (simpler variant)
            (47 | 1047, b'h') => grid.enter_alt_screen(),
            (47 | 1047, b'l') => grid.leave_alt_screen(),
            // DECSET 2004 — Bracketed paste mode (ignore for now)
            (2004, b'h' | b'l') => {}
            _ => {
                tracing::trace!("unhandled DEC mode: param={}, final=0x{:02x}", param, final_byte);
            }
        }
    }
}

/// Handle SGR (Select Graphic Rendition) — CSI Ps m
fn handle_sgr(grid: &mut Grid, params: &[u16]) {
    use crate::cell::Color;

    // If no params, treat as reset (SGR 0)
    if params.is_empty() {
        grid.pen = crate::cell::CellAttributes::default();
        return;
    }

    let mut i = 0;
    while i < params.len() {
        match params[i] {
            // Reset
            0 => grid.pen = crate::cell::CellAttributes::default(),
            // Bold
            1 => grid.pen.bold = true,
            // Dim/faint — we map to non-bold for now
            2 => grid.pen.bold = false,
            // Italic
            3 => grid.pen.italic = true,
            // Underline
            4 => grid.pen.underline = true,
            // Blink (slow) — ignore visual, just accept
            5 | 6 => {}
            // Inverse
            7 => grid.pen.inverse = true,
            // Hidden
            8 => grid.pen.hidden = true,
            // Strikethrough
            9 => grid.pen.strikethrough = true,
            // Normal intensity (neither bold nor faint)
            22 => grid.pen.bold = false,
            // Not italic
            23 => grid.pen.italic = false,
            // Not underlined
            24 => grid.pen.underline = false,
            // Not blinking — ignore
            25 => {}
            // Not inverse
            27 => grid.pen.inverse = false,
            // Not hidden
            28 => grid.pen.hidden = false,
            // Not strikethrough
            29 => grid.pen.strikethrough = false,
            // Standard foreground colors (30-37)
            c @ 30..=37 => grid.pen.fg = Color::Indexed(c as u8 - 30),
            // Extended foreground color
            38 => {
                i += 1;
                if i < params.len() {
                    match params[i] {
                        // 256-color: 38;5;n
                        5 => {
                            i += 1;
                            if i < params.len() {
                                grid.pen.fg = Color::Indexed(params[i] as u8);
                            }
                        }
                        // RGB: 38;2;r;g;b
                        2 => {
                            if i + 3 < params.len() {
                                let r = params[i + 1] as u8;
                                let g = params[i + 2] as u8;
                                let b = params[i + 3] as u8;
                                grid.pen.fg = Color::Rgb(r, g, b);
                                i += 3;
                            }
                        }
                        _ => {}
                    }
                }
            }
            // Default foreground
            39 => grid.pen.fg = Color::Default,
            // Standard background colors (40-47)
            c @ 40..=47 => grid.pen.bg = Color::Indexed(c as u8 - 40),
            // Extended background color
            48 => {
                i += 1;
                if i < params.len() {
                    match params[i] {
                        // 256-color: 48;5;n
                        5 => {
                            i += 1;
                            if i < params.len() {
                                grid.pen.bg = Color::Indexed(params[i] as u8);
                            }
                        }
                        // RGB: 48;2;r;g;b
                        2 => {
                            if i + 3 < params.len() {
                                let r = params[i + 1] as u8;
                                let g = params[i + 2] as u8;
                                let b = params[i + 3] as u8;
                                grid.pen.bg = Color::Rgb(r, g, b);
                                i += 3;
                            }
                        }
                        _ => {}
                    }
                }
            }
            // Default background
            49 => grid.pen.bg = Color::Default,
            // Bright foreground colors (90-97)
            c @ 90..=97 => grid.pen.fg = Color::Indexed(c as u8 - 90 + 8),
            // Bright background colors (100-107)
            c @ 100..=107 => grid.pen.bg = Color::Indexed(c as u8 - 100 + 8),
            _ => {
                tracing::trace!("unhandled SGR param: {}", params[i]);
            }
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Color;

    #[test]
    fn test_sgr_bold_fg() {
        let mut grid = Grid::new(80, 24);
        // SGR 1;31 = bold + red fg
        let actions = vec![TerminalAction::CsiDispatch {
            params: vec![1, 31],
            intermediates: vec![],
            final_byte: b'm',
        }];
        apply_actions(&mut grid, &actions);
        assert!(grid.pen.bold);
        assert_eq!(grid.pen.fg, Color::Indexed(1)); // red
    }

    #[test]
    fn test_sgr_256_color() {
        let mut grid = Grid::new(80, 24);
        // SGR 38;5;202 = fg color 202
        let actions = vec![TerminalAction::CsiDispatch {
            params: vec![38, 5, 202],
            intermediates: vec![],
            final_byte: b'm',
        }];
        apply_actions(&mut grid, &actions);
        assert_eq!(grid.pen.fg, Color::Indexed(202));
    }

    #[test]
    fn test_sgr_rgb() {
        let mut grid = Grid::new(80, 24);
        // SGR 38;2;100;150;200 = fg RGB(100, 150, 200)
        let actions = vec![TerminalAction::CsiDispatch {
            params: vec![38, 2, 100, 150, 200],
            intermediates: vec![],
            final_byte: b'm',
        }];
        apply_actions(&mut grid, &actions);
        assert_eq!(grid.pen.fg, Color::Rgb(100, 150, 200));
    }

    #[test]
    fn test_sgr_reset() {
        let mut grid = Grid::new(80, 24);
        grid.pen.bold = true;
        grid.pen.fg = Color::Indexed(1);
        let actions = vec![TerminalAction::CsiDispatch {
            params: vec![0],
            intermediates: vec![],
            final_byte: b'm',
        }];
        apply_actions(&mut grid, &actions);
        assert!(!grid.pen.bold);
        assert_eq!(grid.pen.fg, Color::Default);
    }

    #[test]
    fn test_erase_in_line() {
        let mut grid = Grid::new(10, 1);
        for ch in "ABCDEFGHIJ".chars() {
            grid.write_char(ch);
        }
        grid.cursor_col = 5;
        // EL 0 — erase to right
        let actions = vec![TerminalAction::CsiDispatch {
            params: vec![0],
            intermediates: vec![],
            final_byte: b'K',
        }];
        apply_actions(&mut grid, &actions);
        assert_eq!(grid.cell(0, 4).unwrap().content, "E");
        assert_eq!(grid.cell(0, 5).unwrap().content, " ");
        assert_eq!(grid.cell(0, 9).unwrap().content, " ");
    }

    #[test]
    fn test_alt_screen_via_csi() {
        let mut grid = Grid::new(80, 24);
        grid.write_char('Z');
        // DECSET 1049 — enter alt screen
        let actions = vec![TerminalAction::CsiDispatch {
            params: vec![1049],
            intermediates: vec![b'?'],
            final_byte: b'h',
        }];
        apply_actions(&mut grid, &actions);
        assert!(grid.is_alt_screen());
        // DECRST 1049 — leave alt screen
        let actions = vec![TerminalAction::CsiDispatch {
            params: vec![1049],
            intermediates: vec![b'?'],
            final_byte: b'l',
        }];
        apply_actions(&mut grid, &actions);
        assert!(!grid.is_alt_screen());
        assert_eq!(grid.cell(0, 0).unwrap().content, "Z");
    }
}
