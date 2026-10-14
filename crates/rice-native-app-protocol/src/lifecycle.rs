use crate::{AppId, RequestId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseRequest {
    pub request: RequestId,
    pub app: AppId,
    pub reason: CloseReason,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseResponse {
    pub request: RequestId,
    pub app: AppId,
    pub decision: CloseDecision,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CloseReason {
    Requested = 0,
    LastWindowClosed = 1,
    HostShutdown = 2,
    ProtocolFault = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CloseDecision {
    Approved = 0,
    Pending = 1,
    Cancelled = 2,
}
