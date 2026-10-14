//! One-account mailbox reducer.

use crate::{
    Account, ComposeDraft, ConnectionState, Label, MAX_PAGE_SIZE, MailboxCommand, MailboxEvent,
    MailboxFailure, MailboxIntent, MessageEnvelope, MessageId, MessageListQuery, MessageSummary,
    OpenMessage, OperationId, PageToken, SyncFailure, SyncPhase, SyncReason, SyncState,
};
use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

const MAX_MUTATION_MESSAGES: usize = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApplyOutcome {
    Applied,
    IgnoredStale,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ComposeStatus {
    Editing,
    Saving(OperationId),
    Sending(OperationId),
    Failed(MailboxFailure),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComposeState {
    pub draft: ComposeDraft,
    pub status: ComposeStatus,
}

impl Default for ComposeState {
    fn default() -> Self {
        Self {
            draft: ComposeDraft::default(),
            status: ComposeStatus::Editing,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageListState {
    pub query: Option<MessageListQuery>,
    pub messages: Vec<MessageSummary>,
    pub next_page: Option<PageToken>,
    pub total_estimate: Option<u32>,
    pub page_size: u16,
    pub loading: bool,
    pub active_operation: Option<OperationId>,
}

impl Default for MessageListState {
    fn default() -> Self {
        Self {
            query: None,
            messages: Vec::new(),
            next_page: None,
            total_estimate: None,
            page_size: 50,
            loading: false,
            active_operation: None,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum SelectedMessage {
    #[default]
    None,
    Loading {
        id: MessageId,
        operation: OperationId,
        envelope: Option<MessageEnvelope>,
        plain_body: String,
        next_chunk: u32,
    },
    Ready(OpenMessage),
    Failed {
        id: MessageId,
        failure: MailboxFailure,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PendingOperation {
    Subscribe,
    Sync,
    List { replace: bool },
    Open { message: MessageId },
    MarkRead,
    Star,
    Archive,
    Trash,
    SaveDraft,
    Send,
}

impl PendingOperation {
    const fn name(&self) -> &'static str {
        match self {
            Self::Subscribe => "subscribe",
            Self::Sync => "sync",
            Self::List { .. } => "list",
            Self::Open { .. } => "open",
            Self::MarkRead => "mark-read",
            Self::Star => "star",
            Self::Archive => "archive",
            Self::Trash => "trash",
            Self::SaveDraft => "save-draft",
            Self::Send => "send",
        }
    }

    const fn is_mutation(&self) -> bool {
        matches!(
            self,
            Self::MarkRead | Self::Star | Self::Archive | Self::Trash
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MailboxError {
    AlreadySubscribed,
    SubscribePending,
    NotSubscribed,
    InvalidPageSize(u16),
    NoNextPage,
    EmptyMessageSet,
    TooManyMessages(usize),
    NoRecipients,
    ComposeBusy,
    OperationExhausted,
    UnknownOperation(OperationId),
    UnexpectedResult {
        operation: OperationId,
        expected: &'static str,
        actual: &'static str,
    },
    MismatchedMessage {
        expected: MessageId,
        actual: MessageId,
    },
    OutOfOrderChunk {
        expected: u32,
        actual: u32,
    },
    EnvelopeMissing,
}

impl fmt::Display for MailboxError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadySubscribed => formatter.write_str("mailbox is already subscribed"),
            Self::SubscribePending => formatter.write_str("mailbox subscription is pending"),
            Self::NotSubscribed => formatter.write_str("mailbox is not subscribed"),
            Self::InvalidPageSize(size) => write!(
                formatter,
                "message page size {size} is outside 1..={MAX_PAGE_SIZE}"
            ),
            Self::NoNextPage => formatter.write_str("message list has no next page"),
            Self::EmptyMessageSet => formatter.write_str("message command has no targets"),
            Self::TooManyMessages(count) => write!(
                formatter,
                "message command has {count} targets; maximum is {MAX_MUTATION_MESSAGES}"
            ),
            Self::NoRecipients => formatter.write_str("message has no recipients"),
            Self::ComposeBusy => formatter.write_str("compose draft is being saved or sent"),
            Self::OperationExhausted => {
                formatter.write_str("mailbox operation identifiers exhausted")
            }
            Self::UnknownOperation(operation) => {
                write!(formatter, "mailbox operation {operation} is unknown")
            }
            Self::UnexpectedResult {
                operation,
                expected,
                actual,
            } => write!(
                formatter,
                "mailbox operation {operation} expected {expected}, received {actual}"
            ),
            Self::MismatchedMessage { expected, actual } => write!(
                formatter,
                "opened message {actual} does not match requested message {expected}"
            ),
            Self::OutOfOrderChunk { expected, actual } => write!(
                formatter,
                "message body chunk {actual} arrived while chunk {expected} was expected"
            ),
            Self::EnvelopeMissing => {
                formatter.write_str("message body completed before its envelope arrived")
            }
        }
    }
}

impl std::error::Error for MailboxError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Mailbox {
    account: Account,
    subscribed: bool,
    sync: SyncState,
    labels: Vec<Label>,
    list: MessageListState,
    selected: SelectedMessage,
    compose: ComposeState,
    last_failure: Option<MailboxFailure>,
    next_operation: u64,
    pending: BTreeMap<OperationId, PendingOperation>,
}

impl Mailbox {
    #[must_use]
    pub fn new(account: Account) -> Self {
        Self {
            account,
            subscribed: false,
            sync: SyncState::default(),
            labels: Vec::new(),
            list: MessageListState::default(),
            selected: SelectedMessage::None,
            compose: ComposeState::default(),
            last_failure: None,
            next_operation: 1,
            pending: BTreeMap::new(),
        }
    }

    #[must_use]
    pub const fn account(&self) -> &Account {
        &self.account
    }

    #[must_use]
    pub const fn subscribed(&self) -> bool {
        self.subscribed
    }

    #[must_use]
    pub const fn sync(&self) -> &SyncState {
        &self.sync
    }

    #[must_use]
    pub fn labels(&self) -> &[Label] {
        &self.labels
    }

    #[must_use]
    pub const fn list(&self) -> &MessageListState {
        &self.list
    }

    #[must_use]
    pub const fn selected(&self) -> &SelectedMessage {
        &self.selected
    }

    #[must_use]
    pub const fn compose(&self) -> &ComposeState {
        &self.compose
    }

    #[must_use]
    pub const fn last_failure(&self) -> Option<&MailboxFailure> {
        self.last_failure.as_ref()
    }

    /// Replaces the local compose DTO. `RicePaper` can adapt its document into this
    /// value without becoming a dependency of the mailbox domain.
    ///
    /// # Errors
    ///
    /// Returns [`MailboxError::ComposeBusy`] while a save or send is pending.
    pub fn edit_compose(&mut self, draft: ComposeDraft) -> Result<(), MailboxError> {
        if matches!(
            self.compose.status,
            ComposeStatus::Saving(_) | ComposeStatus::Sending(_)
        ) {
            return Err(MailboxError::ComposeBusy);
        }
        self.compose = ComposeState {
            draft,
            status: ComposeStatus::Editing,
        };
        Ok(())
    }

    /// Reduces a user command to exactly one provider-adapter intent.
    ///
    /// # Errors
    ///
    /// Returns [`MailboxError`] when the command is invalid for the current
    /// mailbox state or a bounded request limit is exceeded.
    pub fn issue(&mut self, command: MailboxCommand) -> Result<MailboxIntent, MailboxError> {
        match command {
            MailboxCommand::Subscribe => self.subscribe(),
            MailboxCommand::Sync { reason } => self.sync_intent(reason),
            MailboxCommand::List { query, page_size } => self.list_intent(query, page_size),
            MailboxCommand::ListNextPage => self.next_page_intent(),
            MailboxCommand::Open { message } => self.open_intent(message),
            MailboxCommand::MarkRead { messages, read } => {
                self.require_subscribed()?;
                let messages = validated_targets(messages)?;
                let operation = self.reserve(PendingOperation::MarkRead)?;
                Ok(MailboxIntent::MarkRead {
                    operation,
                    account: self.account.id.clone(),
                    messages,
                    read,
                })
            }
            MailboxCommand::Star { messages, starred } => {
                self.require_subscribed()?;
                let messages = validated_targets(messages)?;
                let operation = self.reserve(PendingOperation::Star)?;
                Ok(MailboxIntent::Star {
                    operation,
                    account: self.account.id.clone(),
                    messages,
                    starred,
                })
            }
            MailboxCommand::Archive { messages } => {
                self.require_subscribed()?;
                let messages = validated_targets(messages)?;
                let operation = self.reserve(PendingOperation::Archive)?;
                Ok(MailboxIntent::Archive {
                    operation,
                    account: self.account.id.clone(),
                    messages,
                })
            }
            MailboxCommand::Trash { messages } => {
                self.require_subscribed()?;
                let messages = validated_targets(messages)?;
                let operation = self.reserve(PendingOperation::Trash)?;
                Ok(MailboxIntent::Trash {
                    operation,
                    account: self.account.id.clone(),
                    messages,
                })
            }
            MailboxCommand::SaveDraft { draft } => self.save_draft_intent(draft),
            MailboxCommand::Send { draft } => self.send_intent(draft),
        }
    }

    /// Applies a provider event. Results from superseded list/open operations
    /// are consumed but reported as stale so they cannot replace current UI.
    ///
    /// # Errors
    ///
    /// Returns [`MailboxError`] for an unknown operation, an event that does
    /// not match its command, or invalid chunk ordering.
    #[allow(clippy::too_many_lines)]
    pub fn apply(&mut self, event: MailboxEvent) -> Result<ApplyOutcome, MailboxError> {
        match event {
            MailboxEvent::SubscriptionSnapshot {
                operation,
                labels,
                sync,
            } => {
                self.expect(operation, "subscribe", |pending| {
                    matches!(pending, PendingOperation::Subscribe)
                })?;
                self.pending.remove(&operation);
                self.subscribed = true;
                self.account.connection = ConnectionState::Connected;
                self.labels = normalized_labels(labels);
                self.sync = sync;
                self.last_failure = None;
            }
            MailboxEvent::LabelsChanged { labels } => {
                self.labels = normalized_labels(labels);
            }
            MailboxEvent::SyncStateChanged { operation, sync } => {
                if let Some(operation) = operation {
                    self.expect(operation, "sync", |pending| {
                        matches!(pending, PendingOperation::Sync)
                    })?;
                    self.pending.remove(&operation);
                }
                self.update_connection(&sync.phase);
                self.sync = sync;
            }
            MailboxEvent::MessagePage { operation, page } => {
                let replace = match self.pending.get(&operation) {
                    Some(PendingOperation::List { replace }) => *replace,
                    Some(pending) => {
                        return Err(MailboxError::UnexpectedResult {
                            operation,
                            expected: pending.name(),
                            actual: "message-page",
                        });
                    }
                    None => return Err(MailboxError::UnknownOperation(operation)),
                };
                self.pending.remove(&operation);
                if self.list.active_operation != Some(operation) {
                    return Ok(ApplyOutcome::IgnoredStale);
                }
                if replace {
                    self.list.messages.clear();
                }
                merge_summaries(&mut self.list.messages, page.items);
                self.list.next_page = page.next;
                self.list.total_estimate = page.total_estimate;
                self.list.loading = false;
                self.list.active_operation = None;
                self.last_failure = None;
            }
            MailboxEvent::MessageEnvelope {
                operation,
                envelope,
            } => {
                let expected = self.open_message(operation)?;
                if envelope.summary.id != expected {
                    return Err(MailboxError::MismatchedMessage {
                        expected,
                        actual: envelope.summary.id,
                    });
                }
                let SelectedMessage::Loading {
                    operation: active,
                    envelope: slot,
                    ..
                } = &mut self.selected
                else {
                    return Ok(ApplyOutcome::IgnoredStale);
                };
                if *active != operation {
                    return Ok(ApplyOutcome::IgnoredStale);
                }
                *slot = Some(envelope);
            }
            MailboxEvent::MessageBodyChunk {
                operation,
                message,
                chunk,
            } => {
                let expected = self.open_message(operation)?;
                if message != expected {
                    return Err(MailboxError::MismatchedMessage {
                        expected,
                        actual: message,
                    });
                }
                let SelectedMessage::Loading {
                    operation: active,
                    envelope,
                    plain_body,
                    next_chunk,
                    ..
                } = &mut self.selected
                else {
                    return Ok(ApplyOutcome::IgnoredStale);
                };
                if *active != operation {
                    return Ok(ApplyOutcome::IgnoredStale);
                }
                if chunk.index != *next_chunk {
                    return Err(MailboxError::OutOfOrderChunk {
                        expected: *next_chunk,
                        actual: chunk.index,
                    });
                }
                plain_body.push_str(&chunk.text);
                *next_chunk = next_chunk.saturating_add(1);
                if chunk.last {
                    let envelope = envelope.take().ok_or(MailboxError::EnvelopeMissing)?;
                    let plain_body = core::mem::take(plain_body);
                    self.selected = SelectedMessage::Ready(OpenMessage {
                        envelope,
                        plain_body,
                    });
                    self.pending.remove(&operation);
                    self.last_failure = None;
                }
            }
            MailboxEvent::MessagesChanged {
                operation,
                updated,
                removed,
            } => {
                let Some(pending) = self.pending.get(&operation) else {
                    return Err(MailboxError::UnknownOperation(operation));
                };
                if !pending.is_mutation() {
                    return Err(MailboxError::UnexpectedResult {
                        operation,
                        expected: pending.name(),
                        actual: "messages-changed",
                    });
                }
                self.pending.remove(&operation);
                for summary in updated {
                    self.update_summary(summary);
                }
                self.remove_messages(&removed);
                self.last_failure = None;
            }
            MailboxEvent::DraftSaved {
                operation,
                id,
                revision,
            } => {
                self.expect(operation, "save-draft", |pending| {
                    matches!(pending, PendingOperation::SaveDraft)
                })?;
                self.pending.remove(&operation);
                self.compose.draft.id = Some(id);
                self.compose.draft.provider_revision = revision;
                self.compose.draft.dirty = false;
                self.compose.status = ComposeStatus::Editing;
                self.last_failure = None;
            }
            MailboxEvent::Sent { operation, message } => {
                self.expect(operation, "send", |pending| {
                    matches!(pending, PendingOperation::Send)
                })?;
                self.pending.remove(&operation);
                if let Some(message) = message {
                    self.update_summary(message);
                }
                self.compose = ComposeState::default();
                self.last_failure = None;
            }
            MailboxEvent::OperationFailed { operation, failure } => {
                let pending = self
                    .pending
                    .remove(&operation)
                    .ok_or(MailboxError::UnknownOperation(operation))?;
                self.apply_failure(operation, pending, failure);
            }
        }
        Ok(ApplyOutcome::Applied)
    }

    fn subscribe(&mut self) -> Result<MailboxIntent, MailboxError> {
        if self.subscribed {
            return Err(MailboxError::AlreadySubscribed);
        }
        if self
            .pending
            .values()
            .any(|pending| matches!(pending, PendingOperation::Subscribe))
        {
            return Err(MailboxError::SubscribePending);
        }
        let operation = self.reserve(PendingOperation::Subscribe)?;
        self.account.connection = ConnectionState::Connecting;
        Ok(MailboxIntent::Subscribe {
            operation,
            account: self.account.id.clone(),
        })
    }

    fn sync_intent(&mut self, reason: SyncReason) -> Result<MailboxIntent, MailboxError> {
        self.require_subscribed()?;
        let operation = self.reserve(PendingOperation::Sync)?;
        self.sync.phase = SyncPhase::Syncing(reason);
        self.sync.progress = None;
        Ok(MailboxIntent::Sync {
            operation,
            account: self.account.id.clone(),
            reason,
            cursor: self.sync.cursor.clone(),
        })
    }

    fn list_intent(
        &mut self,
        query: MessageListQuery,
        page_size: u16,
    ) -> Result<MailboxIntent, MailboxError> {
        self.require_subscribed()?;
        if !(1..=MAX_PAGE_SIZE).contains(&page_size) {
            return Err(MailboxError::InvalidPageSize(page_size));
        }
        let operation = self.reserve(PendingOperation::List { replace: true })?;
        self.list.query = Some(query.clone());
        self.list.next_page = None;
        self.list.total_estimate = None;
        self.list.page_size = page_size;
        self.list.loading = true;
        self.list.active_operation = Some(operation);
        Ok(MailboxIntent::List {
            operation,
            account: self.account.id.clone(),
            query,
            page_size,
            page: None,
        })
    }

    fn next_page_intent(&mut self) -> Result<MailboxIntent, MailboxError> {
        self.require_subscribed()?;
        let query = self.list.query.clone().ok_or(MailboxError::NoNextPage)?;
        let page = self
            .list
            .next_page
            .clone()
            .ok_or(MailboxError::NoNextPage)?;
        let operation = self.reserve(PendingOperation::List { replace: false })?;
        self.list.loading = true;
        self.list.active_operation = Some(operation);
        Ok(MailboxIntent::List {
            operation,
            account: self.account.id.clone(),
            query,
            page_size: self.list.page_size,
            page: Some(page),
        })
    }

    fn open_intent(&mut self, message: MessageId) -> Result<MailboxIntent, MailboxError> {
        self.require_subscribed()?;
        let operation = self.reserve(PendingOperation::Open {
            message: message.clone(),
        })?;
        self.selected = SelectedMessage::Loading {
            id: message.clone(),
            operation,
            envelope: None,
            plain_body: String::new(),
            next_chunk: 0,
        };
        Ok(MailboxIntent::Open {
            operation,
            account: self.account.id.clone(),
            message,
        })
    }

    fn save_draft_intent(&mut self, draft: ComposeDraft) -> Result<MailboxIntent, MailboxError> {
        self.require_subscribed()?;
        if matches!(
            self.compose.status,
            ComposeStatus::Saving(_) | ComposeStatus::Sending(_)
        ) {
            return Err(MailboxError::ComposeBusy);
        }
        let operation = self.reserve(PendingOperation::SaveDraft)?;
        self.compose = ComposeState {
            draft: draft.clone(),
            status: ComposeStatus::Saving(operation),
        };
        Ok(MailboxIntent::SaveDraft {
            operation,
            account: self.account.id.clone(),
            draft,
        })
    }

    fn send_intent(&mut self, draft: ComposeDraft) -> Result<MailboxIntent, MailboxError> {
        self.require_subscribed()?;
        if matches!(
            self.compose.status,
            ComposeStatus::Saving(_) | ComposeStatus::Sending(_)
        ) {
            return Err(MailboxError::ComposeBusy);
        }
        if !draft.has_recipient() {
            return Err(MailboxError::NoRecipients);
        }
        let operation = self.reserve(PendingOperation::Send)?;
        self.compose = ComposeState {
            draft: draft.clone(),
            status: ComposeStatus::Sending(operation),
        };
        Ok(MailboxIntent::Send {
            operation,
            account: self.account.id.clone(),
            draft,
        })
    }

    fn reserve(&mut self, pending: PendingOperation) -> Result<OperationId, MailboxError> {
        let operation =
            OperationId::new(self.next_operation).map_err(|_| MailboxError::OperationExhausted)?;
        self.next_operation = self
            .next_operation
            .checked_add(1)
            .ok_or(MailboxError::OperationExhausted)?;
        self.pending.insert(operation, pending);
        Ok(operation)
    }

    fn require_subscribed(&self) -> Result<(), MailboxError> {
        if self.subscribed {
            Ok(())
        } else {
            Err(MailboxError::NotSubscribed)
        }
    }

    fn expect(
        &self,
        operation: OperationId,
        actual: &'static str,
        predicate: impl FnOnce(&PendingOperation) -> bool,
    ) -> Result<(), MailboxError> {
        let pending = self
            .pending
            .get(&operation)
            .ok_or(MailboxError::UnknownOperation(operation))?;
        if predicate(pending) {
            Ok(())
        } else {
            Err(MailboxError::UnexpectedResult {
                operation,
                expected: pending.name(),
                actual,
            })
        }
    }

    fn open_message(&self, operation: OperationId) -> Result<MessageId, MailboxError> {
        match self.pending.get(&operation) {
            Some(PendingOperation::Open { message }) => Ok(message.clone()),
            Some(pending) => Err(MailboxError::UnexpectedResult {
                operation,
                expected: pending.name(),
                actual: "open-message",
            }),
            None => Err(MailboxError::UnknownOperation(operation)),
        }
    }

    fn update_connection(&mut self, phase: &SyncPhase) {
        self.account.connection = match phase {
            SyncPhase::Offline => ConnectionState::Offline,
            SyncPhase::AuthenticationRequired => ConnectionState::AuthenticationRequired,
            SyncPhase::Idle | SyncPhase::Syncing(_) if self.subscribed => {
                ConnectionState::Connected
            }
            _ => self.account.connection,
        };
    }

    fn update_summary(&mut self, summary: MessageSummary) {
        if let Some(existing) = self
            .list
            .messages
            .iter_mut()
            .find(|candidate| candidate.id == summary.id)
        {
            *existing = summary.clone();
        }
        match &mut self.selected {
            SelectedMessage::Loading {
                envelope: Some(envelope),
                ..
            }
            | SelectedMessage::Ready(OpenMessage { envelope, .. })
                if envelope.summary.id == summary.id =>
            {
                envelope.summary = summary;
            }
            _ => {}
        }
    }

    fn remove_messages(&mut self, removed: &[MessageId]) {
        let removed = removed.iter().collect::<BTreeSet<_>>();
        self.list
            .messages
            .retain(|summary| !removed.contains(&summary.id));
        let selected_removed = match &self.selected {
            SelectedMessage::None => false,
            SelectedMessage::Loading { id, .. } | SelectedMessage::Failed { id, .. } => {
                removed.contains(id)
            }
            SelectedMessage::Ready(message) => removed.contains(&message.envelope.summary.id),
        };
        if selected_removed {
            self.selected = SelectedMessage::None;
        }
    }

    fn apply_failure(
        &mut self,
        operation: OperationId,
        pending: PendingOperation,
        failure: MailboxFailure,
    ) {
        match pending {
            PendingOperation::Subscribe => {
                self.account.connection = ConnectionState::Disconnected;
                self.subscribed = false;
            }
            PendingOperation::Sync => {
                self.sync.phase = SyncPhase::Failed(SyncFailure {
                    message: failure.message.clone(),
                    retryable: failure.retryable,
                });
                self.sync.progress = None;
            }
            PendingOperation::List { .. } => {
                if self.list.active_operation == Some(operation) {
                    self.list.loading = false;
                    self.list.active_operation = None;
                }
            }
            PendingOperation::Open { message } => {
                if matches!(
                    self.selected,
                    SelectedMessage::Loading { operation: active, .. } if active == operation
                ) {
                    self.selected = SelectedMessage::Failed {
                        id: message,
                        failure: failure.clone(),
                    };
                }
            }
            PendingOperation::SaveDraft | PendingOperation::Send => {
                self.compose.status = ComposeStatus::Failed(failure.clone());
            }
            PendingOperation::MarkRead
            | PendingOperation::Star
            | PendingOperation::Archive
            | PendingOperation::Trash => {}
        }
        self.last_failure = Some(failure);
    }
}

fn validated_targets(messages: Vec<MessageId>) -> Result<Vec<MessageId>, MailboxError> {
    if messages.is_empty() {
        return Err(MailboxError::EmptyMessageSet);
    }
    let messages = messages.into_iter().collect::<BTreeSet<_>>();
    if messages.len() > MAX_MUTATION_MESSAGES {
        return Err(MailboxError::TooManyMessages(messages.len()));
    }
    Ok(messages.into_iter().collect())
}

fn normalized_labels(labels: Vec<Label>) -> Vec<Label> {
    let mut normalized = Vec::<Label>::new();
    for label in labels {
        if let Some(existing) = normalized
            .iter_mut()
            .find(|existing| existing.id == label.id)
        {
            *existing = label;
        } else {
            normalized.push(label);
        }
    }
    normalized
}

fn merge_summaries(messages: &mut Vec<MessageSummary>, incoming: Vec<MessageSummary>) {
    for summary in incoming {
        if let Some(existing) = messages
            .iter_mut()
            .find(|existing| existing.id == summary.id)
        {
            *existing = summary;
        } else {
            messages.push(summary);
        }
    }
}
