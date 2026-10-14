use std::collections::VecDeque;

use yserver_protocol::x11::ResourceId;

use crate::model::Window;

pub const SCENE_COMMAND_BYTES: usize = 32;
/// The scene record's stack field reserves its two high bits for managed-frame
/// capabilities; the remaining 14 bits preserve root-child order.
pub const SCENE_OWNED_WINDOW_STACK_FLAG: u16 = 0x8000;
pub const SCENE_RESIZABLE_WINDOW_STACK_FLAG: u16 = 0x4000;
pub const SCENE_RESET: u8 = 0;
pub const SCENE_UPSERT_WINDOW: u8 = 1;
pub const SCENE_REMOVE_WINDOW: u8 = 2;
pub const SCENE_DAMAGE_WINDOW: u8 = 3;

#[derive(Debug)]
pub(crate) struct SceneQueue {
    bytes: VecDeque<u8>,
    capacity: usize,
}

impl SceneQueue {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            bytes: VecDeque::new(),
            capacity: capacity.max(SCENE_COMMAND_BYTES),
        }
    }

    pub(crate) fn pending(&self) -> usize {
        self.bytes.len()
    }

    pub(crate) fn clear(&mut self) {
        self.bytes.clear();
    }

    pub(crate) fn reset(&mut self) -> bool {
        let command = [0; SCENE_COMMAND_BYTES];
        self.push(&command)
    }

    pub(crate) fn drain(&mut self, destination: &mut [u8]) -> usize {
        let aligned_capacity = destination.len() / SCENE_COMMAND_BYTES * SCENE_COMMAND_BYTES;
        let count = aligned_capacity.min(self.bytes.len());
        for (index, byte) in destination[..count].iter_mut().enumerate() {
            let Some(value) = self.bytes.pop_front() else {
                return index;
            };
            *byte = value;
        }
        count
    }

    pub(crate) fn upsert(
        &mut self,
        window: &Window,
        viewable: bool,
        stack_index: u16,
        desktop_layer: bool,
        shell_layer: bool,
        backdrop_material: bool,
        active_window: bool,
        maximized_window: bool,
    ) -> bool {
        let mut command = [0; SCENE_COMMAND_BYTES];
        command[0] = SCENE_UPSERT_WINDOW;
        command[1] = u8::from(viewable)
            | (u8::from(window.override_redirect) << 1)
            | (u8::from(window.class == 2) << 2)
            | (u8::from(desktop_layer) << 3)
            | (u8::from(shell_layer) << 4)
            | (u8::from(backdrop_material) << 5)
            | (u8::from(active_window) << 6)
            | (u8::from(maximized_window) << 7);
        command[2] = window.depth;
        write_u32(&mut command[4..8], window.id.0);
        write_u32(&mut command[8..12], window.parent.0);
        write_i32(&mut command[12..16], i32::from(window.x));
        write_i32(&mut command[16..20], i32::from(window.y));
        write_u16(&mut command[20..22], window.width);
        write_u16(&mut command[22..24], window.height);
        write_u16(&mut command[24..26], window.border_width);
        write_u16(&mut command[26..28], stack_index);
        write_u32(&mut command[28..32], window.background_pixel);
        self.push(&command)
    }

    pub(crate) fn remove(&mut self, window: ResourceId) -> bool {
        let mut command = [0; SCENE_COMMAND_BYTES];
        command[0] = SCENE_REMOVE_WINDOW;
        write_u32(&mut command[4..8], window.0);
        self.push(&command)
    }

    pub(crate) fn damage(
        &mut self,
        window: ResourceId,
        x: i32,
        y: i32,
        width: u16,
        height: u16,
    ) -> bool {
        let mut command = [0; SCENE_COMMAND_BYTES];
        command[0] = SCENE_DAMAGE_WINDOW;
        write_u32(&mut command[4..8], window.0);
        write_i32(&mut command[12..16], x);
        write_i32(&mut command[16..20], y);
        write_u16(&mut command[20..22], width);
        write_u16(&mut command[22..24], height);
        self.push(&command)
    }

    fn push(&mut self, command: &[u8; SCENE_COMMAND_BYTES]) -> bool {
        if command.len() > self.capacity.saturating_sub(self.bytes.len()) {
            return false;
        }
        self.bytes.extend(command);
        true
    }
}

fn write_u16(output: &mut [u8], value: u16) {
    output.copy_from_slice(&value.to_le_bytes());
}

fn write_u32(output: &mut [u8], value: u32) {
    output.copy_from_slice(&value.to_le_bytes());
}

fn write_i32(output: &mut [u8], value: i32) {
    output.copy_from_slice(&value.to_le_bytes());
}
