use super::*;

fn account_id(value: &str) -> AccountId {
    AccountId::new(value).unwrap()
}

fn provider_id(value: &str) -> ProviderId {
    ProviderId::new(value).unwrap()
}

fn label_id(value: &str) -> LabelId {
    LabelId::new(value).unwrap()
}

fn message_id(value: &str) -> MessageId {
    MessageId::new(value).unwrap()
}

fn thread_id(value: &str) -> ThreadId {
    ThreadId::new(value).unwrap()
}

fn address(value: &str) -> MailAddress {
    MailAddress::new("", value).unwrap()
}

fn account() -> Account {
    Account {
        id: account_id("primary"),
        provider: provider_id("gmail"),
        display_name: String::from("Primary Gmail"),
        primary_address: address("owner@example.com"),
        connection: ConnectionState::Disconnected,
    }
}

fn label(value: &str, name: &str, kind: LabelKind) -> Label {
    Label {
        id: label_id(value),
        name: String::from(name),
        kind,
        parent: None,
        unread_count: 0,
        total_count: 0,
    }
}

fn summary(value: &str, received_epoch_ms: i64) -> MessageSummary {
    MessageSummary {
        id: message_id(value),
        thread_id: thread_id(&format!("thread-{value}")),
        labels: vec![label_id("inbox")],
        sender: address("sender@example.com"),
        subject: format!("Subject {value}"),
        snippet: format!("Snippet {value}"),
        received_epoch_ms,
        read: false,
        starred: false,
        has_attachments: false,
    }
}

fn subscribed_mailbox() -> Mailbox {
    let mut mailbox = Mailbox::new(account());
    let operation = mailbox
        .issue(MailboxCommand::Subscribe)
        .unwrap()
        .operation();
    mailbox
        .apply(MailboxEvent::SubscriptionSnapshot {
            operation,
            labels: vec![label(
                "inbox",
                "Inbox",
                LabelKind::System(SystemLabel::Inbox),
            )],
            sync: SyncState::default(),
        })
        .unwrap();
    mailbox
}

fn first_page(mailbox: &mut Mailbox, items: Vec<MessageSummary>) {
    let operation = mailbox
        .issue(MailboxCommand::List {
            query: MessageListQuery {
                label: Some(label_id("inbox")),
                ..MessageListQuery::default()
            },
            page_size: 2,
        })
        .unwrap()
        .operation();
    mailbox
        .apply(MailboxEvent::MessagePage {
            operation,
            page: MessagePage {
                items,
                next: None,
                total_estimate: None,
            },
        })
        .unwrap();
}

#[test]
fn subscription_keeps_provider_neutral_label_roles_and_sync_cursor() {
    let mut mailbox = Mailbox::new(account());
    let subscribe = mailbox.issue(MailboxCommand::Subscribe).unwrap();
    assert!(matches!(
        subscribe,
        MailboxIntent::Subscribe { ref account, .. } if account.as_str() == "primary"
    ));
    assert_eq!(mailbox.account().connection, ConnectionState::Connecting);

    let cursor = SyncCursor::new("history-42").unwrap();
    let operation = subscribe.operation();
    mailbox
        .apply(MailboxEvent::SubscriptionSnapshot {
            operation,
            labels: vec![
                label("inbox", "Inbox", LabelKind::System(SystemLabel::Inbox)),
                label("projects", "Projects", LabelKind::Folder),
                label("follow-up", "Follow up", LabelKind::Label),
                label("follow-up", "Follow-up", LabelKind::Label),
            ],
            sync: SyncState {
                phase: SyncPhase::Idle,
                cursor: Some(cursor.clone()),
                progress: None,
                last_success_epoch_ms: Some(1_786_400_000_000),
            },
        })
        .unwrap();

    assert!(mailbox.subscribed());
    assert_eq!(mailbox.labels().len(), 3, "duplicate IDs are replaced");
    assert_eq!(mailbox.labels()[2].name, "Follow-up");
    assert!(matches!(mailbox.labels()[1].kind, LabelKind::Folder));

    let sync = mailbox
        .issue(MailboxCommand::Sync {
            reason: SyncReason::Manual,
        })
        .unwrap();
    assert!(matches!(
        sync,
        MailboxIntent::Sync {
            reason: SyncReason::Manual,
            cursor: Some(ref actual),
            ..
        } if actual == &cursor
    ));
}

#[test]
fn message_pages_replace_append_deduplicate_and_ignore_stale_results() {
    let mut mailbox = subscribed_mailbox();
    let query = MessageListQuery {
        label: Some(label_id("inbox")),
        ..MessageListQuery::default()
    };
    let first = mailbox
        .issue(MailboxCommand::List {
            query: query.clone(),
            page_size: 2,
        })
        .unwrap();
    let first_operation = first.operation();
    mailbox
        .apply(MailboxEvent::MessagePage {
            operation: first_operation,
            page: MessagePage {
                items: vec![summary("one", 1), summary("two", 2)],
                next: Some(PageToken::new("page-2").unwrap()),
                total_estimate: Some(3),
            },
        })
        .unwrap();

    let next = mailbox.issue(MailboxCommand::ListNextPage).unwrap();
    assert!(matches!(
        next,
        MailboxIntent::List {
            page: Some(ref page),
            page_size: 2,
            ..
        } if page.as_str() == "page-2"
    ));
    let mut replacement = summary("two", 20);
    replacement.read = true;
    mailbox
        .apply(MailboxEvent::MessagePage {
            operation: next.operation(),
            page: MessagePage {
                items: vec![replacement, summary("three", 3)],
                next: None,
                total_estimate: Some(3),
            },
        })
        .unwrap();
    assert_eq!(mailbox.list().messages.len(), 3);
    assert!(mailbox.list().messages[1].read);

    let stale = mailbox
        .issue(MailboxCommand::List {
            query: query.clone(),
            page_size: 2,
        })
        .unwrap();
    let current = mailbox
        .issue(MailboxCommand::List {
            query: MessageListQuery {
                starred_only: true,
                ..query
            },
            page_size: 2,
        })
        .unwrap();
    assert_eq!(
        mailbox
            .apply(MailboxEvent::MessagePage {
                operation: stale.operation(),
                page: MessagePage {
                    items: vec![summary("stale", 4)],
                    next: None,
                    total_estimate: Some(1),
                },
            })
            .unwrap(),
        ApplyOutcome::IgnoredStale
    );
    assert_eq!(mailbox.list().active_operation, Some(current.operation()));
    mailbox
        .apply(MailboxEvent::MessagePage {
            operation: current.operation(),
            page: MessagePage {
                items: vec![summary("current", 5)],
                next: None,
                total_estimate: Some(1),
            },
        })
        .unwrap();
    assert_eq!(mailbox.list().messages[0].id.as_str(), "current");
}

#[test]
fn selected_message_is_assembled_from_ordered_plain_text_chunks() {
    let mut mailbox = subscribed_mailbox();
    let intent = mailbox
        .issue(MailboxCommand::Open {
            message: message_id("message-1"),
        })
        .unwrap();
    let operation = intent.operation();
    let envelope = MessageEnvelope {
        summary: summary("message-1", 1),
        to: vec![address("owner@example.com")],
        cc: vec![address("copy@example.com")],
        bcc: Vec::new(),
        reply_to: vec![address("reply@example.com")],
    };
    mailbox
        .apply(MailboxEvent::MessageEnvelope {
            operation,
            envelope: envelope.clone(),
        })
        .unwrap();

    assert_eq!(
        mailbox
            .apply(MailboxEvent::MessageBodyChunk {
                operation,
                message: message_id("message-1"),
                chunk: TextChunk {
                    index: 1,
                    text: String::from("out of order"),
                    last: false,
                },
            })
            .unwrap_err(),
        MailboxError::OutOfOrderChunk {
            expected: 0,
            actual: 1,
        }
    );
    for chunk in [
        TextChunk {
            index: 0,
            text: String::from("Hello "),
            last: false,
        },
        TextChunk {
            index: 1,
            text: String::from("world"),
            last: true,
        },
    ] {
        mailbox
            .apply(MailboxEvent::MessageBodyChunk {
                operation,
                message: message_id("message-1"),
                chunk,
            })
            .unwrap();
    }
    assert!(matches!(
        mailbox.selected(),
        SelectedMessage::Ready(OpenMessage {
            envelope: actual,
            plain_body,
        }) if actual == &envelope && plain_body == "Hello world"
    ));
}

#[test]
fn mutation_intents_update_or_remove_only_after_provider_confirmation() {
    let mut mailbox = subscribed_mailbox();
    first_page(&mut mailbox, vec![summary("one", 1), summary("two", 2)]);

    let mark = mailbox
        .issue(MailboxCommand::MarkRead {
            messages: vec![message_id("one"), message_id("one")],
            read: true,
        })
        .unwrap();
    assert!(matches!(
        mark,
        MailboxIntent::MarkRead { ref messages, read: true, .. } if messages.len() == 1
    ));
    assert!(
        !mailbox.list().messages[0].read,
        "mutations are confirmed, not guessed"
    );
    let mut read = summary("one", 1);
    read.read = true;
    mailbox
        .apply(MailboxEvent::MessagesChanged {
            operation: mark.operation(),
            updated: vec![read],
            removed: Vec::new(),
        })
        .unwrap();
    assert!(mailbox.list().messages[0].read);

    let star = mailbox
        .issue(MailboxCommand::Star {
            messages: vec![message_id("one")],
            starred: true,
        })
        .unwrap();
    assert!(matches!(star, MailboxIntent::Star { starred: true, .. }));
    let archive = mailbox
        .issue(MailboxCommand::Archive {
            messages: vec![message_id("one")],
        })
        .unwrap();
    assert!(matches!(archive, MailboxIntent::Archive { .. }));
    mailbox
        .apply(MailboxEvent::MessagesChanged {
            operation: archive.operation(),
            updated: Vec::new(),
            removed: vec![message_id("one")],
        })
        .unwrap();
    assert_eq!(mailbox.list().messages.len(), 1);

    let trash = mailbox
        .issue(MailboxCommand::Trash {
            messages: vec![message_id("two")],
        })
        .unwrap();
    assert!(matches!(trash, MailboxIntent::Trash { .. }));
}

#[test]
fn compose_save_and_send_preserve_all_address_fields_and_plain_body() {
    let mut mailbox = subscribed_mailbox();
    let mut draft = ComposeDraft {
        id: None,
        provider_revision: None,
        in_reply_to: Some(message_id("original")),
        to: vec![address("to@example.com")],
        cc: vec![address("cc@example.com")],
        bcc: vec![address("bcc@example.com")],
        subject: String::from("Phase 1"),
        plain_body: String::from("Written through a shared RicePaper document."),
        dirty: true,
    };
    let save = mailbox
        .issue(MailboxCommand::SaveDraft {
            draft: draft.clone(),
        })
        .unwrap();
    assert!(matches!(
        save,
        MailboxIntent::SaveDraft { ref draft, .. }
            if draft.to.len() == 1 && draft.cc.len() == 1 && draft.bcc.len() == 1
                && draft.plain_body.starts_with("Written")
    ));
    mailbox
        .apply(MailboxEvent::DraftSaved {
            operation: save.operation(),
            id: DraftId::new("draft-7").unwrap(),
            revision: Some(ProviderRevision::new("revision-2").unwrap()),
        })
        .unwrap();
    assert_eq!(
        mailbox.compose().draft.id.as_ref().unwrap().as_str(),
        "draft-7"
    );
    assert!(!mailbox.compose().draft.dirty);

    draft.id = mailbox.compose().draft.id.clone();
    draft.provider_revision = mailbox.compose().draft.provider_revision.clone();
    let send = mailbox
        .issue(MailboxCommand::Send {
            draft: draft.clone(),
        })
        .unwrap();
    assert!(matches!(
        mailbox.compose().status,
        ComposeStatus::Sending(_)
    ));
    assert_eq!(
        mailbox.edit_compose(draft).unwrap_err(),
        MailboxError::ComposeBusy
    );
    mailbox
        .apply(MailboxEvent::Sent {
            operation: send.operation(),
            message: None,
        })
        .unwrap();
    assert_eq!(mailbox.compose(), &ComposeState::default());

    assert_eq!(
        mailbox
            .issue(MailboxCommand::Send {
                draft: ComposeDraft::default(),
            })
            .unwrap_err(),
        MailboxError::NoRecipients
    );
}

#[test]
fn provider_failures_end_pending_sync_and_remain_retryable() {
    let mut mailbox = subscribed_mailbox();
    let sync = mailbox
        .issue(MailboxCommand::Sync {
            reason: SyncReason::Periodic,
        })
        .unwrap();
    assert!(matches!(
        mailbox.sync().phase,
        SyncPhase::Syncing(SyncReason::Periodic)
    ));
    mailbox
        .apply(MailboxEvent::OperationFailed {
            operation: sync.operation(),
            failure: MailboxFailure {
                code: String::from("offline"),
                message: String::from("network transport unavailable"),
                retryable: true,
            },
        })
        .unwrap();
    assert!(matches!(
        mailbox.sync().phase,
        SyncPhase::Failed(SyncFailure {
            retryable: true,
            ..
        })
    ));
    assert_eq!(mailbox.last_failure().unwrap().code, "offline");
}
