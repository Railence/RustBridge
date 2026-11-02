//! Extra user-placed joysticks that drive `state.left` like the built-in Move control.
use super::*;
use layout::CustomJoystick;

impl TouchControls {
    /// Draw all custom joysticks.
    pub(super) fn draw_custom_joysticks(&self, b: &mut HudBatch, a: &HudAtlas) {
        let aspect = self.aspect();
        for (i, joy) in self.settings.custom_joysticks.iter().enumerate() {
            let opacity = if self.editor.is_some() { joy.opacity.max(0.25) } else { joy.opacity };
            if opacity <= 0.0 { continue; }

            let is_selected = self.editor.as_ref()
                .and_then(|e| e.selected_joy)
                .is_some_and(|idx| idx == i);

            let [fx, fy, fw, fh] = joy.rect(aspect);
            let (bw, bh) = (b.width, b.height);
            let color = rgba(255, 255, 255, (opacity * 255.0) as u8);

            // Blue outline when selected in editor.
            if is_selected {
                let m = 3.0;
                b.fill(a, fx * bw - m, fy * bh - m, fw * bw + m * 2.0, fh * bh + m * 2.0,
                    rgba(80, 160, 255, 180));
            }

            // Frame sprite.
            sprite(b, a, "ui/joystick_frame", [fx * bw, fy * bh, fw * bw, fh * bh], color);

            // Knob: offset by current finger drag, clamped to radius.
            let knob_offset = self.joy_fingers.get(&(i as u64)).copied();
            let (kox, koy) = if let Some((ox, oy)) = knob_offset {
                let radius = joy.size / 2.0;
                let len = (ox * ox + oy * oy).sqrt();
                if len > radius { (ox / len * radius, oy / len * radius) } else { (ox, oy) }
            } else {
                (0.0, 0.0)
            };
            let knob_size = joy.size * 0.55;
            let kx = joy.center[0] + kox / aspect - knob_size / aspect / 2.0;
            let ky = joy.center[1] + koy - knob_size / 2.0;
            sprite(b, a, "ui/joystick_knob",
                [kx * bw, ky * bh, knob_size / aspect * bw, knob_size * bh], color);
        }
    }

    /// Try to claim a touch for a custom joystick.
    /// Returns true if claimed.
    pub(super) fn custom_joy_update(
        &mut self,
        id: u64,
        point: [f32; 2],
        started: bool,
        ended: bool,
    ) -> bool {
        // Existing finger?
        if let Some(&joy_idx) = self.joy_finger_to_joy.get(&id) {
            if ended {
                self.joy_fingers.remove(&(joy_idx as u64));
                self.joy_finger_to_joy.remove(&id);
            } else {
                let joy = &self.settings.custom_joysticks[joy_idx];
                self.joy_fingers.insert(joy_idx as u64,
                    [point[0] - joy.center[0], point[1] - joy.center[1]]);
            }
            return true;
        }
        if started {
            // Find a custom joystick under this point.
            let aspect = self.aspect();
            for (i, joy) in self.settings.custom_joysticks.iter().enumerate() {
                if layout::contains(joy.rect(aspect), point) {
                    self.joy_finger_to_joy.insert(id, i);
                    self.joy_fingers.insert(i as u64,
                        [point[0] - joy.center[0], point[1] - joy.center[1]]);
                    return true;
                }
            }
        }
        false
    }

    /// Contribute custom joystick movement to `state.left` (called from poll_state).
    pub(super) fn apply_custom_joy_movement(&self, state: &mut crate::gamepad::PadState) {
        if state.left != [0.0, 0.0] { return; } // built-in joystick takes priority
        let aspect = self.aspect();
        for (i, joy) in self.settings.custom_joysticks.iter().enumerate() {
            if let Some(&[ox, oy]) = self.joy_fingers.get(&(i as u64)) {
                let radius = joy.size / 2.0;
                if radius <= 0.0 { continue; }
                let mut left = [ox * aspect / radius, -oy / radius];
                let len = left[0].hypot(left[1]).max(1.0);
                left = left.map(|v| v / len);
                state.left = left;
                return; // first active joystick wins
            }
        }
    }
}

// ── Helper: sprite (plain non-nine-patch sprite) ─────────────────────────────
fn sprite(b: &mut HudBatch, a: &HudAtlas, name: &str, [x, y, w, h]: [f32; 4], color: u32) {
    // The existing draw code uses nine() for joystick_frame/knob, so we do the same.
    nine(b, a, name, 1.0, [x, y, w, h], color);
}
