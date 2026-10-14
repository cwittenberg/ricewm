//! Typed clipboard and drag/drop transfer messages.

use crate::{AppId, DragId, RequestId, WidgetId, WindowId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ClipboardSelection {
    Clipboard = 0,
    Primary = 1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardFormat {
    pub mime_type: String,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClipboardRequest {
    Read {
        request: RequestId,
        selection: ClipboardSelection,
        accepted_mime_types: Vec<String>,
    },
    Write {
        request: RequestId,
        selection: ClipboardSelection,
        mime_type: String,
        data: Vec<u8>,
    },
    /// One atomic clipboard value with alternative representations.
    WriteFormats {
        request: RequestId,
        selection: ClipboardSelection,
        formats: Vec<ClipboardFormat>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardResponse {
    pub request: RequestId,
    pub result: ClipboardResult,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClipboardResult {
    Data { mime_type: String, data: Vec<u8> },
    Empty,
    Rejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DragAction {
    Copy = 0,
    Move = 1,
    Link = 2,
    Ask = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DragPhase {
    Enter = 0,
    Motion = 1,
    Leave = 2,
    Drop = 3,
    Finished = 4,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DragEnvelope {
    pub drag: DragId,
    pub app: AppId,
    pub window: WindowId,
    pub target: Option<WidgetId>,
    pub phase: DragPhase,
    pub action: DragAction,
    pub x: i32,
    pub y: i32,
    pub mime_types: Vec<String>,
    /// Negotiated data supplied only for a drop. Enter/motion/leave messages
    /// advertise MIME types without repeatedly copying the payload.
    pub payload: Option<DragPayload>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DragPayload {
    pub mime_type: String,
    pub data: Vec<u8>,
}

/// A source-side drag offer or its eventual host result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DragSourceEvent {
    pub drag: DragId,
    pub app: AppId,
    pub window: WindowId,
    pub state: DragSourceState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DragSourceState {
    /// The app owns this bounded payload and asks the host to begin XDND once
    /// the pointer leaves its native surface.
    Offer {
        action: DragAction,
        mime_types: Vec<String>,
        payload: DragPayload,
    },
    /// The app completed an in-window gesture before an external drop.
    Cancel,
    /// The host completed, rejected, cancelled, or timed out the XDND session.
    Finished(DragSourceResult),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DragSourceResult {
    Completed(DragAction),
    Rejected,
    Cancelled,
    Failed,
}
