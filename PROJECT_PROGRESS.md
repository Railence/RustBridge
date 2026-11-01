# Touch Control Editor Revamp — Project Progress

## Overall Goal
Full revamp of Horizon Android touch control editor:
- Gear icon replaces top-left toolbar buttons
- Gear menu: Add Button → Add Joystick → Save → Cancel
- Side config panel (left/right based on button position) with: Name, Keybinds×4, Scale, Width, Height, Opacity, Color, Swipeable, Toggle Mode, Hold-to-Toggle, Duplicate/Reset/Delete
- Swipeable: initiating finger can slide between swipeable buttons
- Toggle Mode: tap to latch keys on; tap again to release
- Hold-to-Toggle: hold ≥ duration → latch; short press → momentary
- Add Joystick: reuse existing joystick, movable, configurable
- All new fields serialized and persisted

## Implementation Stages
1. ✅ **Gear icon + basic side panel** — committed as d0cb0d2 on feature/multi-bind-touch-buttons
2. 🔄 **New CustomButton fields** (swipeable, toggle_mode, hold_to_toggle, hold_duration, color) — IN PROGRESS
3. ⬜ **Gear menu: Add Joystick item**
4. ⬜ **Swipeable runtime behavior** in custom_update
5. ⬜ **Toggle Mode + Hold-to-Toggle runtime behavior**
6. ⬜ **Expanded config panel** (all new rows, Duplicate/Reset/Delete)
7. ⬜ **Add Joystick panel** (reuse existing joystick system)
8. ⬜ **Persist color** (move from thread_local to CustomButton.color)
9. ⬜ **Polish + blue selection outline**

## Completed Stages
- Stage 1: Gear icon, draggable, opens menu (Add Button/Save/Cancel). Side panel with Name/Keys×4/Scale/Width/Height/Opacity/Color swatch. Committed d0cb0d2.

## Files Modified
- `crates/horizon-client-render/src/touch/custom.rs` — CustomButton struct, custom_update, draw_custom
- `crates/horizon-client-render/src/touch/editor.rs` — Editor struct, all draw/input code
- `crates/horizon-client-render/src/touch.rs` — test updated for gear flow
- `crates/horizon-client-render/src/touch/layout.rs` — no changes yet (Settings/Placement)

## Key Technical Facts
- `CustomButton.layouts: [Placement; 3]` — one per Mode index
- `TouchControls.custom_fingers: BTreeMap<u64, Vec<String>>` — finger→keys
- `TouchControls.custom_events: Vec<(String, bool)>` — press/release queue
- `custom_update(id, point, started, ended)` — returns true if consumed
- `Settings.button_scale: f32` — global scale (0.5–2.0)
- Normalised coords: x fraction of width, y fraction of height
- `panel_x(cx) -> f32`: 0.02 if button.cx > 0.5, else 0.62
- Gear menu rows: `[mx, my+0..], [mx, my+0.07], [mx, my+0.14], [mx, my+0.21]`
- Color currently in thread_local BUTTON_TINTS (not persisted) — needs moving to CustomButton

## Toggle/Swipeable State Tracking Plan
- Add to `TouchControls`: `toggle_active: BTreeMap<usize, bool>` (button index → toggled?)
- Add to `TouchControls`: `hold_timers: BTreeMap<u64, (usize, Instant)>` (finger → (btn_idx, press_time))
- Add to `TouchControls`: `swipe_finger: Option<u64>` (the one finger allowed to swipe)

## Known Issues / Decisions
- Build verification blocked (cargo check times out — network can't reach static.rust-lang.org)
- Color is thread_local (in-session only); needs moving to CustomButton.color: [u8; 4]
- Joystick implementation exists as Mode::Crosshair/Tap joystick — need to check if custom joystick placement is already supported

## Next Steps (Current Session)
1. Add fields to CustomButton: `swipeable`, `toggle_mode`, `hold_to_toggle`, `hold_duration`, `color`
2. Add toggle/swipe state to TouchControls
3. Implement Swipeable logic in custom_update
4. Implement Toggle/Hold-to-Toggle logic in custom_update
5. Update gear menu to 4 items (Add Joystick)
6. Expand editor panel with all new rows
7. Commit and push

## Continuation Prompt (for next session)
```
Read PROJECT_PROGRESS.md first, then inspect the current state of:
- crates/horizon-client-render/src/touch/custom.rs
- crates/horizon-client-render/src/touch/editor.rs
- crates/horizon-client-render/src/touch.rs
- crates/horizon-client-render/src/touch/layout.rs

Do NOT assume the previous session's description is still accurate — check the actual code.

Continue implementing the touch control editor revamp from the exact stage noted in PROJECT_PROGRESS.md.
Preserve all existing functionality. Update PROJECT_PROGRESS.md at every milestone.
When your message limit approaches, stop adding features, commit working state, update PROJECT_PROGRESS.md with exact next steps, and provide a SESSION HANDOFF summary.
```
