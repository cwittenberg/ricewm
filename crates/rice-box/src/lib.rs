//! Pure provider-neutral mailbox state for the native `RiceWM` mail application.
//!
//! Provider adapters execute [`MailboxIntent`] values and return
//! [`MailboxEvent`] values. This crate deliberately has no network, storage,
//! OAuth, MIME, renderer, or host-protocol dependencies.

mod command;
mod id;
mod mailbox;
mod model;
mod result;

pub use command::{MAX_PAGE_SIZE, MailboxCommand, MailboxIntent, MessageListQuery};
pub use id::{
    AccountId, DraftId, IdError, LabelId, MessageId, OperationId, PageToken, ProviderId,
    ProviderRevision, SyncCursor, ThreadId,
};
pub use mailbox::{
    ApplyOutcome, ComposeState, ComposeStatus, Mailbox, MailboxError, MessageListState,
    SelectedMessage,
};
pub use model::{
    Account, AddressError, ComposeDraft, ConnectionState, Label, LabelKind, MailAddress,
    MessageEnvelope, MessageSummary, OpenMessage, SyncFailure, SyncPhase, SyncProgress, SyncReason,
    SyncState, SystemLabel,
};
pub use result::{MailboxEvent, MailboxFailure, MessagePage, TextChunk};

#[cfg(test)]
mod tests;
