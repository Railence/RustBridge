//! User-added keys share drawing/hit geometry and retain per-finger ownership.
use super::*;
use layout::Placement;
use serde::{Deserialize, Serialize};

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
}

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
        }
    }
    /// Migrate a JSON object that still has the old single `"key"` field.
    pub fn migrate_old_key(key: String, layouts: [Placement; 3]) -> Self {
        let mut keys: [String; 4] = Default::default();
        keys[0] = key;
        Self { keys, custom_label: None, layouts }
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
    pub(super) fn custom_update(
        &mut self,
        id: u64,
        point: [f32; 2],
        started: bool,
        ended: bool,
    ) -> bool {
        if let Some(keys) = self.custom_fingers.get(&id).cloned() {
            if ended {
                self.custom_fingers.remove(&id);
                self.fingers.remove(&id);
                // Release each key that no other finger is still holding.
                for key in keys {
                    if !self.custom_fingers.values().any(|ks: &Vec<String>| ks.contains(&key)) {
                        self.custom_events.push((key, false));
                    }
                }
            }
            return true;
        }
        if started && let Some(index) = self.custom_at(point) {
            let active: Vec<String> = self.settings.custom_buttons[index]
                .active_keys()
                .filter(|k| *k != "key.keyboard.unknown")
                .map(String::from)
                .collect();
            if active.is_empty() {
                return false;
            }
            // Press each key that isn't already held by another finger.
            for key in &active {
                let already = self.custom_fingers.values().any(|ks: &Vec<String>| ks.contains(key));
                if !already {
                    self.custom_events.push((key.clone(), true));
                }
            }
            self.custom_fingers.insert(id, active);
            self.fingers.insert(id, (Role::Ignore, point));
            self.clicked = true;
            return true;
        }
        false
    }
    pub fn held_custom_tokens(&self) -> impl Iterator<Item = &str> {
        self.custom_fingers.values().flat_map(|ks| ks.iter().map(String::as_str))
    }
    pub fn key_events(&mut self) -> Vec<(String, bool)> {
        std::mem::take(&mut self.custom_events)
    }
    pub(super) fn clear_custom(&mut self) {
        let keys: std::collections::BTreeSet<String> = std::mem::take(&mut self.custom_fingers)
            .into_values()
            .flatten()
            .collect();
        self.custom_events
            .extend(keys.into_iter().map(|k| (k, false)));
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
            let pressed = self.custom_fingers.values().any(|ks| {
                ks.iter().any(|k| button.active_keys().any(|bk| bk == k.as_str()))
            });
            let opacity = if self.editor.is_some() {
                p.opacity.max(0.25)
            } else {
                p.opacity
            };
            if opacity <= 0.0 {
                continue;
            }
            text_button(
                b,
                a,
                rect,
                &button.label(),
                pressed,
                rgba(255, 255, 255, (opacity * 255.0) as u8),
                true,
            );
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
}
