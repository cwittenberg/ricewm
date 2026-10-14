//! Sync operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    #[allow(clippy::too_many_lines)]
    pub(super) fn sync_request(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        minor: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let result = self.sync_request_inner(handle, order, sequence, minor, body);
        result.map_err(|error| extension_error(error, minor))
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn sync_request_inner(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        minor: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        match minor {
            x11sync::INITIALIZE => {
                if body.len() != 4 {
                    return Err(bad_length());
                }
                let major = x11sync::MAJOR_VERSION.min(body[0]);
                let minor = if major < x11sync::MAJOR_VERSION {
                    body[1]
                } else {
                    x11sync::MINOR_VERSION.min(body[1])
                };
                Ok(x11sync::encode_initialize_reply(
                    order, sequence, major, minor,
                ))
            }
            x11sync::LIST_SYSTEM_COUNTERS => {
                if !body.is_empty() {
                    return Err(bad_length());
                }
                Ok(x11sync::encode_list_system_counters_reply(order, sequence))
            }
            x11sync::CREATE_COUNTER => {
                let (counter, value) = decode_sync_counter_value(order, body)?;
                self.validate_new_extension_resource(handle, ResourceId(counter))?;
                self.sync_counters.insert(
                    counter,
                    SyncCounter {
                        owner: handle.0,
                        value,
                    },
                );
                Ok(Vec::new())
            }
            x11sync::SET_COUNTER | x11sync::CHANGE_COUNTER => {
                let (counter, value) = decode_sync_counter_value(order, body)?;
                let counter_state = self
                    .sync_counters
                    .get_mut(&counter)
                    .ok_or_else(|| ProtocolError::new(SYNC_FIRST_ERROR, counter))?;
                let old = counter_state.value;
                counter_state.value = if minor == x11sync::CHANGE_COUNTER {
                    old.saturating_add(value)
                } else {
                    value
                };
                let new = counter_state.value;
                self.evaluate_sync_alarms(counter, old, new);
                Ok(Vec::new())
            }
            x11sync::QUERY_COUNTER => {
                let counter = decode_u32_request(order, body)?;
                let value = self.sync_counter_value(counter)?;
                Ok(x11sync::encode_query_counter_reply(order, sequence, value))
            }
            x11sync::DESTROY_COUNTER => {
                let counter = decode_u32_request(order, body)?;
                if self.sync_counters.remove(&counter).is_none() {
                    return Err(ProtocolError::new(SYNC_FIRST_ERROR, counter));
                }
                Ok(Vec::new())
            }
            x11sync::AWAIT => {
                if body.is_empty() || !body.len().is_multiple_of(28) {
                    return Err(bad_length());
                }
                // RiceWM executes requests synchronously. Counter transitions
                // are therefore visible before later requests, and an await
                // whose condition is already met completes immediately.
                Ok(Vec::new())
            }
            x11sync::CREATE_ALARM | x11sync::CHANGE_ALARM => {
                let (id, attributes) = decode_sync_alarm_attributes(order, body)?;
                let existing = self.sync_alarms.get(&id).copied();
                if minor == x11sync::CREATE_ALARM {
                    self.validate_new_extension_resource(handle, ResourceId(id))?;
                } else if existing.is_none() {
                    return Err(ProtocolError::new(SYNC_FIRST_ERROR.saturating_add(1), id));
                }
                let mut alarm = existing.unwrap_or(SyncAlarm {
                    owner: handle.0,
                    counter: 0,
                    wait_value: 0,
                    delta: 0,
                    test_type: x11sync::TEST_POSITIVE_TRANSITION,
                    events: true,
                    state: x11sync::ALARM_STATE_ACTIVE,
                });
                if let Some(counter) = attributes.counter {
                    self.sync_counter_value(counter)?;
                    alarm.counter = counter;
                }
                if let Some(test_type) = attributes.test_type {
                    if test_type > x11sync::TEST_NEGATIVE_COMPARISON {
                        return Err(ProtocolError::new(x11::error::BAD_VALUE, test_type));
                    }
                    alarm.test_type = test_type;
                }
                if let Some(delta) = attributes.delta {
                    alarm.delta = delta;
                }
                if let Some(events) = attributes.events {
                    alarm.events = events;
                }
                if attributes.value.is_some() || attributes.value_type.is_some() {
                    let value = attributes.value.unwrap_or_default();
                    alarm.wait_value =
                        if attributes.value_type == Some(x11sync::VALUE_TYPE_RELATIVE) {
                            self.sync_counter_value(alarm.counter)?
                                .saturating_add(value)
                        } else {
                            value
                        };
                }
                alarm.state = x11sync::ALARM_STATE_ACTIVE;
                self.sync_alarms.insert(id, alarm);
                let current = self.sync_counter_value(alarm.counter)?;
                self.evaluate_sync_alarms(alarm.counter, current, current);
                Ok(Vec::new())
            }
            x11sync::QUERY_ALARM => {
                let id = decode_u32_request(order, body)?;
                let alarm = self
                    .sync_alarms
                    .get(&id)
                    .ok_or_else(|| ProtocolError::new(SYNC_FIRST_ERROR.saturating_add(1), id))?;
                Ok(x11sync::encode_query_alarm_reply(
                    order,
                    sequence,
                    alarm.counter,
                    alarm.wait_value,
                    alarm.delta,
                    alarm.events,
                    alarm.state,
                ))
            }
            x11sync::DESTROY_ALARM => {
                let id = decode_u32_request(order, body)?;
                if self.sync_alarms.remove(&id).is_none() {
                    return Err(ProtocolError::new(SYNC_FIRST_ERROR.saturating_add(1), id));
                }
                Ok(Vec::new())
            }
            x11sync::SET_PRIORITY => {
                if body.len() != 8 {
                    return Err(bad_length());
                }
                let client = read_u32(order, body);
                let priority = read_i32(order, &body[4..8]);
                let owner = if client == 0 { handle.0 } else { client };
                self.client_priorities.insert(owner, priority);
                Ok(Vec::new())
            }
            x11sync::GET_PRIORITY => {
                let client = decode_u32_request(order, body)?;
                let owner = if client == 0 { handle.0 } else { client };
                Ok(x11sync::encode_get_priority_reply(
                    order,
                    sequence,
                    self.client_priorities
                        .get(&owner)
                        .copied()
                        .unwrap_or_default(),
                ))
            }
            x11sync::CREATE_FENCE => {
                if body.len() != 12 {
                    return Err(bad_length());
                }
                let drawable = ResourceId(read_u32(order, body));
                if !self.surfaces.contains_key(&drawable.0) {
                    return Err(ProtocolError::new(x11::error::BAD_DRAWABLE, drawable.0));
                }
                let id = ResourceId(read_u32(order, &body[4..8]));
                self.validate_new_extension_resource(handle, id)?;
                if body[8] > 1 {
                    return Err(ProtocolError::new(
                        x11::error::BAD_VALUE,
                        u32::from(body[8]),
                    ));
                }
                self.sync_fences.insert(
                    id.0,
                    SyncFence {
                        owner: handle.0,
                        triggered: body[8] != 0,
                    },
                );
                Ok(Vec::new())
            }
            x11sync::TRIGGER_FENCE | x11sync::RESET_FENCE => {
                let id = decode_u32_request(order, body)?;
                let fence = self
                    .sync_fences
                    .get_mut(&id)
                    .ok_or_else(|| ProtocolError::new(SYNC_FIRST_ERROR.saturating_add(2), id))?;
                fence.triggered = minor == x11sync::TRIGGER_FENCE;
                Ok(Vec::new())
            }
            x11sync::DESTROY_FENCE => {
                let id = decode_u32_request(order, body)?;
                if self.sync_fences.remove(&id).is_none() {
                    return Err(ProtocolError::new(SYNC_FIRST_ERROR.saturating_add(2), id));
                }
                Ok(Vec::new())
            }
            x11sync::QUERY_FENCE => {
                let id = decode_u32_request(order, body)?;
                let triggered = self
                    .sync_fences
                    .get(&id)
                    .ok_or_else(|| ProtocolError::new(SYNC_FIRST_ERROR.saturating_add(2), id))?
                    .triggered;
                Ok(x11sync::encode_query_fence_reply(
                    order, sequence, triggered,
                ))
            }
            x11sync::AWAIT_FENCE => {
                if body.is_empty() || !body.len().is_multiple_of(4) {
                    return Err(bad_length());
                }
                for fence in body.chunks_exact(4) {
                    let id = read_u32(order, fence);
                    if !self.sync_fences.contains_key(&id) {
                        return Err(ProtocolError::new(SYNC_FIRST_ERROR.saturating_add(2), id));
                    }
                }
                Ok(Vec::new())
            }
            _ => Err(ProtocolError::new(
                x11::error::BAD_REQUEST,
                u32::from(minor),
            )),
        }
    }

    pub(super) fn sync_counter_value(&self, counter: u32) -> Result<i64, ProtocolError> {
        if matches!(
            counter,
            x11sync::SERVERTIME_COUNTER
                | x11sync::IDLETIME_COUNTER
                | x11sync::IDLETIME_DEVICE_VCP
                | x11sync::IDLETIME_DEVICE_VCK
        ) {
            Ok(i64::from(self.timestamp))
        } else {
            self.sync_counters
                .get(&counter)
                .map(|counter| counter.value)
                .ok_or_else(|| ProtocolError::new(SYNC_FIRST_ERROR, counter))
        }
    }

    pub(super) fn evaluate_sync_alarms(&mut self, counter: u32, old: i64, new: i64) {
        let candidates: Vec<u32> = self
            .sync_alarms
            .iter()
            .filter_map(|(id, alarm)| {
                (alarm.counter == counter
                    && alarm.state == x11sync::ALARM_STATE_ACTIVE
                    && x11sync::trigger_fires(alarm.test_type, old, new, alarm.wait_value))
                .then_some(*id)
            })
            .collect();
        let timestamp = self.next_timestamp();
        for id in candidates {
            let Some(mut alarm) = self.sync_alarms.get(&id).copied() else {
                continue;
            };
            let fired_value = alarm.wait_value;
            if alarm.delta == 0
                && matches!(
                    alarm.test_type,
                    x11sync::TEST_POSITIVE_COMPARISON | x11sync::TEST_NEGATIVE_COMPARISON
                )
            {
                alarm.state = x11sync::ALARM_STATE_INACTIVE;
            } else if alarm.delta != 0 {
                for _ in 0..1_000_000 {
                    if !x11sync::comparison_satisfied(alarm.test_type, new, alarm.wait_value) {
                        break;
                    }
                    let Some(next) = alarm.wait_value.checked_add(alarm.delta) else {
                        alarm.state = x11sync::ALARM_STATE_INACTIVE;
                        break;
                    };
                    alarm.wait_value = next;
                }
            }
            self.sync_alarms.insert(id, alarm);
            if alarm.events {
                self.emit_to_client(ClientHandle(alarm.owner), |output, sequence, order| {
                    output.extend_from_slice(&x11sync::encode_alarm_notify_event(
                        order,
                        SYNC_FIRST_EVENT,
                        sequence,
                        id,
                        new,
                        fired_value,
                        timestamp,
                        alarm.state,
                    ));
                });
            }
        }
    }
}
