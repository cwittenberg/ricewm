//! Paged, chunked, and mutation results emitted by provider adapters.

use crate::{
    DraftId, Label, MessageEnvelope, MessageId, MessageSummary, OperationId, PageToken,
    ProviderRevision, SyncState,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MailboxFailure {
    /// Stable provider-neutral category such as `offline`, `auth`, or `quota`.
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessagePage {
    pub items: Vec<MessageSummary>,
    pub next: Option<PageToken>,
    pub total_estimate: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextChunk {
    pub index: u32,
    pub text: String,
    pub last: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MailboxEvent {
    SubscriptionSnapshot {
        operation: OperationId,
        labels: Vec<Label>,
        sync: SyncState,
    },
    LabelsChanged {
        labels: Vec<Label>,
    },
    /// Observer updates use `None`; a terminal response to an issued Sync uses
    /// the matching operation identifier.
    SyncStateChanged {
        operation: Option<OperationId>,
        sync: SyncState,
    },
    MessagePage {
        operation: OperationId,
        page: MessagePage,
    },
    MessageEnvelope {
        operation: OperationId,
        envelope: MessageEnvelope,
    },
    MessageBodyChunk {
        operation: OperationId,
        message: MessageId,
        chunk: TextChunk,
    },
    MessagesChanged {
        operation: OperationId,
        updated: Vec<MessageSummary>,
        removed: Vec<MessageId>,
    },
    DraftSaved {
        operation: OperationId,
        id: DraftId,
        revision: Option<ProviderRevision>,
    },
    Sent {
        operation: OperationId,
        message: Option<MessageSummary>,
    },
    OperationFailed {
        operation: OperationId,
        failure: MailboxFailure,
    },
}
