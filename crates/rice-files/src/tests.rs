use super::*;

fn path(value: &str) -> GuestPath {
    GuestPath::absolute(value).expect("valid test path")
}

fn entry(value: &str, kind: EntryKind, size: u64, modified: i64) -> EntryMetadata {
    EntryMetadata::new(path(value), kind, size, modified)
}

#[test]
fn guest_paths_normalize_without_touching_a_filesystem() {
    let base = path("/home/rice/docs");
    assert_eq!(
        GuestPath::resolve(&base, ".././pics//one.png")
            .expect("relative path")
            .as_str(),
        "/home/rice/pics/one.png"
    );
    assert_eq!(
        GuestPath::resolve(&base, "../../../../etc").expect("root-clamped path"),
        path("/etc")
    );
    assert_eq!(
        GuestPath::resolve(&base, "").expect("empty location input"),
        base
    );
    assert_eq!(path("/").parent(), path("/"));
    assert!(path("/home/rice").is_within(&path("/home")));
    assert!(!path("/homegrown").is_within(&path("/home")));
    assert_eq!(
        GuestPath::absolute("relative"),
        Err(NavigationError::AbsolutePathRequired)
    );
    assert_eq!(
        GuestPath::absolute("/bad\0path"),
        Err(NavigationError::NulByte)
    );
}

#[test]
fn navigation_plans_are_adapter_confirmed_and_reject_stale_commits() {
    let home = path("/home/rice");
    let mut pane = PaneState::new(path("/home/rice/docs"));
    let plan = pane
        .plan_navigation(NavigationCommand::Enter("../pictures".into()), &home)
        .expect("navigation plan");
    assert_eq!(plan.target(), &path("/home/rice/pictures"));
    assert!(plan.intent.follow_symlinks);
    pane.commit_navigation(&plan, path("/mnt/photos"))
        .expect("adapter-resolved symlink");
    assert_eq!(pane.location(), &path("/mnt/photos"));

    let back = pane
        .plan_navigation(NavigationCommand::Back, &home)
        .expect("back plan");
    let reload = pane
        .plan_navigation(NavigationCommand::Reload, &home)
        .expect("reload plan");
    pane.commit_navigation(&reload, path("/mnt/photos"))
        .expect("reload");
    assert_eq!(
        pane.commit_navigation(&back, path("/home/rice/docs")),
        Err(NavigationError::StalePlan)
    );
    assert_eq!(pane.reload_generation, 1);
}

#[test]
fn tabs_and_dual_panes_keep_independent_state_and_reopen() {
    let mut manager = FileManager::new();
    let window_id = manager.open_window(path("/home/rice"));
    let window = manager.window_mut(window_id).expect("window");
    let second_id = window.open_tab(path("/tmp"));
    let tab = window.active_tab_mut();
    tab.set_dual_pane(true);
    tab.secondary.set_view(ViewMode::Details);
    tab.secondary.set_selection([path("/tmp/two")]);
    tab.secondary.set_scroll(ScrollPosition {
        horizontal: 3,
        vertical: 80,
        anchor: Some(path("/tmp/two")),
    });
    tab.primary.set_view(ViewMode::Icons);
    assert_eq!(tab.primary.location(), &path("/tmp"));
    assert_eq!(tab.secondary.view, ViewMode::Details);
    assert_eq!(tab.primary.selection().count(), 0);

    window.close_tab(second_id).expect("close second tab");
    assert_eq!(window.tabs().len(), 1);
    assert_eq!(window.reopen_tab().expect("reopen tab"), second_id);
    assert_eq!(
        window.active_tab().secondary.scroll.anchor,
        Some(path("/tmp/two"))
    );

    manager.close_window(window_id).expect("close window");
    let reopened = manager.reopen_window().expect("reopen window");
    assert_ne!(reopened, window_id);
    assert_eq!(manager.windows()[0].tabs().len(), 2);
}

#[test]
fn view_changes_and_updates_preserve_surviving_selection_and_scroll() {
    let mut pane = PaneState::new(path("/data"));
    pane.set_selection([path("/data/a"), path("/data/gone")]);
    pane.set_scroll(ScrollPosition {
        horizontal: 0,
        vertical: 240,
        anchor: Some(path("/data/a")),
    });
    pane.set_view(ViewMode::Thumbnails);
    pane.reconcile_entries(&[
        entry("/data/a", EntryKind::File, 1, 1),
        entry("/data/b", EntryKind::File, 2, 2),
    ]);
    assert_eq!(
        pane.selection().cloned().collect::<Vec<_>>(),
        [path("/data/a")]
    );
    assert_eq!(pane.scroll.vertical, 240);
    assert_eq!(pane.scroll.anchor, Some(path("/data/a")));

    pane.reconcile_entries(&[entry("/data/b", EntryKind::File, 2, 2)]);
    assert!(pane.selection().next().is_none());
    assert_eq!(pane.scroll.anchor, None);
}

#[test]
fn natural_sort_is_stable_and_deterministic_for_all_keys() {
    let mut entries = vec![
        entry("/d/file10.txt", EntryKind::File, 20, 4).with_media_type("text/plain"),
        entry("/d/file2.txt", EntryKind::File, 30, 1).with_media_type("text/plain"),
        entry("/d/File1.txt", EntryKind::File, 10, 3).with_media_type("text/plain"),
        entry("/d/file01.txt", EntryKind::File, 10, 2).with_media_type("text/plain"),
        entry("/d/folder", EntryKind::Directory, 0, 9),
    ];
    sort_entries(&mut entries, SortStrategy::default());
    assert_eq!(
        entries
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>(),
        [
            "folder",
            "File1.txt",
            "file01.txt",
            "file2.txt",
            "file10.txt"
        ]
    );

    sort_entries(
        &mut entries,
        SortStrategy {
            key: SortKey::Size,
            direction: SortDirection::Descending,
            directories_first: true,
        },
    );
    assert_eq!(entries[0].name, "folder");
    assert_eq!(entries[1].name, "file2.txt");

    for key in [
        SortKey::Name,
        SortKey::Type,
        SortKey::Size,
        SortKey::Extension,
        SortKey::Modified,
    ] {
        sort_entries(
            &mut entries,
            SortStrategy {
                key,
                direction: SortDirection::Ascending,
                directories_first: false,
            },
        );
        assert_eq!(entries.len(), 5);
    }
}

#[test]
fn preferences_keep_per_folder_views_and_missing_bookmarks() {
    let mut preferences = FilePreferences::default();
    preferences.show_hidden = true;
    preferences
        .set_folder(
            path("/pictures"),
            FolderPreferences {
                view: ViewMode::Thumbnails,
                sort: SortStrategy {
                    key: SortKey::Modified,
                    direction: SortDirection::Descending,
                    directories_first: true,
                },
                show_hidden: Some(false),
            },
        )
        .expect("folder preference");
    assert_eq!(
        preferences.folder(&path("/pictures")).view,
        ViewMode::Thumbnails
    );
    assert!(!preferences.shows_hidden_in(&path("/pictures")));
    assert!(preferences.shows_hidden_in(&path("/other")));

    let bookmark = Bookmark::new(path("/gone"), "Old disk").expect("bookmark");
    preferences.add_bookmark(bookmark).expect("add bookmark");
    preferences
        .set_bookmark_availability(&path("/gone"), BookmarkAvailability::Missing)
        .expect("mark missing");
    assert_eq!(
        preferences.bookmarks()[0].availability,
        BookmarkAvailability::Missing
    );
    assert!(matches!(
        preferences.persist_intent(),
        PreferencesIntent::Persist(_)
    ));
}

#[test]
fn operation_commands_validate_scope_and_collapse_nested_sources() {
    let scope = OperationScope::new(vec![path("/home/rice")]).expect("scope");
    let folder = entry("/home/rice/source", EntryKind::Directory, 0, 1);
    let child = entry("/home/rice/source/child", EntryKind::File, 4, 2);
    let command = OperationCommand::copy(&scope, vec![child, folder], path("/home/rice/target"))
        .expect("copy command");
    let plan = OperationPlan::new(CommandId::new(9).expect("id"), command);
    assert_eq!(plan.intents().len(), 1);
    assert!(matches!(
        &plan.intents()[0],
        AdapterIntent::Copy { source, to }
            if source.path == path("/home/rice/source")
                && *to == path("/home/rice/target/source")
    ));

    assert_eq!(
        OperationCommand::move_to(
            &scope,
            vec![entry("/home/rice/source", EntryKind::Directory, 0, 1)],
            path("/home/rice/source/inside")
        ),
        Err(OperationError::TargetInsideSource)
    );
    assert!(matches!(
        OperationCommand::delete(&scope, vec![entry("/etc/passwd", EntryKind::File, 1, 1)]),
        Err(OperationError::OutsideScope(_))
    ));
    assert_eq!(
        OperationCommand::create_file(&scope, path("/")),
        Err(OperationError::OutsideScope(path("/")))
    );
}

#[test]
fn archive_commands_are_bounded_typed_and_scope_checked() {
    let scope = OperationScope::new(vec![path("/home/rice")]).expect("scope");
    let source = entry("/home/rice/report.txt", EntryKind::File, 12, 1);
    let command = OperationCommand::archive(
        &scope,
        vec![source.clone()],
        path("/home/rice/report.txt.zip"),
        ArchiveFormat::Zip,
    )
    .expect("ZIP command");
    let plan = OperationPlan::new(CommandId::new(90).expect("id"), command);
    assert!(matches!(
        plan.intents(),
        [AdapterIntent::Archive { sources, to, format }]
            if sources == &[source.clone()]
                && *to == path("/home/rice/report.txt.zip")
                && *format == ArchiveFormat::Zip
    ));

    assert_eq!(
        OperationCommand::archive(
            &scope,
            vec![
                source.clone(),
                entry("/home/rice/other", EntryKind::File, 2, 1),
            ],
            path("/home/rice/output.gz"),
            ArchiveFormat::Gzip,
        ),
        Err(OperationError::GzipRequiresOneFile)
    );
    assert_eq!(
        OperationCommand::extract(
            &scope,
            entry("/home/rice/folder", EntryKind::Directory, 0, 1),
            path("/home/rice"),
        ),
        Err(OperationError::ArchiveMustBeFile)
    );
    assert!(OperationCommand::extract(&scope, source, path("/home/rice")).is_ok());
}

#[test]
fn all_operation_kinds_emit_only_adapter_intents() {
    let scope = OperationScope::new(vec![path("/work"), path("/trash")]).expect("scope");
    let source = entry("/work/a", EntryKind::File, 5, 8);
    let cases = vec![
        OperationCommand::create_file(&scope, path("/work/new")).expect("create"),
        OperationCommand::create_directory(&scope, path("/work/folder")).expect("mkdir"),
        OperationCommand::rename(&scope, source.clone(), path("/work/b")).expect("rename"),
        OperationCommand::move_to(&scope, vec![source.clone()], path("/work/d")).expect("move"),
        OperationCommand::link(&scope, vec![source.clone()], path("/work/d")).expect("link"),
        OperationCommand::trash(&scope, vec![source.clone()]).expect("trash"),
        OperationCommand::delete(&scope, vec![source.clone()]).expect("delete"),
        OperationCommand::restore(
            &scope,
            entry("/trash/a", EntryKind::File, 5, 8),
            path("/work/a"),
        )
        .expect("restore"),
    ];
    for (index, command) in cases.into_iter().enumerate() {
        let plan = OperationPlan::new(
            CommandId::new(u64::try_from(index + 1).expect("small index")).expect("id"),
            command,
        );
        assert_eq!(plan.intents().len(), 1);
    }
}

#[test]
fn conflict_strategy_supports_apply_all_rename_and_cancel() {
    let destination = path("/target/name");
    let mut strategy = ConflictStrategy::default();
    assert_eq!(
        strategy
            .decide(
                &destination,
                ConflictDecision::Rename(path("/target/name (copy)"))
            )
            .expect("rename"),
        ConflictAction::Rename(path("/target/name (copy)"))
    );
    assert_eq!(
        strategy.decide(
            &destination,
            ConflictDecision::Rename(path("/elsewhere/name"))
        ),
        Err(ConflictError::InvalidRename)
    );
    assert_eq!(
        strategy
            .decide(
                &destination,
                ConflictDecision::ApplyAll(ConflictAction::Replace)
            )
            .expect("sticky replace"),
        ConflictAction::Replace
    );
    assert_eq!(
        strategy
            .decide(&destination, ConflictDecision::Skip)
            .expect("apply-all overrides later prompt"),
        ConflictAction::Replace
    );
    assert_eq!(
        strategy.decide(&destination, ConflictDecision::Cancel),
        Err(ConflictError::Cancelled)
    );
}

#[test]
fn progress_is_bounded_cancellable_and_requires_complete_totals() {
    let id = CommandId::new(1).expect("id");
    let mut progress = ProgressState::new(id, 1, Some(10));
    progress
        .apply(ProgressEvent::StartItem(path("/work/a")))
        .expect("start");
    progress
        .apply(ProgressEvent::AdvanceBytes(10))
        .expect("bytes");
    assert_eq!(
        progress.apply(ProgressEvent::Complete),
        Err(ProgressError::Incomplete)
    );
    progress
        .apply(ProgressEvent::CompleteItem)
        .expect("complete item");
    progress.apply(ProgressEvent::Complete).expect("complete");
    assert_eq!(progress.status, ProgressStatus::Completed);
    assert_eq!(
        progress.apply(ProgressEvent::StartItem(path("/work/b"))),
        Err(ProgressError::WrongState)
    );

    let mut cancelled = ProgressState::new(id, 3, None);
    cancelled.request_cancel().expect("request cancellation");
    cancelled
        .apply(ProgressEvent::CancelDenied)
        .expect("provider denied cancellation");
    assert_eq!(cancelled.status, ProgressStatus::Running);
    cancelled.request_cancel().expect("retry cancellation");
    cancelled
        .apply(ProgressEvent::Recovery(RecoveryRecord {
            action: RecoveryAction::RemovePartial,
            path: path("/work/.partial"),
            related_path: None,
        }))
        .expect("recovery");
    cancelled
        .apply(ProgressEvent::Cancelled)
        .expect("adapter cancelled");
    assert_eq!(cancelled.status, ProgressStatus::Cancelled);
    assert_eq!(cancelled.recovery().len(), 1);
}

#[test]
fn completion_is_bounded_sorted_and_ignores_stale_generations() {
    let mut completion = DirectoryCompletion::default();
    let first = completion
        .plan(&path("/home/rice"), "/home/rice/Do")
        .expect("completion plan");
    let second = completion
        .plan(&path("/home/rice"), "/home/rice/Doc")
        .expect("newer plan");
    assert!(
        !completion
            .resolve(&first, [String::from("Downloads")])
            .expect("stale response")
    );
    assert!(
        completion
            .resolve(
                &second,
                [
                    String::from("Documents"),
                    String::from("Docker"),
                    String::from("../escape"),
                ],
            )
            .expect("current response")
    );
    assert_eq!(
        completion.suggestions(),
        &[path("/home/rice/Docker"), path("/home/rice/Documents")]
    );
}

#[test]
fn provider_search_preview_and_drop_are_adapter_intents_only() {
    let location = ProviderLocation::parse("sftp://server.example/shared").expect("provider");
    assert_eq!(location.scheme(), "sftp");
    assert_eq!(location.authority(), "server.example");
    assert_eq!(location.path(), "/shared");
    assert_eq!(
        ProviderLocation::parse("sftp://user@server/private"),
        Err(ProviderLocationError::InvalidAuthority)
    );
    let connect = ConnectIntent::new(7, location).expect("connect intent");
    assert_eq!(connect.request, 7);

    let query = SearchQuery {
        text: String::from("photo"),
        kind: Some(EntryKind::File),
        minimum_size: Some(1),
        maximum_size: Some(100),
    }
    .validate()
    .expect("search query");
    let mut search = SearchState::new(SearchIntent {
        token: 3,
        root: path("/home/rice"),
        query,
    })
    .expect("search state");
    let image = entry("/home/rice/photo.png", EntryKind::File, 40, 12);
    search.append(image.clone()).expect("search result");
    search.request_cancel().expect("cancel intent");
    assert_eq!(search.status, SearchStatus::CancelRequested);

    let preview = PreviewIntent::for_entry(&image, 99, 320, 200, 1).expect("preview intent");
    assert_eq!(preview.key().source, image.path);
    assert_eq!(preview.maximum_decoded_bytes(), 320 * 200 * 4);
    let shared_fingerprint: rice_image_contract::SourceFingerprint = preview.key().fingerprint;
    assert_eq!(shared_fingerprint.identity, 99);

    let scope = OperationScope::new(vec![path("/home/rice")]).expect("scope");
    let drop = DropPlan::new(
        CommandId::new(8).expect("id"),
        &scope,
        vec![image],
        path("/home/rice/Pictures"),
        DropAction::Move,
    )
    .expect("drop plan");
    assert!(matches!(
        drop.operation.intents(),
        [AdapterIntent::Move { .. }]
    ));
}

#[test]
fn dual_pane_split_drag_is_clamped_and_preserves_both_panes() {
    let mut tab = TabState::new(1, path("/home/rice"));
    tab.set_dual_pane(true);
    tab.secondary.set_view(ViewMode::Details);
    tab.resize_split(20);
    assert_eq!(tab.split_permille(), 150);
    tab.resize_split(900);
    assert_eq!(tab.split_permille(), 850);
    assert_eq!(tab.secondary.view, ViewMode::Details);
}
