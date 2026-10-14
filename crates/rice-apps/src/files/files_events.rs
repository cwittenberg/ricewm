//! RiceFiles event routing, separated from the actor's state and composition.

use super::*;

impl RiceFiles {
    fn quick_filter_input_allowed(&self, target: u64) -> bool {
        !matches!(
            target,
            BREADCRUMB
                | SEARCH_FIELD
                | RENAME_FIELD
                | MOVE_FIELD
                | files_dialogs::FIELD
                | files_remote::CLOUD_FIELD_FIRST..=files_remote::CLOUD_FIELD_LAST
                | files_remote::URL_FIELD
                | files_remote::USER_FIELD
                | files_remote::PASSWORD_FIELD
                        | files_remote::DOMAIN_FIELD
                        | files_remote::UID_FIELD
                        | files_remote::GID_FIELD | files_remote::BRIDGE_URL | files_remote::BRIDGE_TOKEN
                | files_properties::MODE_FIELD
                | files_open::OPEN_WITH_SEARCH
                | files_open::OPEN_WITH_COMMAND
        ) && self.dialog.is_none()
            && self.open_with.is_none()
            && !self.remote.is_open()
            && self.pending_mutation.is_none()
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn handle_event(
        &mut self,
        envelope: &UiEventEnvelope,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        if envelope.app != self.app || envelope.window != self.window {
            return Ok(Vec::new());
        }
        // App shortcuts must match the advertised menu actions independently
        // of which pane or location field currently owns keyboard focus.
        if self.options_dialog.is_none() && self.dialog.is_none() && self.open_with.is_none() && !self.remote.is_open() {
            if let UiEvent::Key { pressed: true, key_code, modifiers: 8, .. } = &envelope.event {
                match key_code {
                    28 => return self.open_tab(request),
                    57 => return Ok(vec![Message::Launch {
                        app: self.app, entrypoint: String::from(ENTRYPOINT),
                        arguments: vec![self.location().as_str().to_owned()],
                    }]),
                    25 if self.model.windows()[0].tabs().len() == 1 => return Ok(vec![Message::HostRequest(HostRequest {
                        app: self.app, window: self.window, operation: HostOperation::RequestClose,
                    })]),
                    25 => return self.close_tab(self.active_tab_id()?),
                    _ => {},
                }
            }
        }
        if self.options_dialog.is_some() {
            return files_options::event(self, envelope, request);
        }
        let raw_target = envelope.target.get();
        if let Some(files_dialogs::Dialog::About { section, scroll }) = &mut self.dialog {
            match &envelope.event {
                UiEvent::Activate if (files_dialogs::ABOUT_NAV + 20..files_dialogs::ABOUT_NAV + 23).contains(&raw_target) => {
                    *section = (raw_target - files_dialogs::ABOUT_NAV - 20) as usize;
                    *scroll = 0;
                    return self.emit_full().map(|message| vec![message]);
                }
                UiEvent::ValueChanged(value) if raw_target == files_dialogs::ABOUT_SCROLL => {
                    if let Some((position, _)) = value.split_once('/')
                        && let Ok(position) = position.parse::<u32>()
                    {
                        *scroll = position;
                    }
                    return self.emit_full().map(|message| vec![message]);
                }
                _ => {}
            }
        }

        if matches!(envelope.event, UiEvent::Activate) && (340_000..340_100).contains(&raw_target) {
            return files_user_actions::execute(self, (raw_target - 340_000) as usize, request);
        }
        if matches!(envelope.event, UiEvent::Activate) && (350_000..350_032).contains(&raw_target) {
            return files_user_actions::execute(self, (raw_target - 350_000 + 100) as usize, request);
        }
        let mut focus_changed = false;
        if let Some(slot) = files_view::pane_for_widget(raw_target) {
            focus_changed = self.focus_pane(slot)?;
        }
        let target = files_menu::action_for_widget(raw_target)
            .or_else(|| files_view::pane_action(raw_target))
            .unwrap_or(raw_target);
        let preview_key_target = self.preview_fullscreen || (210_002..=210_012).contains(&raw_target);
        let preview_seek_shortcut = matches!(self.preview_state, PreviewState::Video { .. })
            && files_view::pane_for_widget(raw_target).is_some()
            && matches!(&envelope.event, UiEvent::Key { modifiers, .. } if modifiers & 1 != 0 && modifiers & (4 | 8 | 64) == 0);
        match (&envelope.event, target) {
            (UiEvent::Pointer { kind: rice_native_app_protocol::PointerEventKind::Wheel, button: 4 | 5, modifiers, .. }, _)
                if modifiers & 4 != 0 && self.dialog.is_none() && self.open_with.is_none() && !self.remote.is_open() => {
                let UiEvent::Pointer { button, .. } = envelope.event else { unreachable!() };
                self.adjust_view_zoom(if button == 4 { 100 } else { -100 })
            }

            (UiEvent::Pointer { kind: rice_native_app_protocol::PointerEventKind::Down, button: 6, .. }, files_preview_view::PREVIEW_MEDIA) => self.toggle_preview_fullscreen(),
            (UiEvent::Key { pressed: true, key_code: 36 | 104, .. }, _) if (preview_key_target || files_view::pane_for_widget(raw_target).is_some()) && matches!(self.preview_state, PreviewState::Video { .. }) => self.open_preview_in_player(request),
            (UiEvent::Key { pressed: true, key_code: 113, modifiers, .. }, _) if preview_key_target || preview_seek_shortcut => self.seek_preview_key(false, *modifiers, request),
            (UiEvent::Key { pressed: true, key_code: 114, modifiers, .. }, _) if preview_key_target || preview_seek_shortcut => self.seek_preview_key(true, *modifiers, request),
            (UiEvent::Key { pressed: true, key_code: 65, .. }, _) if preview_key_target => self.toggle_embedded_video(),
            (UiEvent::Key { pressed: true, key_code: 9 | 27, .. }, _) if self.preview_fullscreen => self.toggle_preview_fullscreen(),
            (UiEvent::Activate, files_preview_view::PREVIEW_FULLSCREEN) => self.toggle_preview_fullscreen(),
            (UiEvent::Activate, files_preview_view::PREVIEW_OPEN_PLAYER) => self.open_preview_in_player(request),
            (UiEvent::Activate, 320_000..=320_063) => {
                if let Some(mount) = self.mount_entries.get((target - 320_000) as usize) {
                    if mount.state != MountState::Connected {
                        if mount.id.starts_with("system-") {return self.emit_notice(rice_i18n::t("This system share is offline. It remains configured in /etc/fstab."));}
                        return self.reopen_network_mount(mount.id.clone(),request);
                    }
                    let path = mount.path.clone();
                    self.begin_navigation(NavigationCommand::Enter(path), request)
                } else {Ok(Vec::new())}
            }
            (UiEvent::Activate, 320_300..=320_363) => {
                if let Some(mount)=self.mount_entries.get((target-320_300) as usize){self.reopen_network_mount(mount.id.clone(),request)}else{Ok(Vec::new())}
            }
            (UiEvent::Activate, 320_400..=320_463) => {
                if let Some(mount)=self.mount_entries.get((target-320_400) as usize){
                    let mount_id=mount.id.clone();self.pending_mount_sync=Some(request);
                    Ok(vec![Message::DataProviderRequest(DataProviderRequest {request,app:self.app,operation:DataProviderOperation::MountForgetNetwork {mount_id}})])
                }else{Ok(Vec::new())}
            }
            (UiEvent::Activate, 320_200..=320_263) => {
                if let Some(mount) = self.mount_entries.get((target - 320_200) as usize) {
                    let mount_id = mount.id.clone(); self.pending_mount_sync = Some(request);
                    Ok(vec![Message::DataProviderRequest(DataProviderRequest {request, app:self.app, operation:DataProviderOperation::MountDisconnect {mount_id}})])
                } else {Ok(Vec::new())}
            }

            (UiEvent::Activate, files_remote::CLOUD | files_remote::CLOUD_BACK) => self.show_cloud_providers(request),
            (UiEvent::Activate, files_remote::CLOUD_FIRST..=files_remote::CLOUD_LAST) => self.select_cloud_provider(target, request),
            (UiEvent::Activate, files_remote::SMB | files_remote::NFS | files_remote::WEBDAV | files_remote::BRIDGE) => self.select_remote_protocol(target),
            (UiEvent::Focus { focused: true }, _) if focus_changed => {
                self.emit_full().map(|message| vec![message])
            }
            (
                UiEvent::Pointer {
                    kind: rice_native_app_protocol::PointerEventKind::Wheel,
                    button,
                    ..
                },
                _,
            ) if files_view::pane_for_widget(raw_target).is_some() => {
                self.scroll_active_pane(*button)
            }
            (
                UiEvent::Pointer {
                    kind: rice_native_app_protocol::PointerEventKind::Up,
                    button: 1,
                    ..
                },
                target,
            ) if self.folder_tree && self.folder_tree_path_for_target(target).is_some() => {
                let path = self
                    .folder_tree_path_for_target(target)
                    .ok_or(FilesActorError::InvalidSettings)?;
                Ok(self.probe_folder_tree_children(&path, request))
            }
            (UiEvent::Activate, BACK) => self.begin_navigation(NavigationCommand::Back, request),
            (UiEvent::Activate, FORWARD) => {
                self.begin_navigation(NavigationCommand::Forward, request)
            }
            (UiEvent::Activate, UP) => self.begin_navigation(NavigationCommand::Up, request),
            (UiEvent::Activate, HOME | PLACE_HOME) => {
                self.begin_navigation(NavigationCommand::Home, request)
            }
            (UiEvent::Activate, RELOAD) => self.refresh_current_location(request),
            (UiEvent::Activate, PLACE_DOCUMENTS) => self.begin_navigation(
                NavigationCommand::Enter(String::from("/root/Documents")),
                request,
            ),
            (UiEvent::Activate, PLACE_DESKTOP) => self.begin_navigation(
                NavigationCommand::Enter(String::from("/root/Desktop")),
                request,
            ),
            (UiEvent::Activate, files_view::PLACE_DOWNLOADS) => self.begin_navigation(
                NavigationCommand::Enter(String::from("/root/Downloads")),
                request,
            ),
            (UiEvent::Activate, PLACE_PICTURES) => self.begin_navigation(
                NavigationCommand::Enter(String::from("/root/Pictures")),
                request,
            ),
            (UiEvent::Activate, files_view::PLACE_VIDEOS) => self.begin_navigation(
                NavigationCommand::Enter(String::from("/root/Videos")),
                request,
            ),
            (UiEvent::Activate, PLACE_TRASH) => self.begin_navigation(
                NavigationCommand::Enter(String::from("/root/.local/share/Trash/files")),
                request,
            ),
            (UiEvent::Activate, PLACE_MOUNTS) if self.local_files_available => self
                .begin_navigation(
                    NavigationCommand::Enter(String::from(self.local_mount_root())),
                    request,
                ),
            (UiEvent::Activate, files_view::PLACE_ADD) => {
                files_preferences::add_current(self, request)
            }
            (UiEvent::Activate, files_view::PLACE_REMOVE) => {
                files_preferences::remove_current(self, request)
            }
            (UiEvent::Activate, target)
                if files_view::bookmark_remove_index(target)
                    .is_some_and(|index| index < self.preferences.bookmarks().len()) =>
            {
                let index = files_view::bookmark_remove_index(target)
                    .ok_or(FilesActorError::InvalidSettings)?;
                files_preferences::remove_bookmark(self, index, request)
            }
            (UiEvent::Activate, target)
                if files_view::bookmark_index(target)
                    .is_some_and(|index| index < self.preferences.bookmarks().len()) =>
            {
                let path = files_view::bookmark_index(target)
                    .and_then(|index| self.preferences.bookmarks().get(index))
                    .map(|bookmark| bookmark.path.as_str().to_owned())
                    .ok_or(FilesActorError::InvalidSettings)?;
                self.begin_navigation(NavigationCommand::Enter(path), request)
            }
            (UiEvent::Activate, target)
                if self.folder_tree && files_view::folder_tree_entry_index(target).is_some() =>
            {
                let index = files_view::folder_tree_entry_index(target)
                    .ok_or(FilesActorError::InvalidSettings)?;
                let path = self
                    .folder_tree_entries()
                    .into_iter()
                    .nth(index)
                    .map(|entry| entry.path.as_str().to_owned());
                if let Some(path) = path {
                    self.begin_navigation(NavigationCommand::Enter(path), request)
                } else {
                    Ok(Vec::new())
                }
            }
            (UiEvent::Activate, target)
                if self.folder_tree && files_view::folder_tree_ancestor_index(target).is_some() =>
            {
                let index = files_view::folder_tree_ancestor_index(target)
                    .ok_or(FilesActorError::InvalidSettings)?;
                let path = files_view::folder_tree_ancestor_paths(self.location().as_str(), self.local_mount_root())
                    .get(index)
                    .map(|(path, _)| path.clone());
                if let Some(path) = path {
                    self.begin_navigation(NavigationCommand::Enter(path), request)
                } else {
                    Ok(Vec::new())
                }
            }
            (UiEvent::ValueChanged(path), BREADCRUMB) => self.update_location_text(path),
            (
                UiEvent::Key {
                    pressed: true,
                    key_code: 13 | 36,
                    ..
                },
                BREADCRUMB,
            ) => self.activate_location(request),
            (
                UiEvent::Key {
                    pressed: true,
                    key_code: 38 | 111,
                    ..
                },
                BREADCRUMB,
            ) => self.move_completion(-1),
            (
                UiEvent::Key {
                    pressed: true,
                    key_code: 40 | 116,
                    ..
                },
                BREADCRUMB,
            ) => self.move_completion(1),
            (UiEvent::ValueChanged(value), SEARCH_FIELD) => self.update_quick_filter(value),
            (UiEvent::Activate, SEARCH) => {
                self.search_expanded = true;
                self.emit_full().map(|message| vec![message])
            }
            (
                UiEvent::Key {
                    pressed: true,
                    key_code: 13 | 36,
                    ..
                },
                SEARCH_FIELD,
            )
            | (UiEvent::Activate, SEARCH_FIELD) => self.start_search(request),
            (
                UiEvent::Key {
                    pressed: true,
                    key_code: 27 | 9,
                    ..
                },
                SEARCH_FIELD,
            ) => {
                self.search_expanded = false;
                self.update_quick_filter("")
            }
            (UiEvent::ValueChanged(value), _)
                if matches!(
                    raw_target,
                    files_view::DIRECTORY_SCROLL | files_view::SECONDARY_DIRECTORY_SCROLL
                ) =>
            {
                self.scroll_active_pane_to(value)
            }
            (
                UiEvent::Key {
                    pressed: true,
                    key_code: 53,
                    modifiers,
                    ..
                },
                _,
            ) if modifiers & 4 != 0 && self.quick_filter_input_allowed(raw_target) => {
                files_clipboard::write(self, request, true)
            }
            (
                UiEvent::Key {
                    pressed: true,
                    key_code: 54,
                    modifiers,
                    ..
                },
                _,
            ) if modifiers & 4 != 0 && self.quick_filter_input_allowed(raw_target) => {
                files_clipboard::write(self, request, false)
            }
            (
                UiEvent::Key {
                    pressed: true,
                    key_code: 55,
                    modifiers,
                    ..
                },
                _,
            ) if modifiers & 4 != 0 && self.quick_filter_input_allowed(raw_target) => {
                files_clipboard::paste(self, request)
            }
            (
                UiEvent::Key {
                    pressed: true,
                    key_code: 38,
                    modifiers,
                    ..
                },
                _,
            ) if modifiers & 4 != 0
                && files_view::pane_for_widget(raw_target).is_some()
                && self.quick_filter_input_allowed(raw_target) =>
            {
                self.select_all_entries(request)
            }
            (
                UiEvent::Key {
                    pressed: true,
                    key_code: 46 | 119,
                    modifiers,
                    ..
                },
                _,
            ) if self.quick_filter_input_allowed(raw_target) && modifiers & 1 != 0 => {
                self.show_delete_permanently()
            }
            (
                UiEvent::Key {
                    pressed: true,
                    key_code: 46 | 119,
                    ..
                },
                _,
            ) if self.quick_filter_input_allowed(raw_target) => {
                let sources = self.selected_entries();
                match FileControls::trash(request.get(), &self.home, sources) {
                    Ok(plan) => self.begin_mutation("Move to Trash", &plan, request),
                    Err(notice) => self.emit_notice(&notice),
                }
            }
            (UiEvent::Activate, VIEW_ICONS | GRID_VIEW) => self.change_view(ViewMode::Icons),
            (UiEvent::Activate, VIEW_COMPACT) => self.change_view(ViewMode::Compact),
            (UiEvent::Activate, VIEW_THUMBNAILS) => self.change_view(ViewMode::Thumbnails),
            (UiEvent::Activate, VIEW_DETAILS | LIST_VIEW) => self.change_view(ViewMode::Details),
            (UiEvent::Activate, target)
                if files_view::detail_disclosure_for_widget(target).is_some() =>
            {
                let (slot, index) = files_view::detail_disclosure_for_widget(target)
                    .ok_or(FilesActorError::InvalidSettings)?;
                self.focus_pane(slot)?;
                let entry = self
                    .visible_entries_at(slot)
                    .get(index)
                    .cloned()
                    .ok_or(FilesActorError::InvalidSettings)?;
                self.toggle_detail_directory(entry, request)
            }
            (UiEvent::Activate, files_view::ZOOM_OUT) => self.adjust_view_zoom(-100),
            (UiEvent::Activate, files_view::ZOOM_IN) => self.adjust_view_zoom(100),
            (UiEvent::ValueChanged(value), files_view::ZOOM_SLIDER) => {
                let zoom = value
                    .split_once('/')
                    .and_then(|(position, _)| position.parse::<u16>().ok())
                    .map(|position| position.saturating_add(MIN_VIEW_ZOOM_PERMILLE))
                    .unwrap_or(self.view_zoom_permille);
                self.set_view_zoom(zoom)
            }
            (
                UiEvent::Key {
                    pressed: true,
                    key_code,
                    modifiers,
                    text,
                },
                _,
            ) if modifiers & (4 | 8) != 0
                && (matches!(*key_code, 21 | 43 | 61 | 86 | 171)
                    || matches!(text.as_deref(), Some("+" | "="))) =>
            {
                self.adjust_view_zoom(100)
            }
            (
                UiEvent::Key {
                    pressed: true,
                    key_code,
                    modifiers,
                    text,
                },
                _,
            ) if modifiers & (4 | 8) != 0
                && (matches!(*key_code, 20 | 45 | 82 | 173)
                    || matches!(text.as_deref(), Some("-"))) =>
            {
                self.adjust_view_zoom(-100)
            }
            (
                UiEvent::Key {
                    pressed: true,
                    key_code: 9 | 27,
                    ..
                },
                _,
            ) if self.quick_filter_input_allowed(raw_target)
                && !self.runtime()?.controls.search_text.is_empty() =>
            {
                self.update_quick_filter("")
            }
            (
                UiEvent::Key {
                    pressed: true,
                    key_code: 22,
                    ..
                },
                _,
            ) if self.quick_filter_input_allowed(raw_target)
                && !self.runtime()?.controls.search_text.is_empty() =>
            {
                self.backspace_quick_filter()
            }
            (
                UiEvent::Key {
                    pressed: true,
                    key_code: 22,
                    ..
                },
                _,
            ) if files_view::pane_for_widget(raw_target).is_some() => {
                self.begin_navigation(NavigationCommand::Up, request)
            }
            (
                UiEvent::Key {
                    pressed: true,
                    key_code,
                    modifiers,
                    ..
                },
                _,
            ) if matches!(*key_code, 37..=40 | 111 | 113 | 114 | 116)
                && files_view::pane_for_widget(raw_target).is_some() =>
            {
                self.move_keyboard_selection(raw_target, *key_code, *modifiers, request)
            }
            (
                UiEvent::Key {
                    pressed: true,
                    key_code: 65,
                    ..
                },
                _,
            ) if files_view::pane_for_widget(raw_target).is_some() => self.toggle_preview(request),
            (
                UiEvent::Key {
                    pressed: true,
                    key_code: 36,
                    ..
                },
                _,
            ) if files_view::pane_for_widget(raw_target).is_some() => self.open_selected(request),
            (
                UiEvent::Key {
                    pressed: true,
                    modifiers,
                    text: Some(text),
                    ..
                },
                _,
            ) if self.quick_filter_input_allowed(raw_target)
                && modifiers & (4 | 8 | 64 | 128) == 0
                && !text.is_empty()
                && text.chars().all(|character| !character.is_control()) =>
            {
                self.append_quick_filter(text)
            }
            (UiEvent::Activate, files_menu::PREFERENCES) => {
                self.dialog = None;
                let mut options = self.options.clone();
                options.zoom = self.view_zoom_permille;
                options.preview = self.runtime()?.controls.preview_visible;
                self.options_dialog = Some(options);
                self.emit_full().map(|message| vec![message])
            }
            (UiEvent::Activate, files_menu::SORT_ASCENDING | files_menu::SORT_DESCENDING) => {
                let mut sort = self.runtime()?.sort;
                sort.direction = if target == files_menu::SORT_ASCENDING { SortDirection::Ascending } else { SortDirection::Descending };
                self.apply_sort(sort)
            }
            (UiEvent::Activate, SORT_NAME) => self.change_sort(SortKey::Name),
            (UiEvent::Activate, SORT_TYPE) => self.change_sort(SortKey::Type),
            (UiEvent::Activate, SORT_SIZE) => self.change_sort(SortKey::Size),
            (UiEvent::Activate, SORT_EXTENSION) => self.change_sort(SortKey::Extension),
            (UiEvent::Activate, SORT_MODIFIED) => self.change_sort(SortKey::Modified),
            (UiEvent::Activate, PREVIEW_TOGGLE) => self.toggle_preview(request),
            (UiEvent::Activate, files_preview_view::PREVIEW_PLAY) => self.toggle_embedded_video(),
            (UiEvent::ValueChanged(value), files_preview_view::PREVIEW_PROGRESS) => {
                self.seek_embedded_video(value, request)
            }
            (UiEvent::Activate, files_menu::OPEN) => self.open_selected(request),
            (UiEvent::Activate, files_menu::OPEN_WITH) => self.show_open_with(request),
            (UiEvent::Activate, files_menu::CONNECT_SERVER) => self.show_connect_server(),
            (UiEvent::Activate, files_menu::NEW_FOLDER) => self.show_create_folder(),
            (UiEvent::Activate, files_menu::CLOSE) => Ok(vec![Message::HostRequest(HostRequest {
                app: self.app,
                window: self.window,
                operation: HostOperation::RequestClose,
            })]),
            (UiEvent::Activate, files_open::OPEN_THEME_LOAD) => self.choose_theme(true, request),
            (UiEvent::Activate, files_open::OPEN_THEME_EDIT) => self.choose_theme(false, request),
            (UiEvent::Activate, files_open::OPEN_WITH_CANCEL) => self.close_open_with(),
            (UiEvent::Activate, files_open::OPEN_WITH_REMEMBER) => self.toggle_open_with_default(),
            (UiEvent::ValueChanged(value), files_open::OPEN_WITH_SEARCH) => {
                self.search_open_with(value)
            }
            (UiEvent::ValueChanged(value), files_open::OPEN_WITH_LIST) => {
                self.scroll_open_with(value)
            }
            (UiEvent::Activate, PLACES_TOGGLE) => {
                self.sidebar_visible = !self.sidebar_visible;
                self.emit_full().map(|message| vec![message])
            }
            (UiEvent::Activate, SIMPLIFIED_VIEW) => {
                files_preferences::toggle_simplified_view(self, request)
            }
            (UiEvent::Activate, FOLDER_TREE) => {
                files_preferences::toggle_folder_tree(self, request)
            }
            (UiEvent::Activate, SPLIT_TOGGLE) => self.toggle_dual_pane(request),
            (UiEvent::Activate, SPLIT_NARROW) => self.resize_split(-100),
            (UiEvent::Activate, SPLIT_WIDE) => self.resize_split(100),
            (
                UiEvent::Pointer {
                    kind:
                        rice_native_app_protocol::PointerEventKind::Down
                        | rice_native_app_protocol::PointerEventKind::Move,
                    x,
                    button: 1,
                    ..
                },
                CONTENT,
            ) => {
                self.sidebar_permille = u16::try_from((*x).clamp(100, 600)).unwrap_or(220);
                self.emit_full().map(|message| vec![message])
            }
            (
                UiEvent::Pointer {
                    kind:
                        rice_native_app_protocol::PointerEventKind::Down
                        | rice_native_app_protocol::PointerEventKind::Move,
                    x,
                    button: 1,
                    ..
                },
                MAIN_SPLIT,
            ) => {
                let position = u16::try_from((*x).clamp(0, 1_000)).unwrap_or_default();
                self.tab_mut()?.resize_split(position);
                self.emit_full().map(|message| vec![message])
            }
            (
                UiEvent::Pointer {
                    kind: rice_native_app_protocol::PointerEventKind::Down,
                    button: 6,
                    ..
                },
                _,
            ) if files_view::entry_for_widget(raw_target).is_some() => {
                let (_, index) =
                    files_view::entry_for_widget(raw_target).expect("guarded entry target");
                self.open_entry_immediately(u64::try_from(index).unwrap_or_default(), request)
            }
            (
                UiEvent::Pointer {
                    kind: rice_native_app_protocol::PointerEventKind::Down,
                    button: 1,
                    modifiers,
                    ..
                },
                _,
            ) if files_view::entry_for_widget(raw_target).is_some() => {
                self.begin_entry_drag(raw_target, *modifiers, request)
            }
            (
                UiEvent::Pointer {
                    kind: rice_native_app_protocol::PointerEventKind::Move,
                    ..
                },
                _,
            ) if self.pointer_selection_active()
                && files_view::entry_for_widget(raw_target).is_some() =>
            {
                self.update_pointer_selection(raw_target, request)
            }
            (
                UiEvent::Pointer {
                    kind: rice_native_app_protocol::PointerEventKind::Up,
                    button: 1,
                    modifiers,
                    ..
                },
                _,
            ) if files_view::entry_for_widget(raw_target).is_some() => {
                if self.pointer_selection_active() {
                    self.finish_pointer_selection(raw_target, request)
                } else {
                    self.finish_entry_drag(raw_target, *modifiers, request)
                }
            }
            (
                UiEvent::Pointer {
                    kind: rice_native_app_protocol::PointerEventKind::Down,
                    button: 1,
                    modifiers,
                    ..
                },
                _,
            ) if files_view::selection_background(raw_target).is_some() => {
                self.begin_background_selection(*modifiers, request)
            }
            (
                UiEvent::Pointer {
                    kind: rice_native_app_protocol::PointerEventKind::Up,
                    button: 1,
                    modifiers,
                    ..
                },
                _,
            ) if files_view::selection_background(raw_target).is_some() => {
                if self.pointer_selection_active() {
                    self.cancel_pointer_selection();
                    Ok(Vec::new())
                } else {
                    self.finish_entry_drag(raw_target, *modifiers, request)
                }
            }
            (UiEvent::ValueChanged(name), RENAME_FIELD) => {
                self.runtime_mut()?.controls.rename_text.clone_from(name);
                self.emit_full().map(|message| vec![message])
            }
            (UiEvent::ValueChanged(target), MOVE_FIELD) => {
                self.runtime_mut()?.controls.move_text.clone_from(target);
                self.emit_full().map(|message| vec![message])
            }
            (UiEvent::ValueChanged(value), files_dialogs::FIELD) => {
                self.update_operation_field(value)
            }
            (UiEvent::ValueChanged(value), target)
                if matches!(
                    target,
                    files_remote::CLOUD_FIELD_FIRST..=files_remote::CLOUD_FIELD_LAST
                        | files_remote::URL_FIELD
                        | files_remote::USER_FIELD
                        | files_remote::PASSWORD_FIELD
                        | files_remote::DOMAIN_FIELD
                        | files_remote::UID_FIELD
                        | files_remote::GID_FIELD | files_remote::BRIDGE_URL | files_remote::BRIDGE_TOKEN
                ) =>
            {
                self.update_remote_field(target, value)
            }
            (UiEvent::Activate, files_remote::CONFIRM) => self.confirm_remote(request),
            (UiEvent::Activate, files_remote::CANCEL) => self.cancel_remote(request),
            (UiEvent::ValueChanged(value), files_properties::MODE_FIELD) => {
                self.update_property_mode(value)
            }
            (UiEvent::ValueChanged(value), files_properties::NAME_FIELD) => {
                self.update_property_name(value)
            }
            (UiEvent::Activate, files_properties::COPY_LOCATION) => {
                self.copy_property_location(request)
            }
            (UiEvent::Activate, files_properties::EXECUTABLE) => self.toggle_property_executable(),
            (UiEvent::Activate, files_properties::READ_ONLY_ATTRIBUTE) => {
                self.toggle_property_read_only()
            }
            (UiEvent::Activate, files_properties::HIDDEN_ATTRIBUTE) => {
                self.toggle_property_hidden()
            }
            (UiEvent::Activate, files_properties::ACCESS_READ_WRITE) => {
                self.choose_property_access(0o600)
            }
            (UiEvent::Activate, files_properties::ACCESS_READ_ONLY) => {
                self.choose_property_access(0o400)
            }
            (UiEvent::Activate, files_properties::ACCESS_WRITE_ONLY) => {
                self.choose_property_access(0o200)
            }
            (UiEvent::Activate, files_properties::ACCESS_NONE) => self.choose_property_access(0),
            (UiEvent::ValueChanged(value), files_open::OPEN_WITH_COMMAND) => {
                self.update_open_with_command(value)
            }
            (UiEvent::Activate, files_open::OPEN_WITH_RUN_COMMAND) => self.run_open_with_command(),
            (UiEvent::Activate, files_dialogs::PROPERTIES_OK) => {
                self.confirm_property_changes(request)
            }
            (UiEvent::Activate, files_dialogs::CANCEL) => {
                if matches!(self.dialog, Some(files_dialogs::Dialog::Conflict { .. })) {
                    self.cancel_conflict()
                } else {
                    self.close_operation_dialog()
                }
            }
            (UiEvent::Activate, files_dialogs::CONFIRM) => self.confirm_operation(request),
            (UiEvent::Activate, files_properties::APPLY_MODE) => {
                self.apply_property_changes(request)
            }
            (UiEvent::Activate, files_dialogs::CONFLICT_SKIP) => self.skip_conflict(),
            (UiEvent::Activate, files_dialogs::CONFLICT_REPLACE) => self.replace_conflict(),
            (UiEvent::Activate, files_dialogs::CONFLICT_APPLY_ALL) => {
                self.toggle_conflict_apply_all()
            }
            (UiEvent::Activate, files_progress::CANCEL) => self.cancel_mutation(request),
            (UiEvent::Activate, RENAME) => self.show_rename(),
            (UiEvent::Activate, MOVE) => self.show_move(),
            (UiEvent::Activate, files_menu::COPY_TO) => self.show_copy(),
            (UiEvent::Activate, files_menu::COMPRESS_ZIP) => {
                let sources = self.selected_entries();
                match FileControls::archive(
                    request.get(),
                    &self.home,
                    &self.location(),
                    sources,
                    rice_files::ArchiveFormat::Zip,
                ) {
                    Ok(plan) => self.begin_mutation("Compressing to ZIP", &plan, request),
                    Err(notice) => self.emit_notice(&notice),
                }
            }
            (UiEvent::Activate, files_menu::COMPRESS_TAR_GZIP) => {
                let sources = self.selected_entries();
                match FileControls::archive(
                    request.get(),
                    &self.home,
                    &self.location(),
                    sources,
                    rice_files::ArchiveFormat::TarGzip,
                ) {
                    Ok(plan) => self.begin_mutation("Compressing to TAR.GZ", &plan, request),
                    Err(notice) => self.emit_notice(&notice),
                }
            }
            (UiEvent::Activate, files_menu::COMPRESS_GZIP) => {
                let sources = self.selected_entries();
                match FileControls::archive(
                    request.get(),
                    &self.home,
                    &self.location(),
                    sources,
                    rice_files::ArchiveFormat::Gzip,
                ) {
                    Ok(plan) => self.begin_mutation("Compressing to GZIP", &plan, request),
                    Err(notice) => self.emit_notice(&notice),
                }
            }
            (UiEvent::Activate, files_menu::EXTRACT_HERE) => {
                let sources = self.selected_entries();
                match FileControls::extract(request.get(), &self.home, &self.location(), sources) {
                    Ok(plan) => self.begin_mutation("Extracting archive", &plan, request),
                    Err(notice) => self.emit_notice(&notice),
                }
            }
            (UiEvent::Activate, files_menu::DOWNLOAD_TO_HOST) => {
                files_download::activate(self, request)
            }
            (UiEvent::Activate, files_menu::PUBLISH_MARKETPLACE) => {
                let selected = self.selected_entries();
                match selected.as_slice() {
                    [entry] if entry.kind == EntryKind::File && entry.path.as_str().to_ascii_lowercase().ends_with(".riceapp") => Ok(vec![Message::Launch {
                        app: self.app,
                        entrypoint: String::from("rice-marketplace-publish"),
                        arguments: vec![entry.path.as_str().to_owned()],
                    }]),
                    _ => self.emit_notice(rice_i18n::t("Select one .riceapp file to publish")),
                }
            }
            (UiEvent::Activate, files_menu::DELETE_PERMANENTLY) => self.show_delete_permanently(),
            (UiEvent::Activate, files_menu::PROPERTIES) => self.show_properties(request),
            (UiEvent::Activate, files_view::HELP_ABOUT) => self.show_about(),
            (UiEvent::Activate, files_menu::CUT) => files_clipboard::write(self, request, true),
            (UiEvent::Activate, files_menu::COPY) => files_clipboard::write(self, request, false),
            (UiEvent::Activate, files_menu::PASTE) => files_clipboard::paste(self, request),
            (UiEvent::Activate, DELETE) => {
                let sources = self.selected_entries();
                if sources
                    .iter()
                    .any(|entry| files_preferences::is_builtin(entry.path.as_str()))
                {
                    return self.emit_notice(rice_i18n::t("Built-in Places cannot be deleted"));
                }
                match FileControls::trash(request.get(), &self.home, sources) {
                    Ok(plan) => self.begin_mutation("Move to Trash", &plan, request),
                    Err(notice) => self.emit_notice(&notice),
                }
            }
            (UiEvent::Activate, NEW_TAB) => self.open_tab(request),
            (UiEvent::Activate, files_menu::OPEN_IN_NEW_TAB) => {
                self.open_selected_directory_tab(request)
            }
            (UiEvent::Activate, files_menu::OPEN_IN_NEW_WINDOW) => {
                self.open_selected_directory_window()
            }
            (UiEvent::Activate, files_menu::OPEN_IN_TERMINAL) => self.open_folder_in_terminal(),
            (UiEvent::Activate, NEW_WINDOW) => Ok(vec![Message::Launch {
                app: self.app,
                entrypoint: String::from(ENTRYPOINT),
                arguments: vec![self.location().as_str().to_owned()],
            }]),
            (
                UiEvent::Pointer {
                    kind: rice_native_app_protocol::PointerEventKind::Up,
                    button: 3,
                    ..
                },
                _,
            ) if files_view::entry_for_widget(raw_target).is_some() => {
                self.select_context_entry(raw_target, request)
            }
            (UiEvent::Activate, target)
                if (files_open::OPEN_WITH_APP_BASE
                    ..files_open::OPEN_WITH_APP_BASE
                        + rice_native_app_protocol::MAX_APPLICATIONS as u64)
                    .contains(&target) =>
            {
                self.choose_open_with(target - files_open::OPEN_WITH_APP_BASE, request)
            }
            (UiEvent::Activate, target)
                if (COMPLETION_BASE..COMPLETION_BASE + 64).contains(&target) =>
            {
                self.activate_completion(target - COMPLETION_BASE, request)
            }
            (UiEvent::Activate, target)
                if (TAB_CLOSE_BASE..TAB_CLOSE_BASE + 10_000).contains(&target) =>
            {
                self.close_tab(target - TAB_CLOSE_BASE)
            }
            (UiEvent::Activate, target) if (TAB_BASE..COMPLETION_BASE).contains(&target) => {
                self.activate_tab(target - TAB_BASE, request)
            }
            (UiEvent::Activate, _) if files_view::entry_for_widget(raw_target).is_some() => {
                // Pointer clicks on key-navigable entries remain Pointer
                // events at the native UI boundary. Semantic activation is
                // therefore keyboard Enter/Space and opens the item.
                let (_, index) =
                    files_view::entry_for_widget(raw_target).expect("guarded entry target");
                self.activate_entry(u64::try_from(index).unwrap_or_default(), request)
            }
            (
                UiEvent::Pointer {
                    kind: rice_native_app_protocol::PointerEventKind::Down,
                    ..
                },
                _,
            ) if focus_changed => self.emit_full().map(|message| vec![message]),
            _ => Ok(Vec::new()),
        }
    }
}
