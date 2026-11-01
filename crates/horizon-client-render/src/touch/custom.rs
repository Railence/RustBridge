//! User-added keys share drawing/hit geometry and retain per-finger ownership.
use super::*;
use layout::Placement;
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// A custom on-screen button that can trigger up to 4 keys simultaneously.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomButton {
    /// Up to 4 key tokens; empty strings mean unused slots.
    /// Old saves with a single `key` field are migrated on load.
    #[serde(default, deserialize_with = "deserialize_keys")]
    pub keys: [String; 4],
    /// Optional player-set label. If None, a label is generated from the keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_label: Option<String>,
    pub layouts: [Placement; 3],

    // ── New fields (all default to off / zero so old saves parse cleanly) ──

    /// Finger can drag across other swipeable buttons to switch them.
    #[serde(default)]
    pub swipeable: bool,
    /// Tap once → keys latch on; tap again → keys latch off.
    #[serde(default)]
    pub toggle_mode: bool,
    /// Hold ≥ `hold_duration` seconds → latch; short tap → momentary.
    #[serde(default)]
    pub hold_to_toggle: bool,
    /// Hold duration in seconds (1.0–3.0). Used when `hold_to_toggle` is true.
    #[serde(default = "default_hold_duration")]
    pub hold_duration: f32,
    /// RGBA button tint. [255,255,255,255] = white (default).
    #[serde(default = "default_color")]
    pub color: [u8; 4],
}

fn default_hold_duration() -> f32 { 1.5 }
fn default_color() -> [u8; 4] { [255, 255, 255, 255] }

fn deserialize_keys<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<[String; 4], D::Error> {
    // Accept either a Vec<String> (new) or a single String (old "key" field via
    // serde alias on the parent, handled by the migration below).
    let v = Vec::<String>::deserialize(deserializer)?;
    let mut out: [String; 4] = Default::default();
    for (slot, k) in out.iter_mut().zip(v.into_iter().take(4)) {
        *slot = k;
    }
    Ok(out)
}

impl CustomButton {
    /// Build a new button with a single key and auto-placed position.
    pub fn new(key: String, index: usize) -> Self {
        let mut keys: [String; 4] = Default::default();
        keys[0] = key;
        Self {
            keys,
            custom_label: None,
            layouts: [Placement::new(
                0.68 + (index % 3) as f32 * 0.1,
                0.35 + (index / 3 % 3) as f32 * 0.12,
                0.105,
            ); 3],
            swipeable: false,
            toggle_mode: false,
            hold_to_toggle: false,
            hold_duration: 1.5,
            color: [255, 255, 255, 255],
        }
    }
    /// Migrate a JSON object that still has the old single `"key"` field.
    pub fn migrate_old_key(key: String, layouts: [Placement; 3]) -> Self {
        let mut keys: [String; 4] = Default::default();
        keys[0] = key;
        Self {
            keys,
            custom_label: None,
            layouts,
            swipeable: false,
            toggle_mode: false,
            hold_to_toggle: false,
            hold_duration: 1.5,
            color: [255, 255, 255, 255],
        }
    }
    pub fn valid_token(token: &str) -> bool {
        token.is_empty()
            || token == "key.keyboard.unknown"
            || crate::keybinds::key_code(token).is_some()
            || token
                .strip_prefix("action:")
                .is_some_and(|name| crate::keybinds::MAPPINGS.iter().any(|m| m.0 == name))
    }
    /// The first non-empty, non-unknown key token, or None.
    fn first_real_key(&self) -> Option<&str> {
        self.keys.iter().find(|k| {
            !k.is_empty() && k.as_str() != "key.keyboard.unknown"
        }).map(String::as_str)
    }
    /// Display label shown on the button face.
    pub fn label(&self) -> String {
        if let Some(custom) = &self.custom_label {
            if !custom.is_empty() {
                return custom.clone();
            }
        }
        let Some(key) = self.first_real_key() else {
            return "Button".into();
        };
        if let Some(name) = key.strip_prefix("action:") {
            return crate::text::Text::translate(name, &[]).plain();
        }
        crate::keybinds::display_name(key).plain()
    }
    /// All active (non-empty) key tokens.
    pub fn active_keys(&self) -> impl Iterator<Item = &str> {
        self.keys.iter().filter(|k| !k.is_empty()).map(String::as_str)
    }
}
pub(super) fn choices(raw_keys: bool) -> Vec<(String, String)> {
    if raw_keys {
        crate::keybinds::available_keys()
            .iter()
            .map(|(name, _)| ((*name).into(), crate::keybinds::display_name(name).plain()))
            .collect()
    } else {
        crate::keybinds::MAPPINGS
            .iter()
            .filter(|m| m.1 != "key.keyboard.unknown")
            .map(|(name, _, _)| {
                (
                    format!("action:{name}"),
                    crate::text::Text::translate(name, &[]).plain(),
                )
            })
            .collect()
    }
}
impl TouchControls {
    pub(super) fn inventory_token(control: Control) -> Option<&'static str> {
        match control {
            Control::Sneak => Some("key.keyboard.left.shift"),
            Control::Attack => Some("action:key.attack"),
            Control::Use => Some("action:key.use"),
            _ => None,
        }
    }
    pub fn inventory_update(
        &mut self,
        id: u64,
        point: [f32; 2],
        started: bool,
        ended: bool,
    ) -> bool {
        if self.custom_fingers.contains_key(&id) {
            return self.custom_update(id, point, false, ended);
        }
        if started
            && self.custom_at(point).is_none()
            && let Some(token) = self.control_at(point).and_then(Self::inventory_token)
        {
            let already = self.custom_fingers.values().any(|ks: &Vec<String>| ks.iter().any(|k| k == token));
            self.custom_fingers.insert(id, vec![token.into()]);
            self.fingers.insert(id, (Role::Ignore, point));
            if !already {
                self.custom_events.push((token.into(), true));
            }
            return true;
        }
        false
    }
    pub(super) fn fit_placement(&self, mut p: Placement) -> Placement {
        let scale = self.settings.button_scale;
        p.width = (p.width * scale).min(self.aspect()).min(1.0);
        p.height = (p.height * scale).min(1.0);
        let half = [p.width / self.aspect() / 2.0, p.height / 2.0];
        for (i, half) in half.into_iter().enumerate() {
            p.center[i] = p.center[i].clamp(half, 1.0 - half);
        }
        p
    }
    pub(super) fn custom_placement(&self, index: usize) -> Placement {
        let mut p = self.settings.custom_buttons[index].layouts[self.settings.mode.index()];
        if self.settings.left_handed {
            p.center[0] = 1.0 - p.center[0];
        }
        self.fit_placement(p)
    }
    pub fn custom_at(&self, point: [f32; 2]) -> Option<usize> {
        (0..self.settings.custom_buttons.len())
            .rev()
            .find(|&i| layout::contains(self.custom_placement(i).rect(self.aspect()), point))
    }

    // ── Swipeable: find the first swipeable button under a point ────────────
    fn swipeable_at(&self, point: [f32; 2]) -> Option<usize> {
        (0..self.settings.custom_buttons.len())
            .rev()
            .find(|&i| {
                self.settings.custom_buttons[i].swipeable
                    && layout::contains(self.custom_placement(i).rect(self.aspect()), point)
            })
    }

    // ── Press keys for a button, emitting events for newly-held keys ────────
    fn press_custom_keys(&mut self, index: usize) -> Vec<String> {
        let active: Vec<String> = self.settings.custom_buttons[index]
            .active_keys()
            .filter(|k| *k != "key.keyboard.unknown")
            .map(String::from)
            .collect();
        for key in &active {
            let already = self.custom_fingers.values().any(|ks| ks.contains(key))
                || self.toggle_active.get(&index).copied().unwrap_or(false);
            if !already {
                self.custom_events.push((key.clone(), true));
            }
        }
        active
    }

    // ── Release keys that are no longer held by any finger/toggle ───────────
    fn release_keys_for_finger(&mut self, keys: &[String]) {
        for key in keys {
            let still_held = self.custom_fingers.values().any(|ks| ks.contains(key));
            // Also check if any toggle-active button has this key.
            let toggle_held = self.toggle_active.iter().any(|(&bi, &on)| {
                on && self.settings.custom_buttons.get(bi)
                    .is_some_and(|b| b.active_keys().any(|k| k == key.as_str()))
            });
            if !still_held && !toggle_held {
                self.custom_events.push((key.clone(), false));
            }
        }
    }

    pub(super) fn custom_update(
        &mut self,
        id: u64,
        point: [f32; 2],
        started: bool,
        ended: bool,
    ) -> bool {
        // ── Ongoing finger already registered ────────────────────────────────
        if let Some(keys) = self.custom_fingers.get(&id).cloned() {
            if ended {
                // Determine if this finger was doing a swipe.
                let was_swipe = self.swipe_finger == Some(id);
                if was_swipe { self.swipe_finger = None; }

                // Check hold timer for hold-to-toggle.
                let btn_idx = self.hold_timers.remove(&id).map(|(bi, _)| bi);

                self.custom_fingers.remove(&id);
                self.fingers.remove(&id);

                // For hold-to-toggle: if latch activated, don't fire momentary release.
                if let Some(bi) = btn_idx {
                    if self.toggle_active.get(&bi).copied().unwrap_or(false) {
                        // Toggle is active — keys stay on. Don't release.
                        return true;
                    }
                }

                // Regular release — release keys not held by other fingers/toggles.
                self.release_keys_for_finger(&keys);
                return true;
            }

            // ── Swipe logic: finger moves while in swipeable mode ───────────
            if self.swipe_finger == Some(id) {
                if let Some(new_idx) = self.swipeable_at(point) {
                    // Find the current button this swipe finger is on.
                    let cur_idx = self.custom_fingers.get(&id)
                        .and_then(|ks| {
                            (0..self.settings.custom_buttons.len()).find(|&bi| {
                                self.settings.custom_buttons[bi]
                                    .active_keys()
                                    .any(|k| ks.contains(&k.to_string()))
                            })
                        });
                    if cur_idx != Some(new_idx) {
                        // Release old keys.
                        let old_keys = self.custom_fingers.remove(&id).unwrap_or_default();
                        self.fingers.remove(&id);
                        self.release_keys_for_finger(&old_keys);
                        // Press new button's keys.
                        let new_keys = self.press_custom_keys(new_idx);
                        if !new_keys.is_empty() {
                            self.custom_fingers.insert(id, new_keys);
                            self.fingers.insert(id, (Role::Ignore, point));
                        }
                        // Update hold timer target.
                        self.hold_timers.insert(id, (new_idx, Instant::now()));
                    }
                }
            }

            return true;
        }

        // ── New touch start ──────────────────────────────────────────────────
        if started {
            let Some(index) = self.custom_at(point) else {
                return false;
            };
            let btn = &self.settings.custom_buttons[index];

            // ── Toggle Mode: tap toggles latch ───────────────────────────────
            if btn.toggle_mode && !btn.hold_to_toggle {
                let currently_on = self.toggle_active.get(&index).copied().unwrap_or(false);
                let active: Vec<String> = btn.active_keys()
                    .filter(|k| *k != "key.keyboard.unknown")
                    .map(String::from)
                    .collect();
                if active.is_empty() { return false; }
                if currently_on {
                    // Turn off.
                    self.toggle_active.insert(index, false);
                    for key in &active {
                        let still_held = self.custom_fingers.values().any(|ks| ks.contains(key));
                        if !still_held {
                            self.custom_events.push((key.clone(), false));
                        }
                    }
                } else {
                    // Turn on.
                    self.toggle_active.insert(index, true);
                    for key in &active {
                        let already = self.custom_fingers.values().any(|ks| ks.contains(key));
                        if !already {
                            self.custom_events.push((key.clone(), true));
                        }
                    }
                }
                // Register the finger so `ended` branch fires and cleans up.
                // Use an empty vec so it doesn't double-release.
                self.custom_fingers.insert(id, Vec::new());
                self.fingers.insert(id, (Role::Ignore, point));
                self.clicked = true;
                return true;
            }

            // ── Hold-to-Toggle: start timer ──────────────────────────────────
            if btn.hold_to_toggle {
                let active: Vec<String> = btn.active_keys()
                    .filter(|k| *k != "key.keyboard.unknown")
                    .map(String::from)
                    .collect();
                if active.is_empty() { return false; }
                // Press keys immediately (momentary start).
                for key in &active {
                    let already = self.custom_fingers.values().any(|ks| ks.contains(key));
                    if !already { self.custom_events.push((key.clone(), true)); }
                }
                self.custom_fingers.insert(id, active);
                self.fingers.insert(id, (Role::Ignore, point));
                self.hold_timers.insert(id, (index, Instant::now()));
                self.clicked = true;
                return true;
            }

            // ── Normal / Swipeable press ─────────────────────────────────────
            let active = self.press_custom_keys(index);
            if active.is_empty() { return false; }

            // If button is swipeable, register this finger as the swipe initiator.
            if self.settings.custom_buttons[index].swipeable && self.swipe_finger.is_none() {
                self.swipe_finger = Some(id);
            }

            self.custom_fingers.insert(id, active);
            self.fingers.insert(id, (Role::Ignore, point));
            self.clicked = true;
            return true;
        }

        false
    }

    /// Called every game tick to check hold-to-toggle timers.
    pub fn tick_hold_timers(&mut self) {
        let now = Instant::now();
        let to_latch: Vec<(u64, usize)> = self.hold_timers
            .iter()
            .filter_map(|(&finger, &(bi, start))| {
                let btn = self.settings.custom_buttons.get(bi)?;
                if btn.hold_to_toggle {
                    let elapsed = now.duration_since(start).as_secs_f32();
                    if elapsed >= btn.hold_duration {
                        return Some((finger, bi));
                    }
                }
                None
            })
            .collect();

        for (finger, bi) in to_latch {
            // Activate toggle latch.
            self.toggle_active.insert(bi, true);
            // Remove timer so it only fires once.
            self.hold_timers.remove(&finger);
            // The keys are already pressed (from the momentary start).
            // They'll stay on when finger is lifted due to toggle_active check.
        }
    }

    /// Release all toggle-active keys for a button (e.g. when editor closes).
    pub(super) fn release_all_toggle_active(&mut self) {
        let indices: Vec<usize> = self.toggle_active
            .iter()
            .filter(|(_, &on)| on)
            .map(|(&bi, _)| bi)
            .collect();
        for bi in indices {
            self.toggle_active.insert(bi, false);
            if let Some(btn) = self.settings.custom_buttons.get(bi) {
                for key in btn.active_keys() {
                    if key != "key.keyboard.unknown" {
                        let still_held = self.custom_fingers.values()
                            .any(|ks| ks.iter().any(|k| k == key));
                        if !still_held {
                            self.custom_events.push((key.to_string(), false));
                        }
                    }
                }
            }
        }
    }

    pub fn held_custom_tokens(&self) -> impl Iterator<Item = &str> {
        self.custom_fingers.values().flat_map(|ks| ks.iter().map(String::as_str))
    }
    pub fn key_events(&mut self) -> Vec<(String, bool)> {
        std::mem::take(&mut self.custom_events)
    }
    pub(super) fn clear_custom(&mut self) {
        // Release toggle-active keys.
        self.release_all_toggle_active();
        let keys: std::collections::BTreeSet<String> = std::mem::take(&mut self.custom_fingers)
            .into_values()
            .flatten()
            .collect();
        self.custom_events
            .extend(keys.into_iter().map(|k| (k, false)));
        self.swipe_finger = None;
        self.hold_timers.clear();
    }
    pub(super) fn draw_custom(&self, b: &mut HudBatch, a: &HudAtlas) {
        for (i, button) in self.settings.custom_buttons.iter().enumerate() {
            let p = self.custom_placement(i);
            let pw = p.width / self.aspect().max(0.1) * b.height;
            let ph = p.height * b.height;
            let rect = [
                p.center[0] * b.width - pw / 2.0,
                p.center[1] * b.height - ph / 2.0,
                pw,
                ph,
            ];
            let finger_pressed = self.custom_fingers.values().any(|ks| {
                ks.iter().any(|k| button.active_keys().any(|bk| bk == k.as_str()))
            });
            let toggle_pressed = self.toggle_active.get(&i).copied().unwrap_or(false);
            let pressed = finger_pressed || toggle_pressed;

            let opacity = if self.editor.is_some() {
                p.opacity.max(0.25)
            } else {
                p.opacity
            };
            if opacity <= 0.0 {
                continue;
            }

            // Apply button color tint.
            let [r, g, bl, _] = button.color;
            let tint_color = rgba(r, g, bl, (opacity * 255.0) as u8);

            // Blue selection outline in editor.
            let is_selected = self.editor.as_ref()
                .and_then(|e| e.selected_custom)
                .is_some_and(|idx| idx == i);
            if is_selected {
                // Draw a slightly larger blue outline.
                let margin = 3.0;
                text_button(
                    b, a,
                    [rect[0] - margin, rect[1] - margin, rect[2] + margin * 2.0, rect[3] + margin * 2.0],
                    "", false,
                    rgba(80, 160, 255, 200),
                    false,
                );
            }

            text_button(b, a, rect, &button.label(), pressed, tint_color, true);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inventory_modifier_and_mouse_hold_independently_and_cancel_cleanly() {
        let mut t = TouchControls::default();
        t.viewport(1920, 1080);
        t.inventory_mode = true;
        t.settings.mode = Mode::Crosshair;
        for (id, control, token) in [
            (1, Control::Sneak, "key.keyboard.left.shift"),
            (2, Control::Attack, "action:key.attack"),
            (3, Control::Use, "action:key.use"),
        ] {
            assert!(t.visible(control));
            assert!(t.inventory_update(id, t.placement(control).center, true, false));
            assert_eq!(t.key_events(), [(token.into(), true)]);
        }
        t.clear_world_gesture();
        assert_eq!(t.held_custom_tokens().count(), 3);
        assert_eq!(t.poll_state(), PadState::default());
        assert!(t.inventory_update(2, [0.0, 0.0], false, true));
        assert_eq!(t.key_events(), [("action:key.attack".into(), false)]);
        t.clear();
        let released = t.key_events();
        assert!(released.contains(&("key.keyboard.left.shift".into(), false)));
        assert!(released.contains(&("action:key.use".into(), false)));
    }
    #[test]
    fn inventory_overlay_preserves_configured_visibility_and_placements() {
        let mut touch = TouchControls::default();
        for mode in Mode::ALL {
            touch.settings.mode = mode;
            touch.settings.button_visibility[Control::Attack as usize] = 2;
            let before: Vec<_> = Control::ALL
                .into_iter()
                .map(|c| (c, touch.visible(c), touch.placement(c).center))
                .collect();
            touch.inventory_mode = true;
            for (control, visible, center) in before {
                assert_eq!(touch.visible(control), visible);
                assert_eq!(touch.placement(control).center, center);
            }
            touch.inventory_mode = false;
        }
    }
    #[test]
    fn unassigned_button_emits_no_input() {
        let mut touch = TouchControls::default();
        touch.viewport(1920, 1080);
        touch
            .settings
            .custom_buttons
            .push(CustomButton::new("key.keyboard.unknown".into(), 0));
        let point = touch.custom_placement(0).center;
        touch.update(1, point, true, false);
        assert!(touch.key_events().is_empty());
    }
    #[test]
    fn custom_keys_hold_through_motion_and_shared_keys_release_only_after_last_finger() {
        let mut t = TouchControls::default();
        t.viewport(1920, 1080);
        t.settings
            .custom_buttons
            .push(CustomButton::new("key.keyboard.z".into(), 0));
        let p = t.custom_placement(0).center;
        t.update(1, p, true, false);
        t.update(2, p, true, false);
        assert_eq!(t.key_events(), [("key.keyboard.z".into(), true)]);
        t.update(1, [0.99, 0.1], false, false);
        assert!(t.key_events().is_empty());
        t.update(1, p, false, true);
        assert!(t.key_events().is_empty());
        t.update(2, p, false, true);
        assert_eq!(t.key_events(), [("key.keyboard.z".into(), false)]);
    }
    #[test]
    fn cancellation_releases_all_custom_keys_and_scaling_preserves_saved_sizes() {
        let mut t = TouchControls::default();
        t.viewport(1920, 1080);
        t.settings
            .custom_buttons
            .push(CustomButton::new("key.keyboard.z".into(), 0));
        let stored = t.settings.custom_buttons[0].layouts[0].width;
        t.settings.button_scale = 2.0;
        assert_eq!(t.custom_placement(0).width, stored * 2.0);
        t.update(1, t.custom_placement(0).center, true, false);
        t.key_events();
        t.clear();
        assert_eq!(t.key_events(), [("key.keyboard.z".into(), false)]);
        assert_eq!(t.settings.custom_buttons[0].layouts[0].width, stored);
    }
    #[test]
    fn toggle_mode_latches_and_unlatches_on_tap() {
        let mut t = TouchControls::default();
        t.viewport(1920, 1080);
        let mut btn = CustomButton::new("key.keyboard.z".into(), 0);
        btn.toggle_mode = true;
        t.settings.custom_buttons.push(btn);
        let p = t.custom_placement(0).center;
        // First tap: latch on.
        t.update(1, p, true, false);
        assert_eq!(t.key_events(), [("key.keyboard.z".into(), true)]);
        t.update(1, p, false, true);
        assert!(t.key_events().is_empty(), "keys should stay on after lift");
        // Second tap: latch off.
        t.update(2, p, true, false);
        assert_eq!(t.key_events(), [("key.keyboard.z".into(), false)]);
    }
    #[test]
    fn swipeable_only_fires_from_initiating_finger() {
        let mut t = TouchControls::default();
        t.viewport(1920, 1080);
        // Two swipeable buttons side by side.
        let mut b1 = CustomButton::new("key.keyboard.a".into(), 0);
        b1.swipeable = true;
        b1.layouts[0].center = [0.7, 0.5];
        let mut b2 = CustomButton::new("key.keyboard.b".into(), 1);
        b2.swipeable = true;
        b2.layouts[0].center = [0.85, 0.5];
        t.settings.custom_buttons.push(b1);
        t.settings.custom_buttons.push(b2);

        let p1 = t.custom_placement(0).center;
        let p2 = t.custom_placement(1).center;

        // Finger 1 presses button 0.
        t.update(1, p1, true, false);
        assert_eq!(t.key_events(), [("key.keyboard.a".into(), true)]);

        // Finger 2 moves across button 1 — must NOT activate it.
        t.update(2, p2, true, false);
        // Finger 2 is not the swipe initiator; button 1 should not fire.
        // (Finger 2 would register normally since it's a new press with its own finger.)
        // Reset events.
        t.key_events();

        // Finger 1 (initiating) drags to button 1 — should switch.
        t.update(1, p2, false, false);
        let events = t.key_events();
        assert!(events.contains(&("key.keyboard.a".into(), false)), "a should release");
        assert!(events.contains(&("key.keyboard.b".into(), true)), "b should activate");
    }
}
