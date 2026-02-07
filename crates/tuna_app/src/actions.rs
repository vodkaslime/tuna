use std::collections::HashMap;

use bevy::input::keyboard::KeyCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Modifiers {
    pub cmd: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl Modifiers {
    pub const NONE: Self = Self {
        cmd: false,
        ctrl: false,
        alt: false,
        shift: false,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyChord {
    pub mods: Modifiers,
    pub key: KeyCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    TabNew,
    TabClose,
    TabNext,
    TabPrev,
    SplitVertical,
    SplitHorizontal,
    PaneFocusLeft,
    PaneFocusRight,
    PaneFocusUp,
    PaneFocusDown,
    FindToggle,
    FindAgainUp,
    FindAgainDown,
}

#[derive(Debug, Default)]
pub struct KeyMap {
    map: HashMap<KeyChord, Action>,
}

impl KeyMap {
    pub fn iterm2_macos() -> Self {
        use Action::*;
        use KeyCode::*;

        let cmd = Modifiers {
            cmd: true,
            ..Modifiers::NONE
        };
        let cmd_shift = Modifiers {
            cmd: true,
            shift: true,
            ..Modifiers::NONE
        };

        let mut map = HashMap::new();
        map.insert(KeyChord { mods: cmd, key: KeyT }, TabNew);
        map.insert(KeyChord { mods: cmd, key: KeyW }, TabClose);
        map.insert(KeyChord { mods: cmd, key: ArrowRight }, TabNext);
        map.insert(KeyChord { mods: cmd, key: ArrowLeft }, TabPrev);

        // iTerm2 also supports Cmd+{ / Cmd+} via shifted Cmd+[ / Cmd+]
        map.insert(KeyChord { mods: cmd_shift, key: BracketRight }, TabNext);
        map.insert(KeyChord { mods: cmd_shift, key: BracketLeft }, TabPrev);

        // Split panes.
        map.insert(KeyChord { mods: cmd, key: KeyD }, SplitVertical);
        map.insert(KeyChord { mods: cmd_shift, key: KeyD }, SplitHorizontal);

        // Focus pane by direction.
        let cmd_alt = Modifiers {
            cmd: true,
            alt: true,
            ..Modifiers::NONE
        };
        map.insert(KeyChord { mods: cmd_alt, key: ArrowLeft }, PaneFocusLeft);
        map.insert(KeyChord { mods: cmd_alt, key: ArrowRight }, PaneFocusRight);
        map.insert(KeyChord { mods: cmd_alt, key: ArrowUp }, PaneFocusUp);
        map.insert(KeyChord { mods: cmd_alt, key: ArrowDown }, PaneFocusDown);

        // Find.
        map.insert(KeyChord { mods: cmd, key: KeyF }, FindToggle);
        map.insert(KeyChord { mods: cmd, key: KeyG }, FindAgainUp);
        map.insert(KeyChord { mods: cmd_shift, key: KeyG }, FindAgainDown);

        Self { map }
    }

    pub fn resolve(&self, chord: KeyChord) -> Option<Action> {
        self.map.get(&chord).copied()
    }
}
