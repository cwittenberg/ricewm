use super::*;

// Stable semantic cursor IDs consumed by rice-display-core.mjs. Keep native
// applications in the desktop's selected cursor theme, size and color.
pub(super) fn named_shape(name: &[u8]) -> u32 {
    match name {
        b"pointer" | b"hand" | b"hand1" | b"hand2" | b"pointing_hand" => 1,
        b"text" | b"xterm" | b"ibeam" => 2,
        b"e-resize" | b"w-resize" | b"ew-resize" | b"col-resize" | b"sb_h_double_arrow" | b"h_double_arrow" | b"size_hor" | b"left_side" | b"right_side" => 3,
        b"n-resize" | b"s-resize" | b"ns-resize" | b"row-resize" | b"sb_v_double_arrow" | b"v_double_arrow" | b"size_ver" | b"top_side" | b"bottom_side" => 4,
        b"nw-resize" | b"se-resize" | b"nwse-resize" | b"size_fdiag" | b"top_left_corner" | b"bottom_right_corner" => 5,
        b"ne-resize" | b"sw-resize" | b"nesw-resize" | b"size_bdiag" | b"top_right_corner" | b"bottom_left_corner" => 6,
        b"move" | b"fleur" | b"all-scroll" | b"size_all" => 7,
        b"crosshair" | b"cross" | b"tcross" => 8,
        b"wait" | b"watch" => 9,
        b"progress" | b"left_ptr_watch" => 10,
        b"not-allowed" | b"no-drop" | b"forbidden" | b"crossed_circle" => 11,
        b"grab" | b"openhand" | b"open_hand" => 12,
        b"grabbing" | b"closedhand" | b"closed_hand" => 13,
        b"help" | b"question_arrow" => 14,
        _ => 0,
    }
}

pub(super) fn glyph_shape(glyph: u16) -> u32 {
    match glyph {
        58 | 60 => 1, // XC_hand1 / XC_hand2
        152 => 2, // XC_xterm
        70 | 96 | 108 => 3,
        16 | 114 | 138 => 4,
        14 | 134 => 5,
        12 | 136 => 6,
        52 => 7,
        30 | 34 | 130 => 8,
        150 => 9,
        92 => 14,
        _ => 0,
    }
}

impl DisplayServer {
    pub(super) fn resolve_cursor(&self, id: u32) -> Result<Option<RiceCursor>, ProtocolError> {
        if id == 0 { return Ok(None); }
        self.cursors.get(&id).cloned().map(Some)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_CURSOR, id))
    }

    pub(super) fn assign_window_cursor(&mut self, window: ResourceId, cursor: Option<RiceCursor>) {
        if let Some(cursor) = cursor {
            self.window_cursors.insert(window.0, cursor);
        } else {
            self.window_cursors.remove(&window.0);
        }
    }

    pub fn pointer_cursor(&self) -> u32 {
        let mut window = self.hit_test(self.pointer_x, self.pointer_y);
        // Resolve live ancestry: an unset child cursor inherits its parent's.
        for _ in 0..self.windows.len() {
            if let Some(cursor) = self.window_cursors.get(&window.0) {
                return cursor.shape();
            }
            if window == ROOT_WINDOW { break; }
            let Some(state) = self.windows.get(&window.0) else { break; };
            window = state.parent;
        }
        0
    }
}
