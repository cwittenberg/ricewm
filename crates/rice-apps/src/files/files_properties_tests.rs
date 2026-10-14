use super::*;
use rice_files::GuestPath;
use rice_native_app_protocol::{DirectoryEntry, VfsError, VfsErrorCode};
use rice_ui::Patch;

fn request(value: u64) -> RequestId {
    RequestId::new(value).expect("request")
}

fn entry(path: &str, kind: EntryKind, mode: u32) -> EntryMetadata {
    EntryMetadata::new(
        GuestPath::absolute(path).expect("guest path"),
        kind,
        1_536,
        1_704_067_201_234,
    )
    .with_mode(mode)
}

fn metadata(kind: FileKind, mode: u32) -> FileMetadata {
    FileMetadata {
        kind,
        size: 1_536,
        modified_millis: 1_704_067_201_234,
        mode,
    }
}

fn response(id: u64, result: VfsResult) -> VfsResponse {
    VfsResponse {
        request: request(id),
        result,
    }
}

fn directory_entry(name: &str, kind: FileKind, size: u64) -> DirectoryEntry {
    DirectoryEntry {
        name: name.to_owned(),
        metadata: FileMetadata {
            kind,
            size,
            modified_millis: 0,
            mode: 0o755,
        },
        mime_type: None,
    }
}

#[test]
fn validates_octal_modes_and_formats_special_bits() {
    assert_eq!(parse_mode("0644"), Ok(0o644));
    assert_eq!(parse_mode("755"), Ok(0o755));
    assert_eq!(parse_mode("0o4755"), Ok(0o4755));
    assert!(parse_mode("0888").is_err());
    assert!(parse_mode("64").is_err());
    assert_eq!(symbolic_mode(0o4755, EntryKind::File), "-rwsr-xr-x");
    assert_eq!(symbolic_mode(0o1770, EntryKind::Directory), "drwxrwx--T");
}

#[test]
fn stat_then_set_mode_requires_read_back_before_success() {
    let mut state = PropertiesState::new(
        vec![entry("/root/code.rs", EntryKind::File, 0o644)],
        request(1),
    );
    assert_eq!(state.stage, Stage::Loading);
    assert!(state.awaits(request(1)));
    assert_eq!(
        state.accept(&response(
            1,
            VfsResult::Metadata(metadata(FileKind::File, 0o640))
        )),
        None
    );
    assert_eq!(state.stage, Stage::Ready);
    assert_eq!(state.mode_input, "0640");
    assert_eq!(state.metadata.as_ref().map(|value| value.mode), Some(0o640));

    state.mode_input = String::from("0600");
    assert!(state.apply_enabled());
    state.stage = Stage::Applying;
    state.request = Some(request(2));
    state.requested_mode = Some(0o600);
    let follow_up = state.accept(&response(2, VfsResult::Done));
    assert_eq!(
        follow_up,
        Some(VfsOperation::Stat {
            path: String::from("/root/code.rs")
        })
    );
    assert_eq!(state.stage, Stage::Verifying);
    assert!(
        !state
            .feedback
            .as_deref()
            .is_some_and(|text| text.contains("changed"))
    );

    assert_eq!(
        state.accept(&response(
            2,
            VfsResult::Metadata(metadata(FileKind::File, 0o600))
        )),
        None
    );
    assert_eq!(state.stage, Stage::Ready);
    assert_eq!(state.mode_input, "0600");
    assert_eq!(
        state.feedback.as_deref(),
        Some("Permissions changed to 0600")
    );
}

#[test]
fn denial_is_visible_and_never_claims_success() {
    let mut state = PropertiesState::new(
        vec![entry("/root/private", EntryKind::File, 0o644)],
        request(1),
    );
    state.accept(&response(
        1,
        VfsResult::Metadata(metadata(FileKind::File, 0o644)),
    ));
    state.mode_input = String::from("0600");
    state.stage = Stage::Applying;
    state.request = Some(request(2));
    state.requested_mode = Some(0o600);
    assert_eq!(
        state.accept(&response(
            2,
            VfsResult::Error(VfsError {
                code: VfsErrorCode::PermissionDenied,
                message: String::from("read-only mount"),
            })
        )),
        None
    );
    assert_eq!(state.stage, Stage::Ready);
    assert_eq!(state.metadata.as_ref().map(|value| value.mode), Some(0o644));
    let feedback = state.feedback.as_deref().expect("denial feedback");
    assert!(feedback.contains("permission denied"));
    assert!(feedback.contains("read-only mount"));
    assert!(!feedback.contains("changed to"));
}

#[test]
fn rendered_metadata_includes_mime_timestamp_count_and_link_target() {
    let target = GuestPath::absolute("/root/photo.png").expect("target");
    let linked = entry("/root/current.png", EntryKind::Symlink, 0o777)
        .with_media_type("image/png")
        .with_symlink_target(target);
    let mut state = PropertiesState::new(vec![linked], request(1));
    state.accept(&response(
        1,
        VfsResult::Metadata(metadata(FileKind::Symlink, 0o777)),
    ));
    let values = rows(&state);
    for expected in [
        ("Selected", "1 item"),
        ("Type", "Symbolic link"),
        ("Path", "/root/current.png"),
        ("MIME type", "image/png"),
        ("Link target", "/root/photo.png"),
    ] {
        assert!(
            values
                .iter()
                .any(|(label, value)| { *label == expected.0 && value == expected.1 })
        );
    }
    assert!(
        values
            .iter()
            .any(|(label, value)| { *label == "Modified" && value == "1/1/2024, 00:00:01" })
    );

    let mut rendered = Vec::new();
    patches(&mut rendered, &state, false);
    assert!(rendered.iter().any(|patch| {
        matches!(
            patch,
            Patch::Create { widget, .. }
                if widget.id.get() == MODE_FIELD
                    && widget.kind == WidgetKind::Label
                    && widget.text.as_deref().is_some_and(|text| text.contains("read-only"))
        )
    }));
    assert!(!rendered.iter().any(|patch| {
        matches!(
            patch,
            Patch::Create { widget, .. }
                if widget.id.get() == APPLY_MODE
        )
    }));
}

#[test]
fn directory_properties_open_before_recursive_totals_finish() {
    let mut state = PropertiesState::new(
        vec![entry("/root/Projects", EntryKind::Directory, 0o755)],
        request(1),
    );
    assert_eq!(state.title(), "Properties: Projects");
    assert_eq!(
        state.accept(&response(
            1,
            VfsResult::Metadata(metadata(FileKind::Directory, 0o755))
        )),
        Some(VfsOperation::ReadDirectory {
            path: String::from("/root/Projects"),
            cursor: 0,
            limit: 128,
        })
    );
    assert!(
        rows(&state)
            .iter()
            .any(|(label, value)| { *label == "Contents" && value.starts_with("Calculating…") })
    );

    assert_eq!(
        state.accept(&response(
            1,
            VfsResult::Directory {
                entries: vec![
                    directory_entry("README.md", FileKind::File, 100),
                    directory_entry("src", FileKind::Directory, 0),
                ],
                next_cursor: None,
            }
        )),
        Some(VfsOperation::ReadDirectory {
            path: String::from("/root/Projects/src"),
            cursor: 0,
            limit: 128,
        })
    );
    assert_eq!(
        state.accept(&response(
            1,
            VfsResult::Directory {
                entries: vec![directory_entry("main.rs", FileKind::File, 200)],
                next_cursor: None,
            }
        )),
        None
    );
    let values = rows(&state);
    assert!(
        values
            .iter()
            .any(|(label, value)| { *label == "Contents" && value == "Files: 2; folders: 1" })
    );
    assert!(
        values
            .iter()
            .any(|(label, value)| { *label == "Contents size" && value == "300 B (300 bytes)" })
    );
}

#[test]
fn script_permissions_render_friendly_access_and_executable_controls() {
    let mut state = PropertiesState::new(
        vec![entry("/root/change_wallpaper.sh", EntryKind::File, 0o644)],
        request(1),
    );
    state.accept(&response(
        1,
        VfsResult::Metadata(metadata(FileKind::File, 0o644)),
    ));
    let mut rendered = Vec::new();
    patches(&mut rendered, &state, false);

    assert!(rendered.iter().any(|patch| matches!(
        patch,
        Patch::Create { widget, .. }
            if widget.id.get() == ACCESS_MENU
                && widget.text.as_deref() == Some("Read and write")
    )));
    assert!(rendered.iter().any(|patch| matches!(
        patch,
        Patch::Create { widget, .. }
            if widget.id.get() == DETAILS && widget.text.is_none()
    )));
    assert!(rendered.iter().any(|patch| matches!(
        patch,
        Patch::Create { widget, .. }
            if widget.id.get() == PERMISSION_HEADING
                && widget.text.as_deref() == Some("Permissions")
    )));
    assert!(rendered.iter().any(|patch| matches!(
        patch,
        Patch::Create { widget, .. }
            if widget.id.get() == PERMISSION_SYMBOLIC
                && widget.text.as_deref() == Some("-rw-r--r--")
    )));
    assert!(rendered.iter().any(|patch| matches!(
        patch,
        Patch::Create { widget, .. }
            if widget.id.get() == EXECUTABLE
                && widget.kind == WidgetKind::Toggle
                && !widget.state.contains(WidgetState::CHECKED)
    )));

    state.update_metadata(&metadata(FileKind::File, 0o755));
    let mut executable = Vec::new();
    patches(&mut executable, &state, false);
    assert!(executable.iter().any(|patch| matches!(
        patch,
        Patch::Create { widget, .. }
            if widget.id.get() == MODE_FIELD && widget.text.as_deref() == Some("0755")
    )));
    assert!(executable.iter().any(|patch| matches!(
        patch,
        Patch::Create { widget, .. }
            if widget.id.get() == EXECUTABLE
                && widget.state.contains(WidgetState::CHECKED)
    )));
}

#[test]
fn redmond_properties_expose_only_real_editable_metadata() {
    let mut state = PropertiesState::new(
        vec![entry("/root/.bashrc", EntryKind::File, 0o644)],
        request(1),
    );
    state.accept(&response(
        1,
        VfsResult::Metadata(metadata(FileKind::File, 0o644)),
    ));
    let mut rendered = Vec::new();
    patches(&mut rendered, &state, true);
    let texts = rendered
        .iter()
        .filter_map(|patch| match patch {
            Patch::Create { widget, .. } => widget.text.as_deref(),
            _ => None,
        })
        .collect::<Vec<_>>();
    for fake in ["Sharing", "Web Sharing", "MS-DOS name:", "Created:"] {
        assert!(
            !texts.contains(&fake),
            "fabricated property survived: {fake}"
        );
    }
    for (id, kind) in [
        (NAME_FIELD, WidgetKind::TextField),
        (COPY_LOCATION, WidgetKind::Button),
        (READ_ONLY_ATTRIBUTE, WidgetKind::CheckBox),
        (HIDDEN_ATTRIBUTE, WidgetKind::CheckBox),
        (EXECUTABLE, WidgetKind::CheckBox),
        (MODE_FIELD, WidgetKind::TextField),
    ] {
        assert!(
            rendered.iter().any(|patch| matches!(
                patch,
                Patch::Create { widget, .. } if widget.id.get() == id && widget.kind == kind
            )),
            "missing real property control {id}"
        );
    }
}

#[test]
fn rename_then_mode_change_is_a_verified_vfs_sequence() {
    let mut state = PropertiesState::new(
        vec![entry("/root/old.sh", EntryKind::File, 0o644)],
        request(1),
    );
    state.accept(&response(
        1,
        VfsResult::Metadata(metadata(FileKind::File, 0o644)),
    ));
    state.name_input = String::from("new.sh");
    state.mode_input = String::from("0755");
    state.stage = Stage::Renaming;
    state.requested_path = Some(GuestPath::absolute("/root/new.sh").expect("new path"));
    state.requested_mode = Some(0o755);
    assert_eq!(
        state.accept(&response(2, VfsResult::Done)),
        Some(VfsOperation::SetMode {
            path: String::from("/root/new.sh"),
            mode: 0o755,
        })
    );
    assert_eq!(state.entries[0].path.as_str(), "/root/new.sh");
    assert_eq!(state.entries[0].name, "new.sh");
    assert_eq!(state.stage, Stage::Applying);
}


#[test]
fn localized_property_titles_and_access_labels_preserve_metadata() {
    let entry = EntryMetadata::new(GuestPath::absolute("/root/Properties {name}.txt").unwrap(), EntryKind::File, 7, 0);
    let state = PropertiesState::new(vec![entry], RequestId::new(901).unwrap());
    rice_i18n::with_locale(2, || {
        assert_eq!(state.title(), "属性：Properties {name}.txt");
        assert_eq!(state.redmond_title(), "Properties {name}.txt 属性");
        assert_eq!(owner_access_label(0o600), "读写");
        assert_eq!(owner_access_label(0o400), "只读");
        assert_eq!(owner_access_label(0o200), "只写");
        assert_eq!(owner_access_label(0), "无访问权限");
        assert_eq!(state.entries[0].path.as_str(), "/root/Properties {name}.txt");
        assert_eq!(state.entries[0].name, "Properties {name}.txt");
    });
    rice_i18n::with_locale(1, || assert_eq!(state.title(), "Propiedades: Properties {name}.txt"));
}

#[test]
fn chinese_folder_totals_translate_counts_and_partial_scan_notices() {
    rice_i18n::with_locale(2, || {
        let mut state = PropertiesState::new(vec![entry("/root/Projects", EntryKind::Directory, 0o755)], request(902));
        state.folder_scan = None;
        for count in [0, 1, 2, 101] {
            state.folder_totals = Some(FolderTotals {files:count,folders:1,bytes:300,unreadable_folders:0});
            let (contents,size)=folder_rows(&state);
            assert_eq!(contents,format!("文件：{count}；文件夹：1"));
            assert_eq!(size,"300 B（300 字节）");
        }
        state.folder_totals.as_mut().unwrap().unreadable_folders=1;
        assert!(folder_rows(&state).0.ends_with("（无法读取部分文件夹）"));
        assert_eq!(state.folder_totals.as_ref().unwrap().bytes,300);
    });
}

#[test]
fn translated_permission_validation_keeps_octal_rules_and_diagnostics() {
    rice_i18n::with_locale(2, || {
        assert_eq!(parse_mode("0644"),Ok(0o644));
        assert_eq!(parse_mode("0o755"),Ok(0o755));
        assert_eq!(parse_mode("888"),Err("请输入三位或四位八进制数字（例如 0644）"));
        let error=VfsError {code:VfsErrorCode::PermissionDenied,message:"/root/{category}.txt".into()};
        assert_eq!(vfs_error(&error),"权限被拒绝：/root/{category}.txt");
        assert_eq!(error.code,VfsErrorCode::PermissionDenied);
        assert_eq!(symbolic_mode(0o755,EntryKind::Directory),"drwxr-xr-x");
    });
}

#[test]
fn static_feedback_retranslates_without_changing_operation_success_state() {
    let mut state=PropertiesState::new(vec![entry("/root/Save.txt",EntryKind::File,0o644)],request(903));
    state.feedback=Some("Changes applied".into());
    for (locale,expected) in [(0,"Changes applied"),(1,"Cambios aplicados"),(2,"已应用更改")] {
        rice_i18n::with_locale(locale, || {
            let mut output=Vec::new();
            patches(&mut output,&state,false);
            assert!(output.iter().any(|patch| matches!(patch,Patch::Create {widget,..}
                if widget.id.get()==FEEDBACK && widget.text.as_deref()==Some(expected))));
            assert_eq!(state.feedback.as_deref(),Some("Changes applied"));
            assert_eq!(feedback_icon(state.feedback.as_deref().unwrap()),IconId::Information);
        });
    }
}

#[test]
fn permission_feedback_refreshes_language_without_altering_success_checks() {
    let feedback=Feedback::formatted("Permissions changed to {mode}",vec![("mode","0644".into())]);
    for (locale,expected) in [(0,"Permissions changed to 0644"),(1,"Permisos cambiados a 0644"),(2,"权限已更改为 0644")] {
        rice_i18n::with_locale(locale, || {
            assert_eq!(feedback.render(),expected);
            assert_eq!(&*feedback,"Permissions changed to 0644");
            assert_eq!(feedback_icon(&feedback),IconId::Information);
        });
    }
    let mismatch=Feedback::formatted("Verification failed: requested {requested}, host reported {reported}",vec![("requested","0755".into()),("reported","0644".into())]);
    rice_i18n::with_locale(2, || {
        assert_eq!(mismatch.render(),"验证失败：请求的权限为 0755，系统报告的权限为 0644");
        assert_eq!(feedback_icon(&mismatch),IconId::Warning);
    });
}

#[test]
fn filesystem_feedback_retranslates_category_and_preserves_host_message() {
    let error=VfsError {code:VfsErrorCode::PermissionDenied,message:"/root/{error}.txt".into()};
    let feedback=rice_i18n::with_locale(2, || Feedback::failure("Name was not changed: {error}",&error));
    assert_eq!(&*feedback,"Name was not changed: permission denied: /root/{error}.txt");
    for (locale,expected) in [
        (0,"Name was not changed: permission denied: /root/{error}.txt"),
        (1,"No se cambió el nombre: permiso denegado: /root/{error}.txt"),
        (2,"未更改名称：权限被拒绝：/root/{error}.txt"),
    ] {
        rice_i18n::with_locale(locale, || assert_eq!(feedback.render(),expected));
    }
    assert_eq!(feedback.error.as_ref(),Some(&error));
    assert_eq!(feedback_icon(&feedback),IconId::Warning);
}

#[test]
fn chinese_symlink_properties_preserve_read_only_mode_and_target() {
    let path=GuestPath::absolute("/root/Target {mode}").unwrap();
    let mut state=PropertiesState::new(vec![entry("/root/Link",EntryKind::Symlink,0o777).with_symlink_target(path.clone())],request(904));
    state.metadata=Some(metadata(FileKind::Symlink,0o777));
    state.stage=Stage::Ready;
    rice_i18n::with_locale(2, || {
        let mut output=Vec::new();patches(&mut output,&state,false);
        assert!(output.iter().any(|patch| matches!(patch,Patch::Create {widget,..}
            if widget.id.get()==MODE_FIELD && widget.kind==WidgetKind::Label && widget.text.as_deref()==Some("0777（只读符号链接）"))));
        assert_eq!(state.entries[0].symlink_target.as_ref(),Some(&path));
        assert_eq!(state.metadata.as_ref().unwrap().mode,0o777);
    });
}


#[test]
fn invalid_rename_path_details_refresh_without_changing_input_or_metadata() {
    for name in ["Save\0{name}".to_owned(), "x".repeat(GuestPath::MAX_BYTES + 1)] {
        let (mut actor, _) = super::super::tests::loaded_actor();
        let mut state = PropertiesState::new(vec![entry("/root/Save.txt", EntryKind::File, 0o644)], request(901));
        state.metadata = Some(metadata(FileKind::File, 0o644));
        state.stage = Stage::Ready;
        state.name_input = name.clone();
        actor.dialog = Some(Dialog::Properties(state));
        rice_i18n::with_locale(2, || actor.apply_property_changes(request(902))).unwrap();
        let Some(Dialog::Properties(state)) = actor.dialog.as_ref() else { panic!("properties"); };
        let expected_error = GuestPath::absolute("/root").unwrap().join(&name).unwrap_err();
        assert_eq!(state.feedback.as_ref().unwrap().path_error, Some(expected_error));
        let initial_fallback = state.feedback.as_ref().unwrap().fallback.clone();
        for &locale in rice_i18n::LOCALE_CODES {
            rice_i18n::with_locale(locale, || {
                let expected = rice_i18n::format("Name is invalid: {error}", &[("error", &navigation_error(expected_error))]);
                let mut output = Vec::new();
                patches(&mut output, state, true);
                let text = |id| output.iter().find_map(|patch| match patch {
                    Patch::Create { widget, .. } if widget.id.get() == id => widget.text.as_deref(),
                    _ => None,
                });
                assert_eq!(text(FEEDBACK), Some(expected.as_str()));
                assert_eq!(text(NAME_FIELD), Some(name.as_str()));
                assert_eq!(state.entries[0].path.as_str(), "/root/Save.txt");
                assert_eq!(state.metadata.as_ref().unwrap().mode, 0o644);
                assert_eq!(state.feedback.as_ref().unwrap().fallback, initial_fallback);
                if locale != 0 { assert_ne!(expected, initial_fallback); }
            });
        }
    }
}
