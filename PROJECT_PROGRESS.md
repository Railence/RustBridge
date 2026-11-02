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
1. ✅ **Gear icon + basic side panel** — committed d0cb0d2
2. ✅ **New CustomButton fields + runtime behavior** — committed d3bc1d1
3. ✅ **Gear menu: Add Joystick item** — committed d3bc1d1
4. ✅ **Swipeable runtime behavior** — committed d3bc1d1
5. ✅ **Toggle Mode + Hold-to-Toggle runtime behavior** — committed d3bc1d1
6. ✅ **Expanded config panel** (all rows + Duplicate/Reset/Delete) — committed d3bc1d1
7. ✅ **Persist color** (moved from thread_local to CustomButton.color) — committed d3bc1d1
8. ✅ **Blue selection outline** — committed d3bc1d1
9. ✅ **Add Joystick** — CustomJoystick struct, draw, input, config panel — committed 7e5dcd0
10. ✅ **tick_hold_timers() game loop** — called from pad.rs before poll_state() — committed d1a5528
11. ⬜ **Scrollable panel** (if panel overflows screen)

## Completed Stages
- Stage 1: Gear icon, draggable, opens menu. Side panel with Name/Keys×4/Scale/Width/Height/Opacity/Color swatch.
- Stage 2–8: Swipeable/Toggle/Hold-to-Toggle, gear menu 4 items, full config panel, color serialized, blue outline.
- Stage 9: Custom joystick system — `CustomJoystick` struct in layout.rs, `joystick.rs` module with draw/update/apply_movement, joy_fingers tracking, editor Add Joystick button (up to 4), drag to move, size+opacity sliders, Delete, joystick config panel in draw_editor.
- Stage 10: `tick_hold_timers()` called in `crates/horizon-client/src/game/pad.rs` Android section before `poll_state()`.

## Files Modified
- `crates/horizon-client-render/src/touch/custom.rs` — CustomButton struct, custom_update, draw_custom, tick_hold_timers
- `crates/horizon-client-render/src/touch/editor.rs` — Editor struct, all draw/input code, selected_joy, joystick panel
- `crates/horizon-client-render/src/touch.rs` — joy_fingers/joy_finger_to_joy fields, joystick mod, wiring
- `crates/horizon-client-render/src/touch/layout.rs` — CustomJoystick struct, custom_joysticks in Settings
- `crates/horizon-client-render/src/touch/joystick.rs` — NEW: draw/update/apply_movement for custom joysticks
- `crates/horizon-client/src/game/pad.rs` — tick_hold_timers() call in Android game tick

## Key Technical Facts
- `CustomJoystick { center: [f32;2], size: f32, opacity: f32 }` — stored in Settings.custom_joysticks: Vec
- Max 4 custom joysticks (editor enforces)
- `joy_fingers: HashMap<u64, [f32;2]>` — joy_idx → knob offset (on TouchControls, keyed by joy index NOT finger id)
- `joy_finger_to_joy: HashMap<u64, usize>` — finger_id → joy_idx
- `apply_custom_joy_movement` only fires if built-in joystick is inactive (state.left == [0,0])
- Slider opt 5 = joy size (0.06..0.40), opt 6 = joy opacity
- `panel_x(cx)`: 0.02 if cx > 0.5, else 0.62
- `draw_slider_row(b, a, px, base_y, row, label, value, value_str, active, sw, sh)`

## Known Issues
- Build verification blocked (cargo check times out — rust-toolchain pins 1.95.0, not downloadable in sandbox; stable 1.97.0 is installed but times out compiling full crate tree)
- Scrollable panel not yet implemented (low priority — most panels fit in screen height)

## Remaining Work
1. **Scrollable panel** — if a panel's content (e.g. button config with 17 rows) overflows the screen, add scroll_offset + up/down scroll indicator. Tracked on Editor struct.

## Continuation Prompt (for next session)
```
Read PROJECT_PROGRESS.md first, then inspect the current state of the modified files listed there.
Do NOT assume the previous session's description is still accurate — check the actual code.
Continue implementing the touch control editor revamp from the exact stage noted in PROJECT_PROGRESS.md.
Preserve all existing functionality. Update PROJECT_PROGRESS.md at every milestone.
When your message limit approaches, stop adding features, commit working state, update PROJECT_PROGRESS.md with exact next steps, and provide a SESSION HANDOFF summary.
```
