use crossbeam_channel::{Receiver, Sender};
use thiserror::Error;

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::grid::Scroll;
use alacritty_terminal::index::{Column, Direction, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::{Config as TermConfig, RenderableContent, Term};
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::term::search::{Match, RegexSearch};
use alacritty_terminal::vte::ansi::Processor;

use crate::pty::{PtyDimensions, PtyError, PtyEvent, PtySession};
use crate::screen::{CellColor, ScreenCell, ScreenSnapshot};

#[derive(Debug, Error)]
pub enum TerminalSessionError {
    #[error("pty error")]
    Pty(#[from] PtyError),
}

#[derive(Debug, Clone, Copy)]
struct TermDimensions {
    cols: usize,
    rows: usize,
}

impl Dimensions for TermDimensions {
    fn total_lines(&self) -> usize {
        self.rows
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.cols
    }
}

#[derive(Clone)]
struct TermEventProxy {
    tx: Sender<Event>,
}

impl EventListener for TermEventProxy {
    fn send_event(&self, event: Event) {
        let _ = self.tx.send(event);
    }
}

pub struct TerminalSession {
    pty: PtySession,
    pty_rx: Receiver<PtyEvent>,
    processor: Processor,
    term: Term<TermEventProxy>,
    term_event_rx: Receiver<Event>,
    cell_width_px: u16,
    cell_height_px: u16,
    exited: bool,
    exit_code: Option<i32>,
}

impl TerminalSession {
    pub fn spawn_shell(
        dimensions: PtyDimensions,
        scrollback_lines: usize,
    ) -> Result<Self, TerminalSessionError> {
        let (pty, pty_rx) = PtySession::spawn_shell(dimensions)?;

        let mut config = TermConfig::default();
        config.scrolling_history = scrollback_lines;

        let term_dimensions = TermDimensions {
            cols: usize::from(dimensions.cols),
            rows: usize::from(dimensions.rows),
        };

        let (term_event_tx, term_event_rx) = crossbeam_channel::unbounded();
        let term = Term::new(
            config,
            &term_dimensions,
            TermEventProxy { tx: term_event_tx },
        );

        Ok(Self {
            pty,
            pty_rx,
            processor: Processor::default(),
            term,
            term_event_rx,
            // Reasonable defaults until we have real font metrics.
            cell_width_px: 8,
            cell_height_px: 16,
            exited: false,
            exit_code: None,
        })
    }

    pub fn poll_pty(&mut self) -> bool {
        let mut changed = false;
        while let Ok(event) = self.pty_rx.try_recv() {
            match event {
                PtyEvent::Output(bytes) => {
                    self.processor.advance(&mut self.term, &bytes);
                    self.drain_term_events();
                    changed = true;
                }
                PtyEvent::Exited(code) => {
                    self.exited = true;
                    self.exit_code = Some(code);
                }
            }
        }
        self.drain_term_events();
        changed
    }

    pub fn is_exited(&self) -> bool {
        self.exited
    }

    pub fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }

    pub fn write_bytes(&self, bytes: &[u8]) -> Result<(), PtyError> {
        self.pty.write_all(bytes)
    }

    pub fn resize(&mut self, dimensions: PtyDimensions) -> Result<(), TerminalSessionError> {
        self.pty.resize(dimensions)?;

        let term_dimensions = TermDimensions {
            cols: usize::from(dimensions.cols),
            rows: usize::from(dimensions.rows),
        };
        self.term.resize(term_dimensions);
        Ok(())
    }

    pub fn renderable_content(&self) -> RenderableContent<'_> {
        self.term.renderable_content()
    }

    pub fn display_offset(&self) -> usize {
        self.term.grid().display_offset()
    }

    pub fn cursor_point(&self) -> Point {
        self.term.renderable_content().cursor.point
    }

    pub fn clear_selection(&mut self) {
        self.term.selection = None;
    }

    pub fn set_selection_viewport(
        &mut self,
        start_col: u16,
        start_row: u16,
        end_col: u16,
        end_row: u16,
    ) {
        let max_col = self.term.columns().saturating_sub(1) as u16;
        let max_row = self.term.screen_lines().saturating_sub(1) as u16;

        let start_col = start_col.min(max_col);
        let start_row = start_row.min(max_row);
        let end_col = end_col.min(max_col);
        let end_row = end_row.min(max_row);

        let display_offset = self.term.grid().display_offset() as i32;
        let start = Point::new(
            Line(i32::from(start_row) - display_offset),
            Column(usize::from(start_col)),
        );
        let end = Point::new(
            Line(i32::from(end_row) - display_offset),
            Column(usize::from(end_col)),
        );

        let (start_side, end_side) = if start <= end {
            (Side::Left, Side::Right)
        } else {
            (Side::Right, Side::Left)
        };

        let mut selection = Selection::new(SelectionType::Simple, start, start_side);
        selection.update(end, end_side);
        self.term.selection = Some(selection);
    }

    pub fn selection_text(&self) -> Option<String> {
        self.term.selection_to_string()
    }

    pub fn scroll_to_display_offset(&mut self, offset: usize) {
        let current = self.term.grid().display_offset();
        if current == offset {
            return;
        }

        let delta = offset as i32 - current as i32;
        self.term.scroll_display(Scroll::Delta(delta));
    }

    pub fn scroll_display(&mut self, scroll: Scroll) -> bool {
        let before = self.term.grid().display_offset();
        self.term.scroll_display(scroll);
        before != self.term.grid().display_offset()
    }

    pub fn search_next(
        &self,
        regex: &mut RegexSearch,
        origin: Point,
        direction: Direction,
        side: Direction,
        max_lines: Option<usize>,
    ) -> Option<Match> {
        self.term.search_next(regex, origin, direction, side, max_lines)
    }

    pub fn snapshot(&self) -> ScreenSnapshot {
        let content = self.term.renderable_content();

        let default_fg = resolve_default_color(alacritty_terminal::vte::ansi::NamedColor::Foreground, content.colors);
        let default_bg = resolve_default_color(alacritty_terminal::vte::ansi::NamedColor::Background, content.colors);

        // Be conservative: always allocate a full viewport buffer.
        let cols = self.term.columns() as u16;
        let rows = self.term.screen_lines() as u16;
        let default_cell = ScreenCell {
            ch: ' ',
            fg: CellColor::DefaultForeground,
            bg: CellColor::DefaultBackground,
            selected: false,
        };
        let mut cells = vec![default_cell; usize::from(cols) * usize::from(rows)];

        let selection = content.selection;
        let cursor_point = content.cursor.point;
        let cursor_shape = content.cursor.shape;

        for indexed in content.display_iter {
            let point = indexed.point;
            let row = point.line.0 + content.display_offset as i32;
            let col = point.column.0 as i32;
            if row < 0 || col < 0 {
                continue;
            }

            let (row, col) = (row as u16, col as u16);
            if row >= rows || col >= cols {
                continue;
            }

            let idx = usize::from(row) * usize::from(cols) + usize::from(col);
            let mut fg = cell_color(indexed.cell.fg, content.colors);
            let mut bg = cell_color(indexed.cell.bg, content.colors);
            if indexed.cell.flags.contains(alacritty_terminal::term::cell::Flags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }
            let selected = selection.is_some_and(|sel| sel.contains_cell(&indexed, cursor_point, cursor_shape));
            cells[idx] = ScreenCell {
                ch: indexed.cell.c,
                fg,
                bg,
                selected,
            };
        }

        let cursor = content.cursor.point;
        let cursor_visible = content.display_offset == 0;
        let cursor_row = (cursor.line.0 + content.display_offset as i32).max(0) as u16;
        let cursor_col = cursor.column.0 as u16;

        ScreenSnapshot {
            cols,
            rows,
            cursor_col,
            cursor_row,
            cursor_visible,
            default_fg,
            default_bg,
            cells,
        }
    }

    fn drain_term_events(&mut self) {
        while let Ok(event) = self.term_event_rx.try_recv() {
            match event {
                Event::PtyWrite(text) => {
                    let _ = self.pty.write_all(text.as_bytes());
                }
                Event::TextAreaSizeRequest(formatter) => {
                    let window_size = WindowSize {
                        num_lines: self.term.screen_lines() as u16,
                        num_cols: self.term.columns() as u16,
                        cell_width: self.cell_width_px,
                        cell_height: self.cell_height_px,
                    };
                    let text = formatter(window_size);
                    let _ = self.pty.write_all(text.as_bytes());
                }
                Event::ColorRequest(index, formatter) => {
                    let colors = self.term.renderable_content().colors;
                    let rgb = if index < alacritty_terminal::term::color::COUNT {
                        colors[index].unwrap_or_else(|| fallback_color(index))
                    } else {
                        fallback_color(index)
                    };
                    let response = formatter(rgb);
                    let _ = self.pty.write_all(response.as_bytes());
                }
                // TODO: wire up clipboard store/load via `tuna_platform`.
                _ => {}
            }
        }
    }
}

fn fallback_color(index: usize) -> alacritty_terminal::vte::ansi::Rgb {
    use alacritty_terminal::vte::ansi::Rgb;

    match index {
        0..=255 => {
            let (r, g, b) = xterm_256_rgb(index as u8);
            Rgb { r, g, b }
        }
        // Foreground / background defaults, in case the palette doesn't specify them.
        256 => Rgb { r: 0xe5, g: 0xe5, b: 0xe5 },
        257 => Rgb { r: 0x10, g: 0x10, b: 0x12 },
        _ => Rgb::default(),
    }
}

fn xterm_256_rgb(index: u8) -> (u8, u8, u8) {
    match index {
        0 => (0x00, 0x00, 0x00),
        1 => (0xcd, 0x00, 0x00),
        2 => (0x00, 0xcd, 0x00),
        3 => (0xcd, 0xcd, 0x00),
        4 => (0x00, 0x00, 0xee),
        5 => (0xcd, 0x00, 0xcd),
        6 => (0x00, 0xcd, 0xcd),
        7 => (0xe5, 0xe5, 0xe5),
        8 => (0x7f, 0x7f, 0x7f),
        9 => (0xff, 0x00, 0x00),
        10 => (0x00, 0xff, 0x00),
        11 => (0xff, 0xff, 0x00),
        12 => (0x5c, 0x5c, 0xff),
        13 => (0xff, 0x00, 0xff),
        14 => (0x00, 0xff, 0xff),
        15 => (0xff, 0xff, 0xff),
        16..=231 => {
            let idx = index - 16;
            let r = idx / 36;
            let g = (idx % 36) / 6;
            let b = idx % 6;
            (cube_component(r), cube_component(g), cube_component(b))
        }
        232..=255 => {
            let gray = 8 + 10 * (index - 232);
            (gray, gray, gray)
        }
    }
}

fn cube_component(value: u8) -> u8 {
    match value {
        0 => 0,
        1 => 95,
        2 => 135,
        3 => 175,
        4 => 215,
        5 => 255,
        _ => 0,
    }
}

fn resolve_default_color(named: alacritty_terminal::vte::ansi::NamedColor, colors: &Colors) -> CellColor {
    colors[named]
        .map(|rgb| CellColor::Rgb {
            r: rgb.r,
            g: rgb.g,
            b: rgb.b,
        })
        .unwrap_or_else(|| match named {
            alacritty_terminal::vte::ansi::NamedColor::Foreground => CellColor::DefaultForeground,
            alacritty_terminal::vte::ansi::NamedColor::Background => CellColor::DefaultBackground,
            _ => CellColor::DefaultForeground,
        })
}

fn cell_color(color: alacritty_terminal::vte::ansi::Color, colors: &Colors) -> CellColor {
    use alacritty_terminal::vte::ansi::{Color, NamedColor};

    match color {
        Color::Named(named) => match named {
            NamedColor::Foreground => CellColor::DefaultForeground,
            NamedColor::Background => CellColor::DefaultBackground,
            _ => resolve_named_color(named, colors),
        },
        Color::Spec(rgb) => CellColor::Rgb {
            r: rgb.r,
            g: rgb.g,
            b: rgb.b,
        },
        Color::Indexed(index) => {
            colors[usize::from(index)]
                .map(|rgb| CellColor::Rgb {
                    r: rgb.r,
                    g: rgb.g,
                    b: rgb.b,
                })
                .unwrap_or(CellColor::Ansi256(index))
        }
    }
}

fn resolve_named_color(named: alacritty_terminal::vte::ansi::NamedColor, colors: &Colors) -> CellColor {
    use alacritty_terminal::vte::ansi::NamedColor;

    if let Some(rgb) = colors[named] {
        return CellColor::Rgb {
            r: rgb.r,
            g: rgb.g,
            b: rgb.b,
        };
    }

    match named {
        NamedColor::DimBlack => CellColor::Ansi256(0),
        NamedColor::DimRed => CellColor::Ansi256(1),
        NamedColor::DimGreen => CellColor::Ansi256(2),
        NamedColor::DimYellow => CellColor::Ansi256(3),
        NamedColor::DimBlue => CellColor::Ansi256(4),
        NamedColor::DimMagenta => CellColor::Ansi256(5),
        NamedColor::DimCyan => CellColor::Ansi256(6),
        NamedColor::DimWhite => CellColor::Ansi256(7),
        _ if (named as u16) <= 15 => CellColor::Ansi256(named as u8),
        _ => CellColor::DefaultForeground,
    }
}
