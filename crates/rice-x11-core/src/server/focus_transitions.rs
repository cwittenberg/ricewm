//! Focus transitions operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn set_focus(&mut self, window: ResourceId) {
        let window = self.managed_client_ancestor(window).unwrap_or(window);
        // Focus cannot move into a disabled owner while one of its modal
        // transients is mapped. Centralizing the redirect here covers client
        // SetInputFocus, pointer activation, taskbar activation, and overview
        // activation without toolkit- or application-specific policy.
        let window = self
            .topmost_managed_modal_transient(window)
            .unwrap_or(window);
        if self.focus_window == window
            || self.map_state(window) != 2
            || self.parked_managed_windows.contains(&window.0)
        {
            return;
        }
        let previous = self.focus_window;
        self.focus_window = window;
        self.last_focus_time = self.next_timestamp();
        for (event_window, focus_in, detail) in self.focus_transition(previous, window) {
            self.emit_window_event(
                event_window,
                FOCUS_CHANGE_MASK,
                |output, sequence, order| {
                    x11::encode_focus_event_with_mode_detail(
                        output,
                        sequence,
                        order,
                        focus_in,
                        event_window,
                        0,
                        detail,
                    );
                },
            );
        }
        self.emit_xinput_focus(previous, false);
        self.emit_xinput_focus(window, true);
        self.send_wm_protocol(window, "WM_TAKE_FOCUS");
        if self.config.native_window_manager {
            self.set_wm_property(
                ROOT_WINDOW,
                "_NET_ACTIVE_WINDOW",
                "WINDOW",
                32,
                window.0.to_le_bytes().to_vec(),
            );
            if let Some(frame) = self.managed_frames.get(&previous.0).copied()
                && self.map_state(frame) == 2
            {
                self.paint_wm_chrome(frame);
            }
            if let Some(frame) = self.managed_frames.get(&window.0).copied() {
                self.restack(frame, None, Some(0));
                self.paint_wm_chrome(frame);
                self.scene_upsert_root_children();
            }
            self.paint_wm_chrome(WM_TASKBAR);
            self.scene_upsert(WM_TASKBAR);
        }
    }

    /// Compute the core FocusOut/FocusIn path for two real windows.
    ///
    /// This is the window-to-window portion of Xorg's `CoreFocusEvents`.
    /// Moving between two reparented top-level clients is nonlinear:
    /// GTK uses that distinction when changing focus between a main
    /// window and its modal file chooser.
    pub(super) fn focus_transition(
        &self,
        previous: ResourceId,
        next: ResourceId,
    ) -> Vec<(ResourceId, bool, u8)> {
        if previous == next {
            return Vec::new();
        }
        let previous_chain = self.window_ancestor_chain(previous);
        let next_chain = self.window_ancestor_chain(next);

        if previous_chain.iter().skip(1).any(|window| *window == next) {
            let mut events = vec![(previous, false, NOTIFY_ANCESTOR)];
            for intermediate in previous_chain.iter().skip(1) {
                if *intermediate == next {
                    break;
                }
                events.push((*intermediate, false, NOTIFY_VIRTUAL));
            }
            events.push((next, true, NOTIFY_INFERIOR));
            return events;
        }

        if next_chain.iter().skip(1).any(|window| *window == previous) {
            let mut events = vec![(previous, false, NOTIFY_INFERIOR)];
            let mut intermediates = Vec::new();
            for intermediate in next_chain.iter().skip(1) {
                if *intermediate == previous {
                    break;
                }
                intermediates.push(*intermediate);
            }
            events.extend(
                intermediates
                    .into_iter()
                    .rev()
                    .map(|window| (window, true, NOTIFY_VIRTUAL)),
            );
            events.push((next, true, NOTIFY_ANCESTOR));
            return events;
        }

        let common = next_chain
            .iter()
            .copied()
            .find(|candidate| previous_chain.contains(candidate));
        let mut events = vec![(previous, false, NOTIFY_NONLINEAR)];
        for intermediate in previous_chain.iter().skip(1) {
            if Some(*intermediate) == common {
                break;
            }
            events.push((*intermediate, false, NOTIFY_NONLINEAR_VIRTUAL));
        }
        let mut intermediates = Vec::new();
        for intermediate in next_chain.iter().skip(1) {
            if Some(*intermediate) == common {
                break;
            }
            intermediates.push(*intermediate);
        }
        events.extend(
            intermediates
                .into_iter()
                .rev()
                .map(|window| (window, true, NOTIFY_NONLINEAR_VIRTUAL)),
        );
        events.push((next, true, NOTIFY_NONLINEAR));
        events
    }

    pub(super) fn window_ancestor_chain(&self, start: ResourceId) -> Vec<ResourceId> {
        let mut chain = vec![start];
        let mut current = start;
        for _ in 0..256 {
            let Some(window) = self.windows.get(&current.0) else {
                break;
            };
            if window.parent == current {
                break;
            }
            chain.push(window.parent);
            current = window.parent;
        }
        chain
    }

    pub(super) fn emit_crossing(
        &mut self,
        window: ResourceId,
        entering: bool,
        root_x: i16,
        root_y: i16,
        state: u16,
    ) {
        let mask = if entering {
            ENTER_WINDOW_MASK
        } else {
            LEAVE_WINDOW_MASK
        };
        let targets: Vec<ClientHandle> = self
            .clients
            .iter()
            .filter_map(|(id, client)| {
                client
                    .event_masks
                    .get(&window.0)
                    .is_some_and(|selected| selected & mask != 0)
                    .then_some(ClientHandle(*id))
            })
            .collect();
        let (origin_x, origin_y) = self.window_origin(window);
        let event_x = clamp_i16(i32::from(root_x).saturating_sub(origin_x));
        let event_y = clamp_i16(i32::from(root_y).saturating_sub(origin_y));
        let time = self.next_timestamp();
        for target in targets {
            self.emit_to_client(target, |output, sequence, order| {
                let event = x11::CrossingEvent {
                    sequence,
                    time,
                    root: ROOT_WINDOW,
                    event: window,
                    child: ResourceId(0),
                    root_x,
                    root_y,
                    event_x,
                    event_y,
                    state,
                    detail: 3,
                    mode: 0,
                };
                if entering {
                    x11::encode_enter_notify_event(output, order, event);
                } else {
                    x11::encode_leave_notify_event(output, order, event);
                }
            });
        }
    }
}
