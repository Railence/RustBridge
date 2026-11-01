use super::*;

// ── Gear icon position (normalised, draggable) ──────────────────────────────
const GEAR_DEFAULT: [f32; 2] = [0.92, 0.08];
const GEAR_SIZE: f32 = 0.07; // fraction of height

// ── Panel layout ────────────────────────────────────────────────────────────
// Panel lives on left (x=0.02) or right (x=0.62) depending on button position.
// All y / h values are fractions of screen height; x / w of screen width.
const PANEL_W: f32 = 0.36;
const PANEL_H: f32 = 0.80;
const PANEL_Y: f32 = 0.10;
fn panel_x(button_cx: f32) -> f32 {
    if button_cx > 0.5 { 0.02 } else { 0.62 }
}

// Rows inside the panel (relative to panel origin)
const ROW_H: f32 = 0.055;
const ROW_GAP: f32 = 0.008;
fn panel_row(px: f32, base_y: f32, row: usize) -> [f32; 4] {
    [px + 0.01, base_y + 0.06 + row as f32 * (ROW_H + ROW_GAP), PANEL_W - 0.02, ROW_H]
}

pub struct Editor {
    /// Gear icon position (normalised centre).
    gear: [f32; 2],
    /// True while the gear menu (Add / Save / Cancel) is open.
    menu_open: bool,
    /// True while the visibility palette is shown (Add Button flow).
    palette: bool,
    /// Which regular control is selected for moving.
    pub selected: Option<Control>,
    /// Which custom button is selected for editing.
    selected_custom: Option<usize>,
    /// Which of the 4 key slots is being picked.
    key_slot: usize,
    key_page: usize,
    raw_keys: bool,
    key_picker: bool,
    label_editing: bool,
    /// 0=width 1=height 2=opacity 3=scale  — active slider
    active_slider: Option<usize>,
    /// Snapshot taken when the editor opened (for Cancel).
    original: Settings,
    drag: Option<(u64, DragKind)>,
    error: Option<String>,
}

enum DragKind {
    /// Dragging a button; stores grab offset from centre.
    Button([f32; 2]),
    /// Dragging the gear icon; stores grab offset from centre.
    Gear([f32; 2]),
    /// Dragging a slider (0=width,1=height,2=opacity,3=scale).
    Slider(usize),
}

// ── Color tint stored per button ─────────────────────────────────────────────
// We extend CustomButton by adding a `tint: [u8; 3]` field via a parallel vec
// in Settings.  Rather than changing the serialised format right now we keep it
// as editor-only in-memory state (resets on reload).  A follow-up can persist it.
thread_local! {
    static BUTTON_TINTS: std::cell::RefCell<Vec<[u8; 3]>> =
        std::cell::RefCell::new(Vec::new());
}
fn get_tint(index: usize) -> [u8; 3] {
    BUTTON_TINTS.with(|t| t.borrow().get(index).copied().unwrap_or([255, 255, 255]))
}
fn set_tint(index: usize, tint: [u8; 3]) {
    BUTTON_TINTS.with(|t| {
        let mut v = t.borrow_mut();
        if v.len() <= index { v.resize(index + 1, [255, 255, 255]); }
        v[index] = tint;
    });
}

// ── Open / close ─────────────────────────────────────────────────────────────
impl TouchControls {
    pub fn open_editor(&mut self) {
        self.clear();
        self.editor = Some(Editor {
            gear: GEAR_DEFAULT,
            menu_open: false,
            palette: false,
            selected: None,
            selected_custom: None,
            key_slot: 0,
            key_page: 0,
            raw_keys: false,
            key_picker: false,
            label_editing: false,
            active_slider: None,
            original: self.settings.clone(),
            drag: None,
            error: None,
        });
    }
    pub fn cancel_editor(&mut self) {
        if let Some(e) = self.editor.take() {
            self.settings = e.original;
        }
        self.clear();
    }

    // ── Main input dispatcher ─────────────────────────────────────────────
    /// Returns true when the editor was closed (Save / Cancel).
    pub fn edit_touch(&mut self, id: u64, p: [f32; 2], started: bool, ended: bool) -> bool {
        let Some(mut e) = self.editor.take() else { return false; };
        self.clicked |= started;

        // Key picker consumes everything.
        if e.key_picker {
            if started { self.handle_key_picker(&mut e, p); }
            self.editor = Some(e);
            return false;
        }

        // Gear drag (move the gear icon).
        if let Some((owner, DragKind::Gear(offset))) = &e.drag {
            if *owner == id {
                e.gear = [p[0] - offset[0], p[1] - offset[1]];
                e.gear[0] = e.gear[0].clamp(GEAR_SIZE / 2.0, 1.0 - GEAR_SIZE / 2.0);
                e.gear[1] = e.gear[1].clamp(GEAR_SIZE / 2.0, 1.0 - GEAR_SIZE / 2.0);
                if ended { e.drag = None; }
            }
            self.editor = Some(e);
            return false;
        }

        // Slider drag.
        if let Some((owner, DragKind::Slider(opt))) = &e.drag {
            if *owner == id {
                let opt = *opt;
                self.apply_slider(&mut e, p, opt);
                if ended { e.drag = None; }
            }
            self.editor = Some(e);
            return false;
        }

        // Button drag.
        if let Some((owner, DragKind::Button(offset))) = &e.drag {
            if *owner == id {
                let off = *offset;
                self.apply_button_drag(&mut e, p, off);
                if ended { e.drag = None; }
            }
            self.editor = Some(e);
            return false;
        }

        if started {
            // ── Gear icon ──────────────────────────────────────────────────
            let gear_r = gear_rect(e.gear);
            if layout::contains(gear_r, p) {
                // Short tap = toggle menu; long-press will drag (handled below).
                e.menu_open = !e.menu_open;
                if e.menu_open {
                    e.selected = None;
                    e.selected_custom = None;
                    e.palette = false;
                }
                if !ended {
                    e.drag = Some((id, DragKind::Gear([p[0] - e.gear[0], p[1] - e.gear[1]])));
                }
                self.editor = Some(e);
                return false;
            }

            // ── Gear menu ──────────────────────────────────────────────────
            if e.menu_open {
                let (mx, my) = menu_origin(e.gear);
                if layout::contains([mx, my,          0.22, 0.06], p) {
                    // Add Button → open palette
                    e.palette = true;
                    e.menu_open = false;
                    self.editor = Some(e);
                    return false;
                } else if layout::contains([mx, my + 0.07,  0.22, 0.06], p) {
                    // Save
                    e.menu_open = false;
                    match self.settings.save() {
                        Ok(()) => { self.clear(); return true; }
                        Err(err) => { e.error = Some(err.to_string()); }
                    }
                    self.editor = Some(e);
                    return false;
                } else if layout::contains([mx, my + 0.14, 0.22, 0.06], p) {
                    // Cancel
                    self.settings = e.original;
                    self.clear();
                    return true;
                } else {
                    e.menu_open = false;
                }
            }

            // ── Visibility palette ─────────────────────────────────────────
            if e.palette {
                // "Create custom button" row at the bottom of the palette.
                if layout::contains([0.22, 0.84, 0.56, 0.07], p) {
                    if self.settings.custom_buttons.len() < 32 {
                        let idx = self.settings.custom_buttons.len();
                        self.settings.custom_buttons
                            .push(CustomButton::new("key.keyboard.unknown".into(), idx));
                        e.selected_custom = Some(idx);
                        e.key_picker = true;
                        e.raw_keys = false;
                        e.key_page = 0;
                        e.palette = false;
                        e.drag = None;
                    }
                    self.editor = Some(e);
                    return false;
                }
                // Toggle vanilla control visibility.
                for (index, control) in Control::ALL.into_iter().enumerate() {
                    if layout::contains(palette_rect(index), p) {
                        let show = !self.visible(control);
                        self.settings.button_visibility[index] = if show { 1 } else { 2 };
                    }
                }
                // Tap outside palette → close it.
                if !layout::contains([0.2, 0.14, 0.6, 0.75], p) {
                    e.palette = false;
                }
                self.editor = Some(e);
                return false;
            }

            // ── Panel interaction ──────────────────────────────────────────
            if let Some(result) = self.handle_panel_input(&mut e, id, p) {
                if result { return true; }
                self.editor = Some(e);
                return false;
            }

            // ── Hit-test buttons ───────────────────────────────────────────
            // Custom buttons.
            if let Some(index) = self.custom_at(p) {
                let center = self.custom_placement(index).center;
                e.selected = None;
                e.selected_custom = Some(index);
                e.active_slider = None;
                e.drag = Some((id, DragKind::Button([p[0] - center[0], p[1] - center[1]])));
                self.editor = Some(e);
                return false;
            }
            // Regular controls.
            if let Some(c) = Control::ALL.into_iter().rev().find(|c| {
                self.visible(*c) && layout::contains(self.placement(*c).rect(self.aspect()), p)
            }) {
                let center = self.placement(c).center;
                e.selected = Some(c);
                e.selected_custom = None;
                e.active_slider = None;
                e.drag = Some((id, DragKind::Button([p[0] - center[0], p[1] - center[1]])));
                self.editor = Some(e);
                return false;
            }
            // Tap on empty space → deselect.
            e.selected = None;
            e.selected_custom = None;
            e.active_slider = None;
        }

        self.editor = Some(e);
        false
    }

    // ── Panel input handling ─────────────────────────────────────────────────
    /// Returns Some(true) = close editor, Some(false) = handled, None = not consumed.
    fn handle_panel_input(&mut self, e: &mut Editor, id: u64, p: [f32; 2]) -> Option<bool> {
        // Custom button panel.
        if let Some(ci) = e.selected_custom {
            let bx = {
                let cx = self.custom_placement(ci).center[0];
                panel_x(cx)
            };
            let by = PANEL_Y;

            // Row 0: Name
            if layout::contains(panel_row(bx, by, 0), p) {
                e.label_editing = !e.label_editing;
                return Some(false);
            }
            // Rows 1-4: Key slots
            for slot in 0..4usize {
                if layout::contains(panel_row(bx, by, 1 + slot), p) {
                    e.key_slot = slot;
                    e.key_picker = true;
                    e.key_page = 0;
                    e.drag = None;
                    return Some(false);
                }
            }
            // Row 5: Scale slider label
            if layout::contains(panel_row(bx, by, 5), p) {
                e.active_slider = Some(3);
                return Some(false);
            }
            // Row 6: Width slider label
            if layout::contains(panel_row(bx, by, 6), p) {
                e.active_slider = Some(0);
                return Some(false);
            }
            // Row 7: Height slider label
            if layout::contains(panel_row(bx, by, 7), p) {
                e.active_slider = Some(1);
                return Some(false);
            }
            // Row 8: Opacity slider label
            if layout::contains(panel_row(bx, by, 8), p) {
                e.active_slider = Some(2);
                return Some(false);
            }
            // Row 9: Color wheel label — open wheel (toggle)
            if layout::contains(panel_row(bx, by, 9), p) {
                // Cycle hue by 30° as a simple "tap to pick color" stub.
                let tint = get_tint(ci);
                let next = hue_rotate(tint, 30.0);
                set_tint(ci, next);
                return Some(false);
            }
            // Row 10: Remove
            if layout::contains(panel_row(bx, by, 10), p) {
                self.settings.custom_buttons.remove(ci);
                e.selected_custom = None;
                e.drag = None;
                return Some(false);
            }
            // Slider track (below active slider label, same x band).
            if let Some(opt) = e.active_slider {
                let row = 5 + opt; // scale=5,width=6,height=7,opacity=8
                let track = slider_track(bx, by, row);
                if layout::contains(track, p) {
                    e.drag = Some((id, DragKind::Slider(opt)));
                    return Some(false);
                }
            }
            // Tap outside panel → deselect.
            let panel = [bx, by, PANEL_W, PANEL_H];
            if !layout::contains(panel, p) {
                e.selected_custom = None;
                e.active_slider = None;
            }
            return Some(false);
        }

        // Regular control panel.
        if let Some(sel) = e.selected {
            let bx = {
                let cx = self.placement(sel).center[0];
                panel_x(cx)
            };
            let by = PANEL_Y;

            if layout::contains(panel_row(bx, by, 0), p) {
                e.active_slider = Some(0); // width
                return Some(false);
            }
            if layout::contains(panel_row(bx, by, 1), p) {
                e.active_slider = Some(1); // height
                return Some(false);
            }
            if layout::contains(panel_row(bx, by, 2), p) {
                e.active_slider = Some(2); // opacity
                return Some(false);
            }
            if layout::contains(panel_row(bx, by, 3), p) {
                // Hide button
                self.settings.button_visibility[sel as usize] = 2;
                e.selected = None;
                return Some(false);
            }
            if let Some(opt) = e.active_slider {
                let track = slider_track(bx, by, opt);
                if layout::contains(track, p) {
                    e.drag = Some((id, DragKind::Slider(opt)));
                    return Some(false);
                }
            }
            let panel = [bx, by, PANEL_W, PANEL_H];
            if !layout::contains(panel, p) {
                e.selected = None;
                e.active_slider = None;
            }
            return Some(false);
        }

        None
    }

    // ── Drag helpers ─────────────────────────────────────────────────────────
    fn apply_button_drag(&mut self, e: &mut Editor, p: [f32; 2], offset: [f32; 2]) {
        let aspect = self.aspect();
        let mut center = [p[0] - offset[0], p[1] - offset[1]];
        if self.settings.left_handed { center[0] = 1.0 - center[0]; }

        if let Some(ci) = e.selected_custom {
            let place = &mut self.settings.custom_buttons[ci]
                .layouts[self.settings.mode.index()];
            let size = (place.size() * self.settings.button_scale).min(aspect).min(1.0);
            place.center = [
                center[0].clamp(size / aspect / 2.0, 1.0 - size / aspect / 2.0),
                center[1].clamp(size / 2.0, 1.0 - size / 2.0),
            ];
        } else if let Some(sel) = e.selected {
            let slot = self.settings.slot(sel);
            let place = &mut self.settings.layouts[self.settings.mode.index()][slot];
            let size = (place.size() * self.settings.button_scale).min(aspect).min(1.0);
            place.center = [
                center[0].clamp(size / aspect / 2.0, 1.0 - size / aspect / 2.0),
                center[1].clamp(size / 2.0, 1.0 - size / 2.0),
            ];
        }
    }

    fn apply_slider(&mut self, e: &mut Editor, p: [f32; 2], opt: usize) {
        // Determine panel x so we can compute slider value from screen coords.
        let px = if let Some(ci) = e.selected_custom {
            panel_x(self.custom_placement(ci).center[0])
        } else if let Some(sel) = e.selected {
            panel_x(self.placement(sel).center[0])
        } else {
            return;
        };
        let track_x = px + 0.02;
        let track_w = PANEL_W - 0.04;
        let value = ((p[0] - track_x) / track_w).clamp(0.0, 1.0);

        if let Some(ci) = e.selected_custom {
            let place = &mut self.settings.custom_buttons[ci]
                .layouts[self.settings.mode.index()];
            match opt {
                0 => place.width   = 0.045 + value * 0.405,
                1 => place.height  = 0.045 + value * 0.405,
                2 => place.opacity = value,
                3 => self.settings.button_scale = 0.5 + value * 1.5,
                _ => {}
            }
        } else if let Some(sel) = e.selected {
            let slot = self.settings.slot(sel);
            let place = &mut self.settings.layouts[self.settings.mode.index()][slot];
            match opt {
                0 => place.width   = 0.045 + value * 0.405,
                1 => place.height  = 0.045 + value * 0.405,
                2 => place.opacity = value,
                _ => {}
            }
        }
    }

    // ── Key picker input ─────────────────────────────────────────────────────
    fn handle_key_picker(&mut self, e: &mut Editor, p: [f32; 2]) {
        if layout::contains([0.15, 0.16, 0.33, 0.06], p) {
            e.raw_keys = false; e.key_page = 0;
        } else if layout::contains([0.52, 0.16, 0.33, 0.06], p) {
            e.raw_keys = true; e.key_page = 0;
        } else if layout::contains([0.2, 0.86, 0.18, 0.07], p) {
            e.key_page = e.key_page.saturating_sub(1);
        } else if layout::contains([0.41, 0.86, 0.18, 0.07], p) {
            e.key_picker = false;
        } else if layout::contains([0.62, 0.86, 0.18, 0.07], p) {
            e.key_page = (e.key_page + 1)
                .min(super::custom::choices(e.raw_keys).len().saturating_sub(1) / 8);
        } else {
            for (i, (token, _)) in super::custom::choices(e.raw_keys)
                .into_iter().skip(e.key_page * 8).take(8).enumerate()
            {
                if layout::contains(key_rect(i), p) {
                    if let Some(ci) = e.selected_custom {
                        self.settings.custom_buttons[ci].keys[e.key_slot] = token;
                    }
                    e.key_picker = false;
                    return;
                }
            }
            // Clear slot
            if layout::contains([0.15, 0.25 + 8.0 * 0.072, 0.7, 0.06], p) {
                if let Some(ci) = e.selected_custom {
                    self.settings.custom_buttons[ci].keys[e.key_slot].clear();
                }
                e.key_picker = false;
            }
        }
    }

    pub fn capture_custom_key(&mut self, name: &str) -> bool {
        if crate::keybinds::key_code(name).is_none() { return false; }
        if let Some(e) = &mut self.editor
            && e.key_picker
            && let Some(i) = e.selected_custom
        {
            self.settings.custom_buttons[i].keys[e.key_slot] = name.into();
            e.key_picker = false;
            true
        } else {
            false
        }
    }

    // ── Draw ─────────────────────────────────────────────────────────────────
    /// Old toolbar — now hidden during editing (no-op kept for call-site compat).
    pub(super) fn draw_editor_toolbar(&self, _b: &mut HudBatch, _a: &HudAtlas) {}

    pub(super) fn draw_editor(&self, b: &mut HudBatch, a: &HudAtlas) {
        let e = self.editor.as_ref().unwrap();
        let (sw, sh) = (b.width, b.height);

        // Key picker overlay.
        if e.key_picker {
            self.draw_key_picker(b, a, e);
            return;
        }

        // Visibility palette overlay.
        if e.palette {
            self.draw_palette(b, a);
            return;
        }

        // Gear icon.
        draw_gear(b, a, e.gear);

        // Gear menu.
        if e.menu_open {
            let (mx, my) = menu_origin(e.gear);
            plate(b, a, [mx, my,          0.22, 0.06], "Add Button", false);
            plate(b, a, [mx, my + 0.07,   0.22, 0.06], "Save",       false);
            plate(b, a, [mx, my + 0.14,   0.22, 0.06], "Cancel",     false);
        }

        // Custom button panel.
        if let Some(ci) = e.selected_custom {
            let bx = panel_x(self.custom_placement(ci).center[0]);
            let by = PANEL_Y;
            let btn = &self.settings.custom_buttons[ci];
            let place = btn.layouts[self.settings.mode.index()];

            // Panel background.
            b.fill(a, sw * bx, sh * by, sw * PANEL_W, sh * PANEL_H, rgba(20, 20, 20, 230));

            let title_col = rgba(245, 245, 245, 255);
            b.text(a, "Button Settings", sw * (bx + 0.02), sh * (by + 0.02), title_col, false);

            // Row 0: Name
            let lbl = btn.custom_label.as_deref().unwrap_or("");
            let name_text = if lbl.is_empty() {
                format!("Name: {} (auto)", btn.label())
            } else {
                format!("Name: {lbl}")
            };
            plate(b, a, panel_row(bx, by, 0), &name_text, e.label_editing);

            // Rows 1-4: Key slots
            for slot in 0..4usize {
                let key = &btn.keys[slot];
                let slot_label = if key.is_empty() || key == "key.keyboard.unknown" {
                    format!("Key {}: (none)", slot + 1)
                } else if let Some(n) = key.strip_prefix("action:") {
                    format!("Key {}: {}", slot + 1, crate::text::Text::translate(n, &[]).plain())
                } else {
                    format!("Key {}: {}", slot + 1, crate::keybinds::display_name(key).plain())
                };
                plate(b, a, panel_row(bx, by, 1 + slot), &slot_label, false);
            }

            // Rows 5-8: Sliders
            let scale_val = (self.settings.button_scale - 0.5) / 1.5;
            let width_val  = (place.width  - 0.045) / 0.405;
            let height_val = (place.height - 0.045) / 0.405;
            let opacity_val = place.opacity;

            draw_slider_row(b, a, bx, by, 5,  "Scale",   scale_val,   format!("{:.0}%", self.settings.button_scale * 100.0), e.active_slider == Some(3), sw, sh);
            draw_slider_row(b, a, bx, by, 6,  "Width",   width_val,   format!("{:.0}%", place.width * 100.0),   e.active_slider == Some(0), sw, sh);
            draw_slider_row(b, a, bx, by, 7,  "Height",  height_val,  format!("{:.0}%", place.height * 100.0),  e.active_slider == Some(1), sw, sh);
            draw_slider_row(b, a, bx, by, 8,  "Opacity", opacity_val, format!("{:.0}%", place.opacity * 100.0), e.active_slider == Some(2), sw, sh);

            // Row 9: Color
            let tint = get_tint(ci);
            let color_label = format!("Color  #{:02X}{:02X}{:02X}  (tap)", tint[0], tint[1], tint[2]);
            plate(b, a, panel_row(bx, by, 9), &color_label, false);
            // Small color swatch next to the label.
            let [cx, cy, cw, ch] = panel_row(bx, by, 9);
            b.fill(a, sw * (cx + cw - 0.04), sh * cy + 2.0, sw * 0.035, sh * ch - 4.0,
                rgba(tint[0], tint[1], tint[2], 255));

            // Row 10: Remove
            plate(b, a, panel_row(bx, by, 10), "Remove button", false);
        }

        // Regular control panel.
        if let Some(sel) = e.selected {
            let bx = panel_x(self.placement(sel).center[0]);
            let by = PANEL_Y;
            let slot = self.settings.slot(sel);
            let place = self.settings.layouts[self.settings.mode.index()][slot];

            b.fill(a, sw * bx, sh * by, sw * PANEL_W, sh * 0.42, rgba(20, 20, 20, 230));

            let title_col = rgba(245, 245, 245, 255);
            b.text(a, sel.name(), sw * (bx + 0.02), sh * (by + 0.02), title_col, false);

            let w_val = (place.width  - 0.045) / 0.405;
            let h_val = (place.height - 0.045) / 0.405;
            let o_val = place.opacity;
            draw_slider_row(b, a, bx, by, 0, "Width",   w_val, format!("{:.0}%", place.width * 100.0),   e.active_slider == Some(0), sw, sh);
            draw_slider_row(b, a, bx, by, 1, "Height",  h_val, format!("{:.0}%", place.height * 100.0),  e.active_slider == Some(1), sw, sh);
            draw_slider_row(b, a, bx, by, 2, "Opacity", o_val, format!("{:.0}%", place.opacity * 100.0), e.active_slider == Some(2), sw, sh);
            plate(b, a, panel_row(bx, by, 3), "Hide button", false);
        }

        // Error toast.
        if e.error.is_some() {
            b.fill(a, sw * 0.2, sh * 0.92, sw * 0.6, sh * 0.06, rgba(180, 30, 30, 220));
            b.text(a, "Save failed \u{2014} tap gear to retry", sw * 0.22, sh * 0.94,
                rgba(255, 255, 255, 255), false);
        }
    }

    fn draw_key_picker(&self, b: &mut HudBatch, a: &HudAtlas, e: &Editor) {
        let (w, h) = (b.width, b.height);
        b.fill(a, w * 0.12, h * 0.14, w * 0.76, h * 0.81, rgba(15, 15, 15, 250));
        plate(b, a, [0.15, 0.16, 0.33, 0.06], "Game action",  !e.raw_keys);
        plate(b, a, [0.52, 0.16, 0.33, 0.06], "Keyboard key",  e.raw_keys);
        for (i, (_, label)) in super::custom::choices(e.raw_keys)
            .iter().skip(e.key_page * 8).take(8).enumerate()
        {
            plate(b, a, key_rect(i), label, false);
        }
        plate(b, a, [0.15, 0.25 + 8.0 * 0.072, 0.7, 0.06], "— Clear this slot —", false);
        plate(b, a, [0.2,  0.86, 0.18, 0.07], "Previous", false);
        plate(b, a, [0.41, 0.86, 0.18, 0.07], "Back",     false);
        plate(b, a, [0.62, 0.86, 0.18, 0.07], "Next",     false);
    }

    fn draw_palette(&self, b: &mut HudBatch, a: &HudAtlas) {
        let (w, h) = (b.width, b.height);
        b.fill(a, w * 0.2, h * 0.14, w * 0.6, h * 0.75, rgba(15, 15, 15, 245));
        for (index, control) in Control::ALL.into_iter().enumerate() {
            let shown = self.visible(control);
            plate(b, a, palette_rect(index),
                &format!("{} {}", if shown { "[-]" } else { "[+]" }, control.name()), shown);
        }
        plate(b, a, [0.22, 0.84, 0.56, 0.07], "Create custom button...", false);
    }
}

// ── Geometry helpers ─────────────────────────────────────────────────────────
fn gear_rect(gear: [f32; 2]) -> [f32; 4] {
    let h = GEAR_SIZE;
    let w = GEAR_SIZE * 0.6; // narrower in normalised-width coords (aspect-agnostic)
    [gear[0] - w / 2.0, gear[1] - h / 2.0, w, h]
}
fn menu_origin(gear: [f32; 2]) -> (f32, f32) {
    // Place menu below the gear, clamped to screen.
    let x = (gear[0] - 0.11).clamp(0.01, 0.76);
    let y = (gear[1] + GEAR_SIZE / 2.0 + 0.01).min(0.80);
    (x, y)
}
fn slider_track(px: f32, base_y: f32, row: usize) -> [f32; 4] {
    let r = panel_row(px, base_y, row);
    // Track sits just below the label row.
    [r[0], r[1] + r[3] + 0.004, PANEL_W - 0.02, 0.025]
}
fn palette_rect(index: usize) -> [f32; 4] {
    [
        0.22 + (index % 2) as f32 * 0.29,
        0.17 + (index / 2) as f32 * 0.11,
        0.27,
        0.09,
    ]
}
fn key_rect(i: usize) -> [f32; 4] {
    [0.15, 0.25 + i as f32 * 0.072, 0.7, 0.06]
}

// ── Draw helpers ─────────────────────────────────────────────────────────────
fn draw_gear(b: &mut HudBatch, a: &HudAtlas, gear: [f32; 2]) {
    let (w, h) = (b.width, b.height);
    let r = gear_rect(gear);
    // Draw as a tinted plate so it's always visible.
    nine(b, a, "ui/button_borderless_light", 1.0,
        [r[0] * w, r[1] * h, r[2] * w, r[3] * h],
        rgba(80, 160, 255, 220));
    b.text(a, "\u{2699}", (r[0] + r[2] / 2.0) * w - 4.0, (r[1] + r[3] / 2.0) * h - 5.0,
        rgba(255, 255, 255, 255), false);
}

fn draw_slider_row(
    b: &mut HudBatch, a: &HudAtlas,
    px: f32, base_y: f32, row: usize,
    label: &str, value: f32, value_str: String,
    active: bool, sw: f32, sh: f32,
) {
    let r = panel_row(px, base_y, row);
    plate(b, a, r, &format!("{label}: {value_str}"), active);
    if active {
        // Draw the slider track + thumb.
        let track = slider_track(px, base_y, row);
        nine(b, a, "ui/slider_background", 1.0,
            [track[0] * sw, track[1] * sh, track[2] * sw, track[3] * sh],
            rgba(255, 255, 255, 255));
        let thumb_x = track[0] + value * track[2];
        nine(b, a, "ui/slider_button_default", 2.0,
            [thumb_x * sw - 3.0, (track[1] - 0.01) * sh, 6.0, (track[3] + 0.02) * sh],
            rgba(255, 255, 255, 255));
    }
}

fn plate(b: &mut HudBatch, a: &HudAtlas, [x, y, w, h]: [f32; 4], label: &str, on: bool) {
    let (bw, bh) = (b.width, b.height);
    nine(b, a, "ui/button_borderless_light", 1.0,
        [x * bw, y * bh, w * bw, h * bh],
        if on { rgba(100, 210, 90, 255) } else { rgba(255, 255, 255, 255) });
    let mut display = label.to_owned();
    while HudBatch::text_width(a, &display) > w * bw - 6.0 && !display.is_empty() {
        display.pop();
    }
    b.text(a, &display, x * bw + 3.0, y * bh + (h * bh - 8.0) / 2.0,
        rgba(35, 35, 35, 255), false);
}

// ── Color helpers ─────────────────────────────────────────────────────────────
fn hue_rotate(rgb: [u8; 3], degrees: f32) -> [u8; 3] {
    let (r, g, bl) = (rgb[0] as f32 / 255.0, rgb[1] as f32 / 255.0, rgb[2] as f32 / 255.0);
    let max = r.max(g).max(bl);
    let min = r.min(g).min(bl);
    let delta = max - min;
    if delta < 0.001 { return [200, 200, 200]; }
    let mut h = if max == r { (g - bl) / delta % 6.0 }
                else if max == g { (bl - r) / delta + 2.0 }
                else { (r - g) / delta + 4.0 };
    h = (h * 60.0 + degrees).rem_euclid(360.0);
    let s = delta / max;
    let v = max;
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r1, g1, b1) = match h as u32 / 60 {
        0 => (c, x, 0.0), 1 => (x, c, 0.0), 2 => (0.0, c, x),
        3 => (0.0, x, c), 4 => (x, 0.0, c), _ => (c, 0.0, x),
    };
    [((r1 + m) * 255.0) as u8, ((g1 + m) * 255.0) as u8, ((b1 + m) * 255.0) as u8]
}
