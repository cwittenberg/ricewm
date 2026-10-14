//! Provider-neutral account, label, message, sync, and compose models.

use crate::{
    AccountId, DraftId, LabelId, MessageId, ProviderId, ProviderRevision, SyncCursor, ThreadId,
};
use core::fmt;

const MAX_ADDRESS_BYTES: usize = 320;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AddressError {
    Empty,
    TooLong,
    Invalid,
}

impl fmt::Display for AddressError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("mail address is empty"),
            Self::TooLong => formatter.write_str("mail address is too long"),
            Self::Invalid => formatter.write_str("mail address is invalid"),
        }
    }
}

impl std::error::Error for AddressError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MailAddress {
    pub name: String,
    pub address: String,
}

impl MailAddress {
    /// Performs only transport-independent validation. The provider adapter is
    /// responsible for full RFC address parsing and normalization.
    ///
    /// # Errors
    ///
    /// Returns [`AddressError`] for an empty, oversized, or structurally
    /// invalid address.
    pub fn new(name: impl Into<String>, address: impl Into<String>) -> Result<Self, AddressError> {
        let address = address.into().trim().to_owned();
        if address.is_empty() {
            return Err(AddressError::Empty);
        }
        if address.len() > MAX_ADDRESS_BYTES {
            return Err(AddressError::TooLong);
        }
        let Some((local, domain)) = address.rsplit_once('@') else {
            return Err(AddressError::Invalid);
        };
        if local.is_empty()
            || domain.is_empty()
            || address.chars().any(char::is_control)
            || address.chars().any(char::is_whitespace)
        {
            return Err(AddressError::Invalid);
        }
        Ok(Self {
            name: name.into(),
            address,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Account {
    pub id: AccountId,
    pub provider: ProviderId,
    pub display_name: String,
    pub primary_address: MailAddress,
    pub connection: ConnectionState,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ConnectionState {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Offline,
    AuthenticationRequired,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SyncReason {
    Initial,
    #[default]
    Manual,
    Periodic,
    Push,
    Recovery,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncFailure {
    pub message: String,
    pub retryable: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum SyncPhase {
    #[default]
    Idle,
    Syncing(SyncReason),
    Offline,
    AuthenticationRequired,
    Failed(SyncFailure),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SyncProgress {
    pub processed: u32,
    pub total: Option<u32>,
    pub backlog: u32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SyncState {
    pub phase: SyncPhase,
    pub cursor: Option<SyncCursor>,
    pub progress: Option<SyncProgress>,
    pub last_success_epoch_ms: Option<i64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SystemLabel {
    Inbox,
    Sent,
    Drafts,
    Spam,
    Trash,
    Archive,
    Starred,
    Important,
    AllMail,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LabelKind {
    /// A provider resource with single-parent folder semantics.
    Folder,
    /// A provider resource that may coexist with other labels on a message.
    Label,
    /// A canonical role whose provider identifier is intentionally opaque.
    System(SystemLabel),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Label {
    pub id: LabelId,
    pub name: String,
    pub kind: LabelKind,
    pub parent: Option<LabelId>,
    pub unread_count: u32,
    pub total_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageSummary {
    pub id: MessageId,
    pub thread_id: ThreadId,
    pub labels: Vec<LabelId>,
    pub sender: MailAddress,
    pub subject: String,
    pub snippet: String,
    pub received_epoch_ms: i64,
    pub read: bool,
    pub starred: bool,
    pub has_attachments: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageEnvelope {
    pub summary: MessageSummary,
    pub to: Vec<MailAddress>,
    pub cc: Vec<MailAddress>,
    pub bcc: Vec<MailAddress>,
    pub reply_to: Vec<MailAddress>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenMessage {
    pub envelope: MessageEnvelope,
    pub plain_body: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComposeDraft {
    pub id: Option<DraftId>,
    pub provider_revision: Option<ProviderRevision>,
    pub in_reply_to: Option<MessageId>,
    pub to: Vec<MailAddress>,
    pub cc: Vec<MailAddress>,
    pub bcc: Vec<MailAddress>,
    pub subject: String,
    pub plain_body: String,
    pub dirty: bool,
}

impl ComposeDraft {
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: None,
            provider_revision: None,
            in_reply_to: None,
            to: Vec::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: String::new(),
            plain_body: String::new(),
            dirty: false,
        }
    }

    #[must_use]
    pub fn has_recipient(&self) -> bool {
        !(self.to.is_empty() && self.cc.is_empty() && self.bcc.is_empty())
    }
}

impl Default for ComposeDraft {
    fn default() -> Self {
        Self::new()
    }
}
