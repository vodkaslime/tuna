# Tuna — Design (actionable v0.1)

Tuna is a cross-platform terminal emulator (macOS/Windows/Linux) rendered with Bevy, with “game UI” aesthetics (low‑poly 3D game‑menu _feel_, not gamification) designed to keep power users in flow.

See also: `.codex/LOW_POLY_UI_UX.md` for the dedicated low‑poly 3D UI/UX spec (motion, layout, and Bevy implementation notes).

This document turns the product goals into an implementable technical plan: crates/modules, dependencies, iTerm2 parity targets (including hotkeys), and a milestone-by-milestone build plan.

---

## Goals

- **Functional parity (core)** with iTerm2 for: **sessions/tabs/windows**, **search**, **hotkey window / global hotkeys**, and **keybindings** (macOS preset).
- **Flow-first UX**: predictable interactions, low-latency feedback, and animations that guide attention without distraction.
- **Gaming aesthetics**: cohesive theme tokens, tasteful animation/sound hooks, and “HUD-like” composition while keeping terminal text pristine.
- **Cross-platform** from day one, with explicit graceful degradation where OS APIs are restricted (especially Wayland global hotkeys).
- **Local-only**: no telemetry, no accounts, no cloud sync.

## Definitions (match iTerm2 mental model)

- **Session**: one PTY-backed terminal instance (one shell process) with its own screen state.
- **Tab**: a container for one or more sessions (split panes). iTerm2 splits create multiple sessions in one tab.
- **Window**: an OS window containing a tab bar + the active tab’s session(s).
- **Hotkey Window**: a special window that toggles via a **system-wide hotkey** and behaves like a quake dropdown.

## Explicit non-goals (v0.1)

- Full iTerm2 feature parity (e.g., triggers, inline images, shell integration features, coprocesses).
- Plugin marketplace / scripting engine.
- Cloud sync, accounts, telemetry.

---

## MVP scope (“Daily Driver”)

### Terminal essentials

- PTY-backed sessions: spawn shell, resize, exit, signal handling.
- ANSI/VT parsing + screen model via proven terminal core crate (no custom emulator).
- Scrollback (50k+ lines), selection, copy/paste, bracketed paste.
- Basic profiles: shell, env, working dir, font, theme, scrollback size.

### Sessions / Tabs / Windows (iTerm2-parity subset)

- New window, new tab, close tab/session, move between tabs, tab titles.
- Detach tab to new window (optional v0.1.1, but data model should support it).
- Window state persistence: restore last window layout (position, size, active tab) locally.
- Split panes (**minimum v0.1**): create vertical/horizontal splits, focus a pane by direction, close the current pane; tab layout persists locally.

### Search (must-have parity)

iTerm2-style find bar:

- Incremental search; highlight all matches; jump next/previous.
- Searches across scrollback (not just viewport).
- Options: case sensitive, regex (optional), wrap-around.
- Mouseless copy behavior compatible with iTerm2 “cmd-f then tab/shift-tab extends selection” (stretch goal).

### Hotkeys / Hotkey Window (must-have parity)

Match iTerm2’s three hotkey concepts:

- **Toggle All Windows**: system-wide hotkey shows/hides all Tuna windows without moving them.
- **Session Hotkey**: optional: bind a hotkey to reveal a specific session/tab (later).
- **Dedicated Hotkey Window**: per-profile quake dropdown window toggled by a system-wide hotkey.

Hotkey Window behavior:

- Global hotkey toggles visibility while Tuna is unfocused.
- Window appears from the top edge; hides upward.
- When configured as “unpinned”, losing focus hides the hotkey window (iTerm2-style).
- Remembers geometry and display; supports reduced-motion (jump cut).
- Restores previously active app focus on hide (OS-specific best effort).

### Keybindings (macOS must match iTerm2 for shipped actions)

- One action map drives everything: inputs → actions → (terminal bytes | app commands).
- **macOS**: ship an `iterm2-macos` preset matching iTerm2 defaults for actions Tuna supports.
- **Windows/Linux**: ship Tuna defaults; fully configurable.

---

## iTerm2 parity: hotkeys + keybindings (macOS preset)

### Principle

Tuna implements actions, not hardcoded keys. The `iterm2-macos` preset is a mapping of iTerm2 default shortcuts to Tuna actions.

### Default shortcuts to match (v0.1)

Sources: iTerm2 docs (General Usage + Hotkeys + FAQ). The find “next/previous” inversion is part of iTerm2’s defaults; Tuna matches it but allows remap (see iTerm2 “Find Again Up/Down” and FAQ note about inversion).

#### Tabs / windows

- New Window: `Cmd+N`
- New Tab: `Cmd+T`
- Close Tab/Session: `Cmd+W`
- Next Tab: `Cmd+Right` **or** `Cmd+}` (same chord as `Shift+Cmd+]`)
- Previous Tab: `Cmd+Left` **or** `Cmd+{` (same chord as `Shift+Cmd+[`)
- Go to Tab 1–9: `Cmd+1` … `Cmd+9`
- Go to Window 1–9: `Cmd+Opt+1` … `Cmd+Opt+9`

#### Sessions (split panes)

- Split Vertically: `Cmd+D`
- Split Horizontally: `Shift+Cmd+D`
- Focus Pane by Direction: `Cmd+Opt+Arrow`
- Close current pane: `Cmd+W` (if last pane in tab, closes the tab)

#### Find

- Find: `Cmd+F`
- Find Again Up: `Cmd+G`
- Find Again Down: `Shift+Cmd+G`

#### Hotkeys (global/system-wide)

iTerm2 does not prescribe a default chord; the user sets it in settings. Tuna mirrors this:

- Toggle All Windows: **unbound by default**, user-configurable system-wide hotkey.
- Dedicated Hotkey Window: created from a profile; **unbound by default**, user-configurable.

### Future (documented now to avoid rework)

- Move pane between splits/tabs/windows (later).
- Resize split panes from keyboard (later).
- Broadcast input to all panes in tab (later).

---

## UX + “gaming aesthetics” spec

### Visual language (MVP)

Goals: distinct, readable, low-noise.

- **Chrome**: low‑poly 3D chips/panels with flat colors + stable lighting (no bundled textures).
- **Terminal text**: always clean monospace; prefer unlit materials for terminal surfaces in 3D.
- **Effects**: subtle depth (lighting/fog) + gentle emissive accents; disable optional effects in **High Contrast** and **Reduced Motion**.
- **Accessibility toggles**: Reduced Motion, High Contrast, Large UI Scale.

### Interaction + animation rules

- Animations communicate state change (tab switch, dropdown show/hide), not decoration.
- Default timings: 120–220ms for small UI transitions; 220–320ms for hotkey dropdown.
- Easing: cubic-out for entrances, cubic-in for exits; snap-to-end for reduced motion.
- Input latency target: <16ms processing; avoid heavy per-frame allocations.

### UI components (ship in v0.1)

- Top chrome: tab bar + window controls (custom, cross-platform), optional menubar-like button.
- Tab items: title + activity indicator (running process) + close button + “dirty” indicator (optional).
- Find bar: floating panel with input + match count + next/prev buttons + options (Aa, .\*).
- Command overlay (optional v0.2): “game menu” palette for actions (quick search actions).

---

## Technical architecture

### Workspace layout (Cargo workspace)

Crates are intentionally layered: core has no Bevy dependency; app/render/theme depend on Bevy.

- `tuna_app` (bin): Bevy app, windows, UI, input routing, animations.
- `tuna_terminal_core` (lib): PTY sessions, terminal engine wrapper, scrollback, selection, search.
- `tuna_renderer` (lib): Bevy render plugin for terminal grid (glyph atlas + instancing + damage).
- `tuna_platform` (lib): clipboard + global hotkey + window-level/focus integrations (OS-specific).
- `tuna_config` (lib): config schema, load/watch, migrations, keybindings and profiles.
- `tuna_theme` (lib): theme tokens, UI widgets (9-slice), asset loading conventions.

### Key design constraint: don’t write a terminal emulator

Pick one terminal engine and wrap it behind a stable internal API:

- Option A: `alacritty_terminal` (battle-tested parser + screen model; good baseline).
- Option B: `wezterm_term` (feature-rich; evaluate integration + size).

Decision spike (Phase 0) chooses one based on:

1. correctness (VT/ANSI coverage, tests/fixtures), 2) performance, 3) integration friction, 4) licensing, 5) community support.

PTY layer: `portable-pty` (cross-platform, ConPTY on Windows).

---

## Dependencies (initial shortlist)

### `tuna_app`

- `bevy` (windowing + input + UI + rendering)
- `bevy_tweening` (or equivalent) for UI + window animations
- `tracing`, `tracing-subscriber` (structured logs)
- `anyhow` (app-level error context)

### `tuna_terminal_core`

- `portable-pty`
- chosen terminal engine crate (`alacritty_terminal` or `wezterm_term`)
- `unicode-segmentation` (selection/search correctness)
- `regex` (optional for regex search)
- `parking_lot` (low-overhead locks)
- `crossbeam-channel` (threads → main communication)
- `thiserror` (library error types)

### `tuna_renderer`

- depends on `bevy_render` internals
- `swash` (glyph rasterization) + `etagere` (atlas packing) _or_ alternatives
- `bytemuck` (GPU instance buffers)

### `tuna_platform`

- `arboard` (clipboard)
- `global-hotkey` (system-wide hotkey registration; X11 on Linux; Wayland limitations)
- `windows` (Windows Win32 APIs) behind `cfg(windows)`
- `objc2` + `objc2-app-kit` (macOS focus/activation/window level) behind `cfg(target_os = "macos")`

### `tuna_config`

- `serde`, `toml` (or `toml_edit` for preserving formatting)
- `notify` (optional config hot-reload)
- `directories` (config/state paths)

### `tuna_theme`

- pure Rust structs; Bevy asset glue lives in `tuna_app`

---

## Crate/module design (actionable)

### `tuna_terminal_core` modules

- `pty/`: spawn + resize + read/write; abstracts OS differences.
  - `PtySession`: owns child process + pty master handle.
  - `PtyEvent`: bytes read, exit, resize ack/error.
- `engine/`: wrapper over chosen terminal engine.
  - `TerminalEngine`: trait exposing `advance(bytes)`, `resize`, `screen_snapshot`, `cursor`, `modes`.
- `model/`: canonical app-facing terminal state.
  - `ScreenModel`: grid cells for viewport + scrollback reference.
  - `Damage`: changed rows/regions since last frame (for renderer).
- `selection/`: selection model (linear, rectangular later), word boundaries.
- `search/`: find state and match index.
  - `FindQuery`: text/regex + flags.
  - `FindIndex`: incremental scan across scrollback with frame budget.
- `session/`: higher-level session controller.
  - `TerminalSession`: ties PTY + engine + selection/search, exposes events to app.
  - `SplitLayout`: (optional in core) data model for split tree; generally owned by app.

Threading model:

- 1 thread per session for PTY reads (blocking I/O).
- Main thread consumes events and advances engine; optionally move parse to a worker pool if needed.

### `tuna_renderer` modules

- `grid/`: converts `ScreenModel` → visible grid + instance data.
- `glyph_cache/`: font loading, glyph rasterization, atlas packing, cache eviction.
- `pipeline/`: Bevy render pipeline for instanced quads (fg glyph + bg rect).
- `damage/`: updates only changed rows/regions (instance buffer sub-updates).
- `overlays/`: cursor, selection highlight, find highlight, IME underline (later).

Renderer v1 strategy (ship it, then optimize):

1. Instanced quads per visible cell (position + uv + fg/bg colors).
2. Glyph atlas texture (monospace font; optional fallback font later).
3. Damage-driven buffer updates (rows) + scroll “memmove” optimization for smooth scrollback.

### `tuna_platform` modules

- `clipboard`: copy/paste abstraction with fallbacks.
- `hotkey`: register/unregister global hotkeys; route to app events.
- `windowing`: always-on-top, skip-taskbar/dock, focus restore best effort.
- `macos/`, `windows/`, `linux/` for platform glue.

### `tuna_app` modules (Bevy plugins)

- `app`: entrypoint + plugin wiring.
- `state`: ECS resources for windows/tabs/sessions.
  - `AppModel`: windows → tabs → sessions mapping (IDs stable across reload).
- `input`: key chord capture (winit), keymap resolution, action dispatch.
  - `Action`: enum for app commands + terminal bytes.
  - `KeyMap`: layered maps (global → window → mode).
- `ui/`: Bevy UI for chrome, tab bar, find bar.
- `animations/`: tween definitions for dropdown + panel transitions.
- `window_manager/`: multi-window handling + hotkey window controller.
- `layout/`: split tree layout, hit testing, drag-to-resize, and keyboard focus traversal.
- `persistence/`: save/restore window layout + last sessions metadata (not process restore).

---

## Configuration + persistence (local-only)

### Files + locations

Use `directories` to resolve per-OS standard paths.

- `config.toml`: user-editable configuration (profiles, keybindings, theme selection, hotkeys).
- `state.toml` (or `state.json`): runtime state (window geometry, last active tabs, last used profile).

Suggested locations:

- macOS: `~/Library/Application Support/Tuna/`
- Linux: `~/.config/tuna/`
- Windows: `%APPDATA%\\Tuna\\`

### Config schema (v1)

High-level shape:

- `version = 1`
- `[profiles.<name>]`: shell + env + working dir + appearance + hotkey-window options
- `[keymaps.<name>]`: maps `KeyChord` → `Action`
- `[defaults]`: default profile + default keymap preset
- `[hotkeys]`: global/system-wide hotkeys (unbound by default)

KeyChord representation (platform-agnostic):

- `mods = ["Cmd", "Shift", "Alt", "Ctrl"]`
- `key = "T" | "F" | "ArrowLeft" | "Enter" | ...`

### Example `config.toml` (illustrative)

```toml
version = 1

[defaults]
profile = "default"
keymap = "iterm2-macos"

[hotkeys]
# System-wide; unset by default (match iTerm2 behavior)
# toggle_all_windows = { mods = ["Cmd", "Shift"], key = "Space" }

[profiles.default]
shell = "/bin/zsh"
cwd = "~"
scrollback_lines = 50000
font_family = "JetBrains Mono"
font_size = 14
theme = "medieval_dark"

# Optional: dedicated hotkey window for this profile
[profiles.default.hotkey_window]
enabled = true
# toggle = { mods = ["Cmd", "Shift"], key = "Backtick" }
pinned = false
screen = "active"
height_ratio = 0.45
animation = "slide" # or "none" for reduced motion

[keymaps.iterm2-macos]
"Cmd+N" = "Window.New"
"Cmd+T" = "Tab.New"
"Cmd+W" = "Session.CloseOrTabClose"
"Cmd+Right" = "Tab.Next"
"Cmd+Left" = "Tab.Previous"
"Cmd+F" = "Find.Toggle"
"Cmd+G" = "Find.AgainUp"
"Shift+Cmd+G" = "Find.AgainDown"
"Cmd+D" = "Split.Vertical"
"Shift+Cmd+D" = "Split.Horizontal"
"Cmd+Alt+ArrowLeft" = "Pane.FocusLeft"
```

Notes:

- Chord string syntax is for human convenience; internally normalize into `KeyChord` structs.
- The parser should accept both `"Opt"` and `"Alt"` as synonyms on macOS.
- Always validate conflicts at load time and surface them in an in-app “Keybindings” screen.

---

## Functionality implementation plan (milestones)

### Phase 0 — Spikes (1–2 weeks)

- Terminal engine evaluation: integrate minimal PTY → engine → “dumb” renderer (debug text) for correctness tests.
- Renderer spike: glyph atlas + instanced grid performance test (60fps with 200x60 viewport).
- Global hotkey spike: register hotkey, receive event, toggle a dummy window; document Wayland limitations.

Exit criteria:

- Selected terminal engine with written rationale.
- Renderer prototype hits target FPS on mid hardware.
- Hotkey works on macOS + Windows + X11; degrades gracefully on Wayland.

### Phase 1 — Core sessions (PTY + engine + model)

- `tuna_terminal_core`: PTY session lifecycle, engine wrapper, screen snapshot, scrollback.
- Basic selection + clipboard copy/paste plumbing (platform clipboard stub acceptable).
- Golden fixtures: feed recorded VT sequences and assert screen output (snapshot tests).

Exit criteria:

- “Hello shell”: type + see output reliably across platforms.
- Resize correctness; process exit handled cleanly.

### Phase 2 — Renderer v1 + viewport UX

- `tuna_renderer`: grid render plugin with damage tracking.
- Smooth scrollback; selection highlight; cursor rendering.
- UI shell (tab bar placeholder, minimal theme).

Exit criteria:

- 50k+ scrollback; scroll and type remain smooth.

### Phase 3 — Tabs + windows

- App model for multiple windows/tabs/sessions.
- Key actions: new tab/window, close, next/prev, tab titles, close-confirm if process running.
- Split panes: create vertical/horizontal split, focus by direction, drag-to-resize, close current pane.
- Local persistence of window layout.

Exit criteria:

- Basic iTerm2-like navigation works entirely via keybindings.

### Phase 4 — Find bar + search across scrollback

- Find UI + incremental scanning with frame budget.
- Highlight all matches; focus/jump behavior; wrap-around option.
- “Find Again Up/Down” actions matching iTerm2 semantics.

Exit criteria:

- Search stays responsive with large scrollback; no frame hitches.

### Phase 5 — Hotkey window (quake dropdown) + global hotkeys

- Toggle All Windows hotkey.
- Dedicated Hotkey Window profile + toggle; pin/unpin; animate vs jump cut.
- Focus restore best-effort (macOS/Windows first); document Linux behavior.

Exit criteria:

- Hotkey window works while unfocused; restores previous app focus on hide (macOS/Windows).

### Phase 6 — Theme polish + “game UI” affordances

- Medieval theme tokens + 9-slice chrome assets.
- Animation polish + reduced motion + high contrast.
- Menu/overlay scaffolding (command palette later).

Exit criteria:

- Visual identity is cohesive without harming readability.

### Phase 7 — Packaging + release hardening

- macOS app bundle + notarization plan.
- Windows installer plan (MSIX/WiX) + signed binaries.
- Linux packaging target (AppImage/Flatpak).
- Crash reporting: local logs only; no telemetry.

---

## Risks + mitigations

- **Bevy multi-window quirks**: keep window logic isolated; test early; consider winit direct calls via Bevy window handles when needed.
- **Wayland global hotkeys**: provide focused-only fallback; optionally use portals if feasible.
- **Renderer complexity**: keep a “fallback renderer” mode (simpler path) to avoid being blocked by pipeline work.
- **Terminal engine integration**: wrap behind trait; keep core tests independent of renderer/UI.

---

## Acceptance criteria (v0.1)

- Sessions/tabs/windows: key-driven navigation; stable titles; close-confirm for running jobs.
- Split panes: `Cmd+D`/`Shift+Cmd+D` creates splits; `Cmd+Opt+Arrow` changes focus; close current pane behaves like iTerm2.
- Find: `Cmd+F` opens; results highlighted; `Cmd+G`/`Shift+Cmd+G` moves between matches; works across scrollback.
- Hotkeys: system-wide toggle works (where supported); hotkey window dropdown works and does not permanently steal focus.
- Performance: 60fps typing/scroll with 50k+ lines scrollback on mid-range hardware.

## References (for parity auditing)

- iTerm2 documentation (general usage / keyboard shortcuts): https://iterm2.com/documentation-one-page.html
- iTerm2 documentation (hotkey window): https://iterm2.com/documentation-hotkey.html
- iTerm2 documentation (keys preferences / hotkeys): https://iterm2.com/documentation-keys-preferences.html
- iTerm2 FAQ (find next/previous inversion note): https://iterm2.com/faq.html
