//! Input events operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn emit_pointer_input(
        &mut self,
        input: PointerInput,
        only_owner: Option<u32>,
        encode: impl Fn(&mut Vec<u8>, ClientByteOrder, x11::PointerEvent),
    ) {
        if let Some(grab) = self.pointer_grab {
            if only_owner.is_some_and(|owner| owner != grab.owner) {
                return;
            }
            if u32::from(grab.event_mask) & input.mask == 0 {
                return;
            }
            let natural = self
                .clients
                .get(&grab.owner)
                .and_then(|client| client.event_masks.get(&input.source.0))
                .is_some_and(|mask| mask & input.mask != 0);
            let event_window = if grab.owner_events && natural {
                input.source
            } else {
                grab.window
            };
            let (origin_x, origin_y) = self.window_origin(event_window);
            let event_x = clamp_i16(i32::from(input.root_x) - origin_x);
            let event_y = clamp_i16(i32::from(input.root_y) - origin_y);
            let time = self.next_timestamp();
            self.emit_to_client(ClientHandle(grab.owner), |output, sequence, order| {
                encode(
                    output,
                    order,
                    x11::PointerEvent {
                        sequence,
                        detail: input.detail,
                        time,
                        root: ROOT_WINDOW,
                        event: event_window,
                        child: ResourceId(0),
                        root_x: input.root_x,
                        root_y: input.root_y,
                        event_x,
                        event_y,
                        state: input.state,
                    },
                );
            });
            return;
        }
        let Some((event_window, child, targets)) =
            self.propagated_targets(input.source, input.mask)
        else {
            return;
        };
        let (origin_x, origin_y) = self.window_origin(event_window);
        let event_x = clamp_i16(i32::from(input.root_x) - origin_x);
        let event_y = clamp_i16(i32::from(input.root_y) - origin_y);
        let time = self.next_timestamp();
        for target in targets.into_iter().filter(|target| only_owner.is_none_or(|owner| target.0 == owner)
        ) {
            self.emit_to_client(target, |output, sequence, order| {
                encode(
                    output,
                    order,
                    x11::PointerEvent {
                        sequence,
                        detail: input.detail,
                        time,
                        root: ROOT_WINDOW,
                        event: event_window,
                        child,
                        root_x: input.root_x,
                        root_y: input.root_y,
                        event_x,
                        event_y,
                        state: input.state,
                    },
                );
            });
        }
    }

    pub(super) fn emit_xinput_raw_pointer_event(&mut self, event_type: u16, detail: u32, dx: i32, dy: i32) {
        let event_bit = 1u32 << event_type;
        let mut deliveries = Vec::new();
        for (&client_id, client) in &self.clients {
            // Raw events belong to the root, not to a hit-tested child. A
            // client selecting AllMasterDevices must receive one delta, not
            // both the physical device's and the master's copy.
            for (device, selectors) in [(2u16, &[0u16, 1, 2][..]), (4, &[0u16, 4][..])] {
                if selectors.iter().any(|selector| {
                    client.xi2_masks
                    .get(&(ROOT_WINDOW.0, *selector))
                    .is_some_and(|mask| mask & event_bit != 0)}) {
                    deliveries.push((ClientHandle(client_id), device));
                }
            }
        }
        if deliveries.is_empty() { return; }
        let time = self.next_timestamp();
        for (client, device) in deliveries {
            self.emit_to_client(client, |output, sequence, order| {
                x11::encode_xi2_raw_event(output, order, sequence, XINPUT_MAJOR_OPCODE,
                    event_type, device, time, detail, 4, dx, dy,
                );
            });
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn emit_xinput_device_event(
        &mut self,
        source: ResourceId,
        event_type: u16,
        detail: u32,
        root_x: i16,
        root_y: i16,
        state: u16,
        pointer: bool,
    ) -> BTreeSet<u32> {
        let event_bit = 1u32.checked_shl(u32::from(event_type)).unwrap_or(0);
        if event_bit == 0 {
            return BTreeSet::new();
        }
        let selectors: &[u16] = if pointer {
            &[2, 4, 1, 0]
        } else {
            &[3, 5, 1, 0]
        };
        let mut deliveries = Vec::new();
        let core_mask = match event_type {
            2 => KEY_PRESS_MASK,
            3 => KEY_RELEASE_MASK,
            4 => BUTTON_PRESS_MASK,
            5 => BUTTON_RELEASE_MASK,
            6 => POINTER_MOTION_MASK | motion_mask_for_state(state),
            _ => 0,
        };
        let grab = if pointer {
            self.pointer_grab.map(|grab| (grab.owner, grab.window, grab.owner_events))
        } else {
            self.keyboard_grab.map(|grab| (grab.owner, grab.window, grab.owner_events))
        };
        let mut current = grab.filter(|grab| !grab.2).map_or(source, |grab| grab.1);
        let mut child = ResourceId(0);
        for _ in 0..256 {
            for (&client_id, client) in &self.clients {
                if grab.is_some_and(|grab| grab.0 != client_id) { continue; }
                if selectors.iter().any(|device| {
                    client.xi2_masks.get(&(current.0, *device))
                    .is_some_and(|mask| mask & event_bit != 0)}) {
                    deliveries.push((ClientHandle(client_id), current, child));
                }
            }
            // XI2proto event processing: try XI2 then core at W before P.
            if !deliveries.is_empty() { break; }
            if self.clients.iter().any(|(&id, client)|
                {
                grab.is_none_or(|grab| grab.0 == id) && client.event_masks.get(&current.0)
                    .is_some_and(|mask| mask & core_mask != 0)}) { return BTreeSet::new(); }
            let Some(window) = self.windows.get(&current.0) else { break; };
            if current == ROOT_WINDOW || window.parent == current { break; }
            child = current;
            current = window.parent;
        }
        if deliveries.is_empty() {
            return BTreeSet::new();
        }
        let time = self.next_timestamp();
        let device_id = if pointer { 2 } else { 3 };
        let source_id = if pointer { 4 } else { 5 };
        let delivered_clients = deliveries.iter().map(|delivery| delivery.0.0).collect();
        for (client, event_window, child) in deliveries {
            let (origin_x, origin_y) = self.window_origin(event_window);
            let event_x = clamp_i16(i32::from(root_x).saturating_sub(origin_x));
            let event_y = clamp_i16(i32::from(root_y).saturating_sub(origin_y));
            self.emit_to_client(client, |output, sequence, order| {
                x11::encode_xi2_device_event(
                    output,
                    order,
                    sequence,
                    XINPUT_MAJOR_OPCODE,
                    event_type,
                    device_id,
                    time,
                    ROOT_WINDOW,
                    event_window,
                    child,
                    root_x,
                    root_y,
                    event_x,
                    event_y,
                    state,
                    detail,
                    source_id,
                    0,
                );
            });
        }
        delivered_clients
    }

    /// Resolve the X client that owns the hit-tested resource. Compositor
    /// frames are server-owned, while their client descendants carry the
    /// connection identity used for per-client core/XI2 suppression.
    pub(super) fn input_resource_owner(&self, mut source: ResourceId) -> Option<u32> {
        for _ in 0..256 {
            let window = self.windows.get(&source.0)?;
            if let Some(owner) = window.owner { return Some(owner); }
            if source == ROOT_WINDOW || window.parent == source { return None; }
            source = window.parent;
        }
        None
    }

    pub(super) fn emit_xinput_crossing(
        &mut self,
        window: ResourceId,
        entering: bool,
        root_x: i16,
        root_y: i16,
        state: u16,
    ) {
        let event_type = if entering { 7 } else { 8 };
        let event_bit = 1u32 << event_type;
        let targets: Vec<ClientHandle> = self
            .clients
            .iter()
            .filter_map(|(&client_id, client)| {
                [2u16, 4, 1, 0]
                    .iter()
                    .any(|device| {
                        client
                            .xi2_masks
                            .get(&(window.0, *device))
                            .is_some_and(|mask| mask & event_bit != 0)
                    })
                    .then_some(ClientHandle(client_id))
            })
            .collect();
        if targets.is_empty() {
            return;
        }
        let (origin_x, origin_y) = self.window_origin(window);
        let event_x = clamp_i16(i32::from(root_x).saturating_sub(origin_x));
        let event_y = clamp_i16(i32::from(root_y).saturating_sub(origin_y));
        let time = self.next_timestamp();
        for target in targets {
            self.emit_to_client(target, |output, sequence, order| {
                x11::encode_xi2_crossing_event(
                    output,
                    order,
                    sequence,
                    XINPUT_MAJOR_OPCODE,
                    event_type,
                    2,
                    time,
                    ROOT_WINDOW,
                    window,
                    root_x,
                    root_y,
                    event_x,
                    event_y,
                    state,
                    0,
                    3,
                    4,
                );
            });
        }
    }

    pub(super) fn emit_xinput_focus(&mut self, window: ResourceId, focused: bool) {
        let event_type = if focused { 9 } else { 10 };
        let event_bit = 1u32 << event_type;
        let targets: Vec<ClientHandle> = self
            .clients
            .iter()
            .filter_map(|(&client_id, client)| {
                [3u16, 5, 1, 0]
                    .iter()
                    .any(|device| {
                        client
                            .xi2_masks
                            .get(&(window.0, *device))
                            .is_some_and(|mask| mask & event_bit != 0)
                    })
                    .then_some(ClientHandle(client_id))
            })
            .collect();
        if targets.is_empty() {
            return;
        }
        let root_x = self.pointer_x;
        let root_y = self.pointer_y;
        let (origin_x, origin_y) = self.window_origin(window);
        let event_x = clamp_i16(i32::from(root_x).saturating_sub(origin_x));
        let event_y = clamp_i16(i32::from(root_y).saturating_sub(origin_y));
        let time = self.next_timestamp();
        for target in targets {
            self.emit_to_client(target, |output, sequence, order| {
                x11::encode_xi2_focus_event(
                    output,
                    order,
                    sequence,
                    XINPUT_MAJOR_OPCODE,
                    event_type,
                    3,
                    time,
                    window,
                    root_x,
                    root_y,
                    event_x,
                    event_y,
                    0,
                    3,
                );
            });
        }
    }
}
