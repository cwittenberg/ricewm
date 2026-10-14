//! Focus requests operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn set_input_focus(&mut self, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let requested = request_resource(body).ok_or_else(bad_length)?;
        let time = read_u32_le(body.get(4..8).ok_or_else(bad_length)?);
        let time = if time == 0 { self.timestamp } else { time };
        // Late WM_TAKE_FOCUS replies must not undo a more recent pointer or
        // keyboard activation. X11 timestamps wrap; compare signed deltas.
        if (time.wrapping_sub(self.last_focus_time) as i32) < 0 ||
            (self.timestamp.wrapping_sub(time) as i32) < 0 { return Ok(Vec::new()); }
        let focus = match requested.0 {
            // Treat the protocol's None sentinel as the root during this
            // click-to-focus stage so browser key input always has a sink.
            0 => ROOT_WINDOW,
            // PointerRoot resolves to the current topmost mapped window.
            1 => self.hit_test(self.pointer_x, self.pointer_y),
            _ => requested,
        };
        if !self.windows.contains_key(&focus.0) {
            return Err(ProtocolError::new(x11::error::BAD_WINDOW, focus.0));
        }
        if self.map_state(focus) != 2 {
            return Err(ProtocolError::new(x11::error::BAD_MATCH, focus.0));
        }
        // A toolkit may answer WM_TAKE_FOCUS after the user activates a Rice
        // window. X11 still remembers its old focus_window, and CurrentTime
        // replies pass the timestamp check. Shell activation, not a delayed
        // toolkit reply, must transfer ownership back from a native task.
        if self.native_task_owns_input() {
            return Ok(Vec::new());
        }
        self.set_focus(focus);
        self.last_focus_time = time;
        self.notify_x11_foreground(focus);
        Ok(Vec::new())
    }
}
