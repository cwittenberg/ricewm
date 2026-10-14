//! In-window drag/drop backed by validated file-operation plans.

use rice_files::{DropAction, EntryKind};
use rice_native_app_protocol::{
    DragAction, DragId, DragPayload, DragSourceEvent, DragSourceResult, DragSourceState, Message,
    RequestId,
};

use super::{FileControls, FilesActorError, RiceFiles, files_view};

impl RiceFiles {
    pub(super) fn begin_entry_drag(
        &mut self,
        target: u64,
        modifiers: u16,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let Some(entry) = self.entry_for_target(target) else {
            self.drag_sources = None;
            self.drag_origin = None;
            self.source_drag = None;
            return Ok(Vec::new());
        };
        self.cancel_pointer_selection();
        let selected = self.selected_entries();
        let (sources, mut messages) = if selected
            .iter()
            .any(|candidate| candidate.path == entry.path)
        {
            (selected, Vec::new())
        } else if modifiers & (1 | 4) != 0 {
            // Modified clicks are not allowed to destroy the old selection on
            // button-down. If this becomes a real drag, only the pressed item
            // is offered; if it remains a click, mouse-up applies Ctrl/Shift.
            (vec![entry.clone()], Vec::new())
        } else {
            self.pane_mut()?.set_selection([entry.path.clone()]);
            let runtime = self.runtime_mut()?;
            runtime.selection_anchor = Some(entry.path.clone());
            runtime.selection_caret = Some(entry.path.clone());
            (
                vec![entry.clone()],
                self.refresh_preview(request, Some(&entry))?,
            )
        };
        self.drag_origin = Some(entry.path.clone());
        self.drag_sources = Some(sources.clone());
        let Some(data) = super::files_clipboard::uri_list(&sources) else {
            self.source_drag = None;
            return Ok(messages);
        };
        let drag = DragId::new(request.get()).ok_or(FilesActorError::UnexpectedDrag)?;
        self.source_drag = Some(drag);
        let action = if sources
            .iter()
            .any(|entry| super::files_preferences::is_builtin(entry.path.as_str()))
        {
            DragAction::Copy
        } else {
            source_action(modifiers)
        };
        messages.push(Message::DragSource(DragSourceEvent {
            drag,
            app: self.app,
            window: self.window,
            state: DragSourceState::Offer {
                action,
                mime_types: vec![
                    String::from("text/uri-list"),
                    String::from("x-special/gnome-copied-files"),
                ],
                payload: DragPayload {
                    mime_type: String::from("text/uri-list"),
                    data,
                },
            },
        }));
        Ok(messages)
    }

    pub(super) fn finish_entry_drag(
        &mut self,
        target: u64,
        modifiers: u16,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let source_drag = self.source_drag.take();
        let mut messages = self.finish_entry_drag_inside(target, modifiers, request)?;
        if let Some(drag) = source_drag {
            messages.push(Message::DragSource(DragSourceEvent {
                drag,
                app: self.app,
                window: self.window,
                state: DragSourceState::Cancel,
            }));
        }
        Ok(messages)
    }

    fn finish_entry_drag_inside(
        &mut self,
        target: u64,
        modifiers: u16,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let Some(sources) = self.drag_sources.take() else {
            return self.pointer_select_entry(target, modifiers, request);
        };
        let origin = self.drag_origin.take();
        let Some(target_entry) = self.entry_for_target(target) else {
            return Ok(Vec::new());
        };
        if target_entry.kind != EntryKind::Directory
            || sources
                .iter()
                .any(|source| source.path == target_entry.path)
        {
            return if origin.as_ref() == Some(&target_entry.path) {
                self.pointer_select_entry(target, modifiers, request)
            } else {
                Ok(Vec::new())
            };
        }
        let (action, label) = if modifiers & 1 != 0 || modifiers & 4 != 0 {
            (DropAction::Copy, "Copy")
        } else if modifiers & 8 != 0 {
            (DropAction::Link, "Link")
        } else {
            (DropAction::Move, "Move")
        };
        if action == DropAction::Move
            && sources
                .iter()
                .any(|entry| super::files_preferences::is_builtin(entry.path.as_str()))
        {
            return self.emit_notice(rice_i18n::t("Built-in Places cannot be moved"));
        }
        match FileControls::drop(
            request.get(),
            &self.home,
            sources,
            target_entry.path,
            action,
        ) {
            Ok(plan) => self.begin_mutation(label, &plan, request),
            Err(notice) => self.emit_notice(&notice),
        }
    }

    pub(crate) fn finish_source_drag(
        &mut self,
        event: &DragSourceEvent,
    ) -> Result<Vec<Message>, FilesActorError> {
        if event.app != self.app || event.window != self.window {
            return Ok(Vec::new());
        }
        let Some(active) = self.source_drag else {
            return Ok(Vec::new());
        };
        if active != event.drag {
            return Err(FilesActorError::UnexpectedDrag);
        }
        let DragSourceState::Finished(result) = event.state else {
            return Err(FilesActorError::UnexpectedDrag);
        };
        self.source_drag = None;
        self.drag_sources = None;
        self.drag_origin = None;
        if matches!(result, DragSourceResult::Completed(DragAction::Move))
            && self.pending_mutation.is_none()
            && self.pending_external_drop.is_none()
            && self.runtime()?.pending_directory.is_none()
        {
            let refresh = RequestId::new(event.drag.get() | (1_u64 << 63))
                .ok_or(FilesActorError::UnexpectedDrag)?;
            return self.begin_navigation(rice_files::NavigationCommand::Reload, refresh);
        }
        let notice = match result {
            DragSourceResult::Completed(DragAction::Copy) => "Files copied to the destination",
            DragSourceResult::Completed(DragAction::Move) => "Files moved to the destination",
            DragSourceResult::Completed(DragAction::Link) => "Links created at the destination",
            DragSourceResult::Completed(DragAction::Ask) => {
                "The destination completed the requested drop"
            }
            DragSourceResult::Rejected => "This location does not accept file drops",
            DragSourceResult::Cancelled => "File drag cancelled",
            DragSourceResult::Failed => "The destination did not complete the drop",
        };
        self.emit_notice(rice_i18n::t(notice))
    }

    fn entry_for_target(&self, target: u64) -> Option<rice_files::EntryMetadata> {
        files_view::entry_for_widget(target)
            .map(|(_, index)| index)
            .and_then(|index| self.visible_entries().get(index).cloned())
    }
}

const fn source_action(modifiers: u16) -> DragAction {
    if modifiers & 1 != 0 {
        DragAction::Copy
    } else {
        match modifiers & 12 {
            12 => DragAction::Ask,
            4 => DragAction::Copy,
            8 => DragAction::Link,
            _ => DragAction::Move,
        }
    }
}
