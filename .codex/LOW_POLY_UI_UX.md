# Tuna — Low‑Poly 3D UI/UX (Design Doc)

Tuna is a terminal rendered with Bevy. The goal of this UI/UX direction is to make Tuna feel like an elegant game menu: tactile, readable, responsive, and “flow‑state friendly” without gamifying work.

Constraints:

- No bundled art assets (no textures, no UI spritesheets). Use procedural meshes, flat colors, and lighting.
- Terminal text must remain crisp and stable (no post‑effects that cause shimmer or blur).
- Cross‑platform input + windowing.

This doc focuses on **the user‑facing experience** and **how to implement it in Bevy**.

---

## 1) Product Pillars

### Clarity first

- Terminal output is always the hero; UI chrome never competes with text.
- Session state is legible at a glance (active tab, activity, alerts).

### Game‑menu “feel” (without distraction)

- Motion communicates state changes: selection, transitions, hierarchy.
- Subtle depth and parallax create tactile affordances.

### Flow‑state ergonomics

- Fast, predictable interactions; keyboard always works.
- Optional Focus Mode reduces chrome and background motion.
- Reduced Motion option disables nonessential animation.

---

## 2) World Metaphor: “Terminal as a 3D Console”

Instead of a flat app UI, Tuna is a minimal low‑poly diorama:

- The **terminal surface** is rendered onto a **3D screen/plane** (unlit material for crispness).
- The **tab bar** is a **row of low‑poly “chips”** (lit material) floating slightly above the screen.
- Context actions (right‑click on a tab) appear as a **3D popover panel** with depth and shadow.
- The background is a simple low‑poly set: a gradient sky + a few geometric shapes for depth.

The world is subtle: it supports the terminal instead of stealing attention.

---

## 3) Layout & Navigation

### Primary layout

- Top: **Tab Deck** (3D chips aligned horizontally; supports many tabs with scroll or compress).
- Center: **Active Terminal Screen** (3D plane with the terminal texture).
- Optional bottom-right: **Small status HUD** (FPS toggle, connection/pty status, search state) as 2D UI for perfect crispness.

### Session Atlas (map view)

Treat sessions as locations on a navigable “map”:

- Sessions are laid out in world space (grid/spiral) and remain stable (no per-frame reflow).
- Users can **zoom out** to see multiple sessions at once and **pan** the camera like a strategy game.
- Selecting a session (tab click or world click) animates the camera to focus it.

### Tab affordances (per tab chip)

- Title (short, elided).
- Activity indicator (small dot pulse when output arrives).
- Close affordance (only on hover/focus to keep calm).
- Index hotkeys remain: `Cmd+1..9` etc.

### Context menu (right-click on a tab)

3D popover options:

- Move Left
- Move Right
- Close Other Tabs
- Close Tab

Popover placement:

- Anchored to the tab chip in screen space, offset downward.
- Clamped to window bounds.

---

## 4) Motion Design (Animations)

Motion should be “game UI”: quick, confident, and readable.

### Timing (default)

- Hover: 80–120ms
- Press: 60–90ms
- Tab switch: 160–240ms (depends on distance)
- Popover open/close: 120–180ms
- Camera settle (if used): 220–320ms

### Easing

- Entrances: ease‑out cubic
- Exits: ease‑in cubic
- Selection settle: ease‑out + tiny overshoot (optional) _only if it does not create jitter_

### Animation rules

- Never animate terminal glyphs themselves.
- Animate _containers_: camera, chip transforms, panel transforms, emissive intensity.
- Respect `Reduced Motion`: use instant transitions (or minimal fade).

### Key interactions to animate

1. **Switch tab**
   - Selected tab chip lifts slightly (Y + scale).
   - Previous active chip drops back.
   - Terminal screen transitions: either
     - **Camera pan** to a different screen in the world, or
     - **Single screen** that slides content via a quick lateral “wipe” (preferred for stability).
2. **New tab (+)**
   - Chip spawns with a quick scale‑in (0.9 → 1.0) and gentle bounce.
3. **Close tab**
   - Chip shrinks/fades and collapses spacing; avoid reflow jitter by animating layout.
4. **Context menu**
   - Popover scales from 0.98 → 1.0 with a fade‑in; shadow ramps in slightly after.

---

## 5) Visual System (Low‑Poly, No Assets)

### Colors

Use flat colors with limited palette, relying on lighting for depth. Suggested base palette:

- Background: deep slate/indigo gradient (ClearColor + fog)
- Chips (inactive): neutral dark
- Chip (active): slightly brighter + subtle emissive rim
- Accent: one “hero” color (cyan, gold, magenta) used sparingly for focus/selection
- Terminal plane: unlit, neutral; no tint

### Materials

- Terminal plane: `StandardMaterial { unlit: true, base_color_texture: terminal_image }`
- UI chips/panels: `StandardMaterial { unlit: false, perceptual_roughness ~0.6–0.9, metallic ~0.0–0.2 }`
- Selection glow: use `emissive` instead of bloom by default (bloom optional, off by default).

### Lighting

Simple, stable lighting (avoid flicker):

- One directional key light.
- One subtle fill (ambient).
- Optional slow “day/night” hue shift in background _only if it doesn’t distract_.

### Typography

No decorative fonts required. Use the existing monospace for UI to simplify:

- UI labels: same font as terminal or a bundled fallback (if later we add embedded fonts).

---

## 6) Interaction Model (Mouse + Keyboard)

### Keyboard‑first

- All session management remains hotkey driven (tab next/prev, number select).
- Add a “Game Menu” command palette (later) with fuzzy search and action list.

### Mouse interactions

- Hovering a tab chip highlights it and reveals close affordance.
- Left click selects.
- Right click opens context popover.
- Clicking outside closes popover.

### Focus + selection clarity

- When terminal has focus, the tab deck still reflects hover but never steals keyboard input.
- Avoid focus fights: the 3D UI emits events; the terminal input system remains the sink for typing.

---

## 7) Implementation Notes (Bevy)

### 7.1 Scene composition

Use two layers:

1. **3D World** (low‑poly scene): chips, panels, background, lights.
2. **2D UI Overlay** (Bevy UI): optional for text overlays that must be pixel‑perfect (status text, debug, find input).

Terminal surface approach:

- Keep the terminal renderer producing an `Image` handle.
- Instead of a 2D `SpriteBundle`, place that image on a 3D plane mesh:
  - `Mesh::from(Rectangle)` or a thin box for a bezel.
  - `StandardMaterial` with `unlit: true`.

### 7.2 Tab deck in 3D

Represent each tab as an entity:

- `TabChip { tab_id }` + `PbrBundle { mesh: box/rounded box (approx), material }`
- Child text label (either 2D overlay aligned above it, or 3D text if we accept slight blur).

Layout:

- For stability, drive positions from a deterministic layout function of tab order.
- Animate towards target positions with a tween/spring (but clamp to avoid jitter).

### 7.3 Animation system (no external crates required)

Implement a small tween component:

- `Tween { from: Transform, to: Transform, t0: f32, duration: f32, easing }`
- System updates `Transform` each frame until complete.

Optional: adopt `bevy_tweening` later once visuals settle.

### 7.4 Picking / hit testing

Options:

- Use a picking plugin (recommended) for raycasting to 3D UI meshes.
- Or implement a simple raycast:
  - `Camera::viewport_to_world` → ray
  - intersect ray with tab chip AABBs / plane

### 7.5 Reduced motion & accessibility

Add `UiSettings` resource:

- `reduced_motion: bool`
- `ui_scale: f32`
- `background_motion: f32` (0..1)

Every animation reads this and either:

- snaps immediately, or
- shortens duration and removes overshoot.

---

## 8) Rollout Plan (Incremental)

### Phase A: 3D terminal screen (foundation)

- Replace 2D sprite panel with 3D plane displaying terminal texture.
- Keep current 2D tab bar temporarily (for functionality).

### Phase B: 3D tab deck + animated switching

- Tab chips in 3D; keep same model operations (new/close/move).
- Add smooth tab transitions (chip lift + camera/plane motion).

### Phase C: Context menu popover in 3D

- Popover panel anchored to chip; animate open/close.

### Phase D: Ambient world (subtle)

- Add low‑poly background diorama, stable lighting, optional tiny particles.
- Add Focus Mode + Reduced Motion + High Contrast toggles.

---

## 9) “Flow” Features (Optional, Non‑Blocking)

These are “game menu polish” features that support focus:

- Focus Mode: hides tab deck until you move mouse to top or press a hotkey.
- Activity pings: subtle pulse on the active tab chip when command completes.
- Session health: small icon for disconnected/exited processes.
- Gentle “breathing” idle animation on background only (never on text).

---

## 10) Success Criteria

The UI is “game‑like” if:

- Tab switching feels responsive and satisfying (motion communicates what changed).
- The terminal never shimmers or blinks due to post‑effects or layout thrash.
- Reduced Motion results in a calm, immediate UI without losing clarity.
- The design remains performant at 60fps on integrated GPUs.
