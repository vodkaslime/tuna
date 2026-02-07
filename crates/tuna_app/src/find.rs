use bevy::prelude::Resource;
use tuna_terminal_core::search::{Direction, Match, RegexSearch};
use tuna_terminal_core::session::TerminalSession;

#[derive(Resource, Debug, Default)]
pub struct FindState {
    pub visible: bool,
    pub query: String,
    pub regex: Option<RegexSearch>,
    pub current_match: Option<Match>,
}

impl FindState {
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if !self.visible {
            self.query.clear();
            self.regex = None;
            self.current_match = None;
        }
    }

    pub fn push_str(&mut self, text: &str) {
        self.query.push_str(text);
        self.rebuild_regex();
    }

    pub fn backspace(&mut self) {
        self.query.pop();
        self.rebuild_regex();
    }

    pub fn clear_match(&mut self) {
        self.current_match = None;
    }

    pub fn update_incremental(&mut self, session: &mut TerminalSession) {
        if !self.visible {
            return;
        }

        let Some(regex) = self.regex.as_mut() else {
            self.current_match = None;
            return;
        };

        let origin = session.cursor_point();
        if let Some(regex_match) = session.search_next(regex, origin, Direction::Right, Direction::Right, None) {
            scroll_match_into_view(session, &regex_match);
            self.current_match = Some(regex_match);
        } else {
            self.current_match = None;
        }
    }

    pub fn find_again(&mut self, session: &mut TerminalSession, direction: Direction) {
        let Some(regex) = self.regex.as_mut() else {
            return;
        };

        let origin = match (&self.current_match, direction) {
            (Some(m), Direction::Right) => *m.end(),
            (Some(m), Direction::Left) => *m.start(),
            (None, _) => session.cursor_point(),
        };

        // Choose the opposite match side to avoid repeatedly selecting the same match when the
        // origin sits on the current match boundary.
        let side = match direction {
            Direction::Right => Direction::Left,
            Direction::Left => Direction::Right,
        };

        if let Some(regex_match) = session.search_next(regex, origin, direction, side, None) {
            scroll_match_into_view(session, &regex_match);
            self.current_match = Some(regex_match);
        }
    }

    fn rebuild_regex(&mut self) {
        if self.query.is_empty() {
            self.regex = None;
            self.current_match = None;
            return;
        }

        // Treat the query as a literal string for now (regex toggle comes later).
        let pattern = escape_regex_literal(&self.query);
        self.regex = RegexSearch::new(&pattern).ok();
    }
}

fn escape_regex_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if matches!(
            ch,
            '.' | '^' | '$' | '|' | '?' | '*' | '+' | '(' | ')' | '[' | ']' | '{' | '}' | '\\'
        ) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

fn scroll_match_into_view(session: &mut TerminalSession, m: &Match) {
    let line = m.start().line.0;
    let desired = if line < 0 { (-line) as usize } else { 0 };
    session.scroll_to_display_offset(desired);
}

pub fn highlight_mask_for_match(
    session: &TerminalSession,
    cols: u16,
    rows: u16,
    regex_match: &Match,
) -> Vec<u8> {
    let mut mask = vec![0_u8; usize::from(cols) * usize::from(rows)];

    let display_offset = session.display_offset() as i32;
    let start = *regex_match.start();
    let end = *regex_match.end();

    let start_line = start.line.0;
    let end_line = end.line.0;
    if start_line > end_line {
        return mask;
    }

    for line in start_line..=end_line {
        let row = line + display_offset;
        if row < 0 || row >= i32::from(rows) {
            continue;
        }

        let col_start = if line == start_line { start.column.0 } else { 0 };
        let col_end = if line == end_line { end.column.0 } else { usize::from(cols).saturating_sub(1) };

        for col in col_start..=col_end.min(usize::from(cols).saturating_sub(1)) {
            let idx = row as usize * usize::from(cols) + col;
            if idx < mask.len() {
                mask[idx] = 1;
            }
        }
    }

    mask
}
