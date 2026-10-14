//! Keyboard input operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    /// Inject a browser keyboard transition into the native X11 event stream.
    #[allow(clippy::too_many_lines)]
    pub fn inject_key(&mut self, pressed: bool, keycode: u8, state: u16) {
        self.release_stale_input_grabs();
        self.input_state = key_state_after_transition(pressed, keycode, state);
        let keymap_index = usize::from(keycode / 8);
        let keymap_bit = 1u8 << (keycode % 8);
        if let Some(byte) = self.keymap.get_mut(keymap_index) {
            if pressed {
                *byte |= keymap_bit;
            } else {
                *byte &= !keymap_bit;
            }
        }
        if let Some(slot) = self.wm_notification_focus {
            if matches!(keycode, 36 | 65 | 135) {
                if pressed { self.push_notification_event(slot, if keycode == 135 { 0x7b } else { 0x401 }, self.pointer_x, self.pointer_y); }
                return;
            }
            if keycode == 9 { self.wm_notification_focus = None; self.paint_wm_chrome(WM_TASKBAR); return; }
        }
        if pressed && keycode == 9 && self.wm_menu_drag.take().is_some() {
            if let Some(menu) = self.windows.get_mut(&WM_MENU.0) { menu.mapped = false; }
            self.wm_menu_hover_row = None;
            self.paint_wm_chrome(WM_TASKBAR);
            self.scene_upsert(WM_TASKBAR);
            self.scene_upsert(WM_MENU);
            self.update_taskbar_pointer(self.pointer_x, self.pointer_y);
            return;
        }
        if self.config.native_window_manager {
            let super_key = matches!(keycode, 133 | 134);
            if pressed && super_key { self.wm_super_pending = true; }
            if pressed && matches!(keycode, 37 | 105 | 64 | 108 | 133 | 134) && self.input_state & 0x40 != 0 && self.input_state & 0x0c != 0 {
                if !self.wm_super_chord {
                    self.wm_super_chord = true; self.wm_super_pending = false;
                    if self.input_state & 8 != 0 { self.dismiss_wm_menu(); self.wm_actions.push_back(0xffff_ff13); }
                    else { self.handle_wm_shortcut(133, 0); }
                }
                return;
            }
            if !pressed && super_key {
                if self.wm_super_pending && !self.wm_super_chord { self.handle_wm_shortcut(133, 0); }
                self.wm_super_pending = false; self.wm_super_chord = false;
                return;
            }
            if super_key { return; }
            if pressed && xkb_modifier_for_keycode(keycode) == 0 { self.wm_super_pending = false; }
        }
        if self.config.native_window_manager && self.windows.get(&WM_MENU.0).is_some_and(|menu| menu.mapped)
            && !matches!(keycode, 133 | 134)
        {
            if pressed { self.handle_wm_menu_key(keycode); }
            return;
        }
        if pressed && self.config.native_window_manager && self.handle_wm_shortcut(keycode, state) {
            return;
        }
        let source = if self.map_state(self.focus_window) == 2 {
            self.focus_window
        } else {
            ROOT_WINDOW
        };
        if !self.emit_xinput_device_event(
            source,
            if pressed { 2 } else { 3 },
            u32::from(keycode),
            self.pointer_x,
            self.pointer_y,
            state,
            false,
        ).is_empty() {
            // XI2 master delivery wins; core events are the fallback, never
            // a second representation of the same physical key transition.
            return;
        }
        if let Some(grab) = self.keyboard_grab {
            let event_window = if grab.owner_events
                && self
                    .clients
                    .get(&grab.owner)
                    .and_then(|client| client.event_masks.get(&source.0))
                    .is_some_and(|mask| {
                        mask & if pressed {
                            KEY_PRESS_MASK
                        } else {
                            KEY_RELEASE_MASK
                        } != 0
                    }) {
                source
            } else {
                grab.window
            };
            let (origin_x, origin_y) = self.window_origin(event_window);
            let root_x = self.pointer_x;
            let root_y = self.pointer_y;
            let event_x = clamp_i16(i32::from(root_x) - origin_x);
            let event_y = clamp_i16(i32::from(root_y) - origin_y);
            let time = self.next_timestamp();
            self.emit_to_client(ClientHandle(grab.owner), |output, sequence, order| {
                x11::encode_key_event(
                    output,
                    order,
                    x11::KeyEvent {
                        pressed,
                        keycode,
                        sequence,
                        time,
                        root: ROOT_WINDOW,
                        event: event_window,
                        root_x,
                        root_y,
                        event_x,
                        event_y,
                        state,
                    },
                );
            });
            return;
        }
        let Some((event_window, _, targets)) = self.propagated_targets(
            source,
            if pressed {
                KEY_PRESS_MASK
            } else {
                KEY_RELEASE_MASK
            },
        ) else {
            return;
        };
        let (origin_x, origin_y) = self.window_origin(event_window);
        let root_x = self.pointer_x;
        let root_y = self.pointer_y;
        let event_x = clamp_i16(i32::from(root_x) - origin_x);
        let event_y = clamp_i16(i32::from(root_y) - origin_y);
        let time = self.next_timestamp();
        for target in targets {
            self.emit_to_client(target, |output, sequence, order| {
                x11::encode_key_event(
                    output,
                    order,
                    x11::KeyEvent {
                        pressed,
                        keycode,
                        sequence,
                        time,
                        root: ROOT_WINDOW,
                        event: event_window,
                        root_x,
                        root_y,
                        event_x,
                        event_y,
                        state,
                    },
                );
            });
        }
    }

    pub(super) fn release_stale_input_grabs(&mut self) {
        if self
            .pointer_grab
            .is_some_and(|grab| self.map_state(grab.window) != 2)
        {
            self.pointer_grab = None;
        }
        if self
            .keyboard_grab
            .is_some_and(|grab| self.map_state(grab.window) != 2)
        {
            self.keyboard_grab = None;
        }
    }
}
