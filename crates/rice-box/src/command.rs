//! User commands and provider-adapter intents.

use crate::{
    AccountId, ComposeDraft, LabelId, MessageId, OperationId, PageToken, SyncCursor, SyncReason,
};

pub const MAX_PAGE_SIZE: u16 = 200;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MessageListQuery {
    pub label: Option<LabelId>,
    pub unread_only: bool,
    pub starred_only: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MailboxCommand {
    Subscribe,
    Sync {
        reason: SyncReason,
    },
    List {
        query: MessageListQuery,
        page_size: u16,
    },
    ListNextPage,
    Open {
        message: MessageId,
    },
    MarkRead {
        messages: Vec<MessageId>,
        read: bool,
    },
    Star {
        messages: Vec<MessageId>,
        starred: bool,
    },
    Archive {
        messages: Vec<MessageId>,
    },
    Trash {
        messages: Vec<MessageId>,
    },
    SaveDraft {
        draft: ComposeDraft,
    },
    Send {
        draft: ComposeDraft,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MailboxIntent {
    Subscribe {
        operation: OperationId,
        account: AccountId,
    },
    Sync {
        operation: OperationId,
        account: AccountId,
        reason: SyncReason,
        cursor: Option<SyncCursor>,
    },
    List {
        operation: OperationId,
        account: AccountId,
        query: MessageListQuery,
        page_size: u16,
        page: Option<PageToken>,
    },
    Open {
        operation: OperationId,
        account: AccountId,
        message: MessageId,
    },
    MarkRead {
        operation: OperationId,
        account: AccountId,
        messages: Vec<MessageId>,
        read: bool,
    },
    Star {
        operation: OperationId,
        account: AccountId,
        messages: Vec<MessageId>,
        starred: bool,
    },
    Archive {
        operation: OperationId,
        account: AccountId,
        messages: Vec<MessageId>,
    },
    Trash {
        operation: OperationId,
        account: AccountId,
        messages: Vec<MessageId>,
    },
    SaveDraft {
        operation: OperationId,
        account: AccountId,
        draft: ComposeDraft,
    },
    Send {
        operation: OperationId,
        account: AccountId,
        draft: ComposeDraft,
    },
}

impl MailboxIntent {
    #[must_use]
    pub const fn operation(&self) -> OperationId {
        match self {
            Self::Subscribe { operation, .. }
            | Self::Sync { operation, .. }
            | Self::List { operation, .. }
            | Self::Open { operation, .. }
            | Self::MarkRead { operation, .. }
            | Self::Star { operation, .. }
            | Self::Archive { operation, .. }
            | Self::Trash { operation, .. }
            | Self::SaveDraft { operation, .. }
            | Self::Send { operation, .. } => *operation,
        }
    }
}
