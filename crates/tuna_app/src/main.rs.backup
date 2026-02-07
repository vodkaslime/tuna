use std::collections::HashMap;

use tuna_terminal_core::pty::PtyDimensions;
use tuna_terminal_core::session::{TerminalSession, TerminalSessionError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PaneId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone)]
enum LayoutNode {
    Leaf(PaneId),
    Split {
        direction: SplitDirection,
        ratio: f32,
        a: Box<LayoutNode>,
        b: Box<LayoutNode>,
    },
}

impl LayoutNode {
    fn first_leaf(&self) -> PaneId {
        match self {
            LayoutNode::Leaf(id) => *id,
            LayoutNode::Split { a, .. } => a.first_leaf(),
        }
    }

    fn split_leaf(&mut self, target: PaneId, direction: SplitDirection, new_id: PaneId) -> bool {
        match self {
            LayoutNode::Leaf(id) if *id == target => {
                *self = LayoutNode::Split {
                    direction,
                    ratio: 0.5,
                    a: Box::new(LayoutNode::Leaf(target)),
                    b: Box::new(LayoutNode::Leaf(new_id)),
                };
                true
            }
            LayoutNode::Leaf(_) => false,
            LayoutNode::Split { a, b, .. } => {
                if a.split_leaf(target, direction, new_id) {
                    true
                } else {
                    b.split_leaf(target, direction, new_id)
                }
            }
        }
    }

    fn remove_leaf(self, target: PaneId) -> Option<LayoutNode> {
        match self {
            LayoutNode::Leaf(id) => (id != target).then_some(LayoutNode::Leaf(id)),
            LayoutNode::Split {
                direction,
                ratio,
                a,
                b,
            } => {
                let a = a.remove_leaf(target);
                let b = b.remove_leaf(target);
                match (a, b) {
                    (Some(a), Some(b)) => Some(LayoutNode::Split {
                        direction,
                        ratio,
                        a: Box::new(a),
                        b: Box::new(b),
                    }),
                    (Some(only), None) | (None, Some(only)) => Some(only),
                    (None, None) => None,
                }
            }
        }
    }

    fn layout(&self, rect: PaneRect, out: &mut Vec<(PaneId, PaneRect)>) {
        match self {
            LayoutNode::Leaf(id) => out.push((*id, rect)),
            LayoutNode::Split {
                direction,
                ratio,
                a,
                b,
            } => match direction {
                SplitDirection::Vertical => {
                    let total = rect.cols.max(1);
                    let mut a_cols = ((f32::from(total) * ratio).round() as i32)
                        .clamp(1, i32::from(total) - 1) as u16;
                    let mut b_cols = total - a_cols;
                    if b_cols == 0 {
                        a_cols = total - 1;
                        b_cols = 1;
                    }

                    a.layout(
                        PaneRect {
                            col: rect.col,
                            row: rect.row,
                            cols: a_cols,
                            rows: rect.rows,
                        },
                        out,
                    );
                    b.layout(
                        PaneRect {
                            col: rect.col + a_cols,
                            row: rect.row,
                            cols: b_cols,
                            rows: rect.rows,
                        },
                        out,
                    );
                }
                SplitDirection::Horizontal => {
                    let total = rect.rows.max(1);
                    let mut a_rows = ((f32::from(total) * ratio).round() as i32)
                        .clamp(1, i32::from(total) - 1) as u16;
                    let mut b_rows = total - a_rows;
                    if b_rows == 0 {
                        a_rows = total - 1;
                        b_rows = 1;
                    }

                    a.layout(
                        PaneRect {
                            col: rect.col,
                            row: rect.row,
                            cols: rect.cols,
                            rows: a_rows,
                        },
                        out,
                    );
                    b.layout(
                        PaneRect {
                            col: rect.col,
                            row: rect.row + a_rows,
                            cols: rect.cols,
                            rows: b_rows,
                        },
                        out,
                    );
                }
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PaneRect {
    pub col: u16,
    pub row: u16,
    pub cols: u16,
    pub rows: u16,
}

pub struct Pane {
    pub session: TerminalSession,
}

pub struct Tab {
    pub id: u64,
    pub title: String,
    root: LayoutNode,
    focused: PaneId,
    panes: HashMap<PaneId, Pane>,
    next_pane_id: u64,
}

impl Tab {
    fn new(
        tab_id: u64,
        dimensions: PtyDimensions,
        scrollback_lines: usize,
    ) -> Result<Self, TerminalSessionError> {
        let pane_id = PaneId(1);
        let session = TerminalSession::spawn_shell(dimensions, scrollback_lines)?;

        let mut panes = HashMap::new();
        panes.insert(
            pane_id,
            Pane {
                session,
            },
        );

        Ok(Self {
            id: tab_id,
            title: "Shell".to_string(),
            root: LayoutNode::Leaf(pane_id),
            focused: pane_id,
            panes,
            next_pane_id: 2,
        })
    }

    pub fn pane_count(&self) -> usize {
        self.panes.len()
    }

    pub fn focused_pane_id(&self) -> PaneId {
        self.focused
    }

    pub fn focused_pane_mut(&mut self) -> Option<&mut Pane> {
        self.panes.get_mut(&self.focused)
    }

    pub fn pane_mut(&mut self, id: PaneId) -> Option<&mut Pane> {
        self.panes.get_mut(&id)
    }

    pub fn focus_pane(&mut self, id: PaneId) -> bool {
        if self.panes.contains_key(&id) {
            self.focused = id;
            true
        } else {
            false
        }
    }

    pub fn layout(&self, dimensions: PtyDimensions) -> Vec<(PaneId, PaneRect)> {
        let mut out = Vec::new();
        self.root.layout(
            PaneRect {
                col: 0,
                row: 0,
                cols: dimensions.cols,
                rows: dimensions.rows,
            },
            &mut out,
        );
        out
    }

    pub fn apply_layout(&mut self, dimensions: PtyDimensions) {
        let layout = self.layout(dimensions);
        for (pane_id, rect) in layout {
            if let Some(pane) = self.panes.get_mut(&pane_id) {
                let _ = pane.session.resize(PtyDimensions::new(rect.cols, rect.rows));
            }
        }
    }

    pub fn split_focused(
        &mut self,
        direction: SplitDirection,
        dimensions: PtyDimensions,
        scrollback_lines: usize,
    ) -> Result<(), TerminalSessionError> {
        let new_id = PaneId(self.next_pane_id);
        self.next_pane_id = self.next_pane_id.saturating_add(1);

        let session = TerminalSession::spawn_shell(dimensions, scrollback_lines)?;
        self.panes.insert(
            new_id,
            Pane {
                session,
            },
        );

        let ok = self.root.split_leaf(self.focused, direction, new_id);
        if ok {
            self.focused = new_id;
            self.apply_layout(dimensions);
        }
        Ok(())
    }

    pub fn close_focused_pane(&mut self, dimensions: PtyDimensions) -> bool {
        if self.panes.len() <= 1 {
            return false;
        }

        let target = self.focused;
        self.panes.remove(&target);

        let root = std::mem::replace(&mut self.root, LayoutNode::Leaf(target));
        let Some(new_root) = root.remove_leaf(target) else {
            return false;
        };

        self.root = new_root;
        self.focused = self.root.first_leaf();
        self.apply_layout(dimensions);
        true
    }

    pub fn focus_by_direction(&mut self, direction: FocusDirection, dimensions: PtyDimensions) {
        let layout = self.layout(dimensions);
        if let Some(next) = focus_adjacent(&layout, self.focused, direction) {
            self.focused = next;
        }
    }

    pub fn pane(&self, id: PaneId) -> Option<&Pane> {
        self.panes.get(&id)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum FocusDirection {
    Left,
    Right,
    Up,
    Down,
}

fn focus_adjacent(
    layout: &[(PaneId, PaneRect)],
    focused: PaneId,
    direction: FocusDirection,
) -> Option<PaneId> {
    let (_, current) = layout.iter().find(|(id, _)| *id == focused)?;

    let mut best: Option<(PaneId, i32)> = None;

    for (id, rect) in layout {
        if *id == focused {
            continue;
        }

        let score = match direction {
            FocusDirection::Left => {
                let touches = rect.col + rect.cols == current.col;
                touches.then(|| overlap_1d(rect.row, rect.rows, current.row, current.rows))
            }
            FocusDirection::Right => {
                let touches = rect.col == current.col + current.cols;
                touches.then(|| overlap_1d(rect.row, rect.rows, current.row, current.rows))
            }
            FocusDirection::Up => {
                let touches = rect.row + rect.rows == current.row;
                touches.then(|| overlap_1d(rect.col, rect.cols, current.col, current.cols))
            }
            FocusDirection::Down => {
                let touches = rect.row == current.row + current.rows;
                touches.then(|| overlap_1d(rect.col, rect.cols, current.col, current.cols))
            }
        };

        let Some(score) = score else {
            continue;
        };
        if score <= 0 {
            continue;
        }

        match best {
            Some((_, best_score)) if best_score >= score => {}
            _ => best = Some((*id, score)),
        }
    }

    best.map(|(id, _)| id)
}

fn overlap_1d(a_start: u16, a_len: u16, b_start: u16, b_len: u16) -> i32 {
    let a0 = i32::from(a_start);
    let a1 = i32::from(a_start) + i32::from(a_len);
    let b0 = i32::from(b_start);
    let b1 = i32::from(b_start) + i32::from(b_len);
    (a1.min(b1) - a0.max(b0)).max(0)
}

pub struct AppModel {
    tabs: Vec<Tab>,
    active: usize,
    next_tab_id: u64,
    generation: u64,
    dimensions: PtyDimensions,
    scrollback_lines: usize,
}

impl AppModel {
    pub fn new(
        dimensions: PtyDimensions,
        scrollback_lines: usize,
    ) -> Result<Self, TerminalSessionError> {
        let mut model = Self {
            tabs: Vec::new(),
            active: 0,
            next_tab_id: 1,
            generation: 1,
            dimensions,
            scrollback_lines,
        };

        model.new_tab()?;
        Ok(model)
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn invalidate(&mut self) {
        self.generation = self.generation.saturating_add(1);
    }

    pub fn dimensions(&self) -> PtyDimensions {
        self.dimensions
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    pub fn tab_summaries(&self) -> Vec<TabSummary> {
        self.tabs
            .iter()
            .map(|tab| TabSummary {
                id: tab.id,
                title: tab.title.clone(),
            })
            .collect()
    }

    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active)
    }

    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active)
    }

    pub fn poll_sessions(&mut self) -> bool {
        let mut active_changed = false;
        let mut exited_tabs = Vec::new();
        for (idx, tab) in self.tabs.iter_mut().enumerate() {
            let mut all_exited = !tab.panes.is_empty();
            for pane in tab.panes.values_mut() {
                let changed = pane.session.poll_pty();
                if changed && idx == self.active {
                    active_changed = true;
                }
                all_exited &= pane.session.is_exited();
            }
            if all_exited {
                exited_tabs.push(tab.id);
            }
        }

        if !exited_tabs.is_empty() {
            for tab_id in exited_tabs {
                self.close_tab_by_id(tab_id);
            }
            active_changed = true;
        } else if active_changed {
            self.generation = self.generation.saturating_add(1);
        }

        active_changed
    }

    pub fn new_tab(&mut self) -> Result<(), TerminalSessionError> {
        let tab_id = self.next_tab_id;
        self.next_tab_id = self.next_tab_id.saturating_add(1);

        let mut tab = Tab::new(tab_id, self.dimensions, self.scrollback_lines)?;
        tab.apply_layout(self.dimensions);

        self.tabs.push(tab);
        self.active = self.tabs.len().saturating_sub(1);
        self.generation = self.generation.saturating_add(1);
        Ok(())
    }

    pub fn close_active_tab(&mut self) -> bool {
        if self.tabs.is_empty() {
            return false;
        }

        self.tabs.remove(self.active);
        if self.tabs.is_empty() {
            self.active = 0;
        } else if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
        }

        self.generation = self.generation.saturating_add(1);
        true
    }

    pub fn next_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        self.active = (self.active + 1) % self.tabs.len();
        self.generation = self.generation.saturating_add(1);
    }

    pub fn prev_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        self.active = if self.active == 0 {
            self.tabs.len() - 1
        } else {
            self.active - 1
        };
        self.generation = self.generation.saturating_add(1);
    }

    pub fn set_active_by_id(&mut self, tab_id: u64) -> bool {
        let Some((idx, _)) = self.tabs.iter().enumerate().find(|(_, tab)| tab.id == tab_id) else {
            return false;
        };
        if idx == self.active {
            return true;
        }
        self.active = idx;
        self.generation = self.generation.saturating_add(1);
        true
    }

    pub fn move_tab_left(&mut self, tab_id: u64) -> bool {
        let Some(idx) = self.tabs.iter().position(|tab| tab.id == tab_id) else {
            return false;
        };
        if idx == 0 {
            return false;
        }
        self.tabs.swap(idx, idx - 1);

        if self.active == idx {
            self.active = idx - 1;
        } else if self.active == idx - 1 {
            self.active = idx;
        }

        self.generation = self.generation.saturating_add(1);
        true
    }

    pub fn move_tab_right(&mut self, tab_id: u64) -> bool {
        let Some(idx) = self.tabs.iter().position(|tab| tab.id == tab_id) else {
            return false;
        };
        if idx + 1 >= self.tabs.len() {
            return false;
        }
        self.tabs.swap(idx, idx + 1);

        if self.active == idx {
            self.active = idx + 1;
        } else if self.active == idx + 1 {
            self.active = idx;
        }

        self.generation = self.generation.saturating_add(1);
        true
    }

    pub fn close_other_tabs(&mut self, keep_tab_id: u64) -> bool {
        if self.tabs.is_empty() {
            return false;
        }

        let mut keep = None;
        for tab in self.tabs.drain(..) {
            if tab.id == keep_tab_id {
                keep = Some(tab);
            }
        }

        let Some(keep) = keep else {
            return false;
        };

        self.tabs.push(keep);
        self.active = 0;
        self.generation = self.generation.saturating_add(1);
        true
    }

    pub fn close_tab_by_id(&mut self, tab_id: u64) -> bool {
        let Some(idx) = self.tabs.iter().position(|tab| tab.id == tab_id) else {
            return false;
        };

        self.tabs.remove(idx);

        if self.tabs.is_empty() {
            self.active = 0;
            let _ = self.new_tab();
            return true;
        }

        if idx < self.active {
            self.active = self.active.saturating_sub(1);
        } else if idx == self.active && self.active >= self.tabs.len() {
            self.active = self.tabs.len().saturating_sub(1);
        }

        self.generation = self.generation.saturating_add(1);
        true
    }

    pub fn resize_all(&mut self, dimensions: PtyDimensions) {
        self.dimensions = dimensions;
        for tab in &mut self.tabs {
            tab.apply_layout(dimensions);
        }
        self.generation = self.generation.saturating_add(1);
    }

    pub fn split_active(&mut self, direction: SplitDirection) -> Result<(), TerminalSessionError> {
        let dimensions = self.dimensions;
        let scrollback_lines = self.scrollback_lines;
        let Some(tab) = self.active_tab_mut() else {
            return Ok(());
        };

        tab.split_focused(direction, dimensions, scrollback_lines)?;
        self.generation = self.generation.saturating_add(1);
        Ok(())
    }

    pub fn close_focused_pane_or_tab(&mut self) {
        let dimensions = self.dimensions;
        let Some(tab) = self.active_tab_mut() else {
            return;
        };

        if tab.close_focused_pane(dimensions) {
            self.generation = self.generation.saturating_add(1);
            return;
        }

        let closed = self.close_active_tab();
        if closed && self.tabs.is_empty() {
            let _ = self.new_tab();
        }
    }

    pub fn focus_pane_by_direction(&mut self, direction: FocusDirection) {
        let dimensions = self.dimensions;
        let Some(tab) = self.active_tab_mut() else {
            return;
        };
        tab.focus_by_direction(direction, dimensions);
        self.generation = self.generation.saturating_add(1);
    }

    pub fn focus_pane_by_id(&mut self, pane_id: PaneId) {
        let Some(tab) = self.active_tab_mut() else {
            return;
        };
        if tab.focus_pane(pane_id) {
            self.generation = self.generation.saturating_add(1);
        }
    }
}

#[derive(Debug, Clone)]
pub struct TabSummary {
    pub id: u64,
    pub title: String,
}
