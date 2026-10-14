//! Typed host wake handling for event-driven desktop refreshes.

use rice_native_app_protocol::{HostEvent, HostEventKind, Message, RequestId};

use super::{
    ContextKind, DesktopActorError, DesktopCommand, DesktopIntent, RiceDesktop,
    TRASH_WATCH_EVENT_TOKEN, WATCH_EVENT_TOKEN, desktop_grid_config,
    desktop_vfs::{PendingDirectory, directory_request},
    replace_item_patches, save_positions_request, switch_context_patches,
};

impl RiceDesktop {
    pub(super) fn filesystem_watch_requests(&self) -> [Message; 2] {
        [
            Message::HostRequest(rice_native_app_protocol::HostRequest {
                app: self.app,
                window: self.window,
                operation: rice_native_app_protocol::HostOperation::SetTimer {
                    token: WATCH_EVENT_TOKEN,
                    interval_millis: 1_000,
                    enabled: true,
                },
            }),
            Message::HostRequest(rice_native_app_protocol::HostRequest {
                app: self.app,
                window: self.window,
                operation: rice_native_app_protocol::HostOperation::SetTimer {
                    token: TRASH_WATCH_EVENT_TOKEN,
                    interval_millis: 750,
                    enabled: true,
                },
            }),
        ]
    }

    pub(crate) fn handle_host_event(
        &mut self,
        event: &HostEvent,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        if event.app != self.app {
            return Err(DesktopActorError::UnexpectedHostEvent);
        }
        if let HostEventKind::RiceBus { topic, payload } = &event.event {
            if topic != super::THEME_CHANGED_TOPIC {
                return Ok(Vec::new());
            }
            let redmond98 = payload == "redmond-98";
            if redmond98 == self.redmond98 {
                return Ok(Vec::new());
            }
            self.redmond98 = redmond98;
            return self.resize_desktop_grid(request);
        }
        match event.event {
            HostEventKind::Timer {
                token: TRASH_WATCH_EVENT_TOKEN,
            } => self.begin_trash_status(request),
            HostEventKind::Timer {
                token: WATCH_EVENT_TOKEN,
            } => {
                if self.context_kind == ContextKind::Rename {
                    self.watch_refresh_pending = false;
                    return Ok(Vec::new());
                }
                self.watch_refresh_pending = true;
                if self.pending_directory.is_some() {
                    return Ok(Vec::new());
                }
                let messages = self.begin_refresh(request, None)?;
                if !messages.is_empty() {
                    self.watch_refresh_pending = false;
                }
                Ok(messages)
            }
            HostEventKind::WindowState {
                window,
                content_width,
                content_height,
                focused,
                ..
            } if window == self.window => {
                let mut messages = if focused {
                    Vec::new()
                } else {
                    self.cancel_focus_owned_interaction()?
                };
                self.content_width = content_width;
                self.content_height = content_height;
                messages.extend(self.resize_desktop_grid(request)?);
                Ok(messages)
            }
            _ => Err(DesktopActorError::UnexpectedHostEvent),
        }
    }

    fn resize_desktop_grid(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let config = desktop_grid_config(
            self.content_width,
            self.content_height,
            self.items.len(),
            self.redmond98,
        );
        if config == self.model.grid().config() {
            return Ok(Vec::new());
        }
        let update = self.model.execute(DesktopCommand::ResizeGrid(config))?;
        if let Some(positions) = update.intents.iter().find_map(|intent| match intent {
            DesktopIntent::PersistPositions(positions) => Some(positions.as_slice()),
            _ => None,
        }) {
            self.positions
                .replace_workspace(self.model.workspace(), &self.items, positions);
        }
        self.rendered_items = self.model.grid().ordered_items();
        let patches = replace_item_patches(
            &self.items,
            self.model.grid(),
            self.model.selection(),
            &self.thumbnail_cache,
        );
        let ui = self.emit(&patches)?;
        if self.items.is_empty() {
            return Ok(vec![ui]);
        }
        self.position_saves.insert(request);
        Ok(vec![
            ui,
            save_positions_request(self.app, request, &self.positions),
        ])
    }

    fn cancel_focus_owned_interaction(&mut self) -> Result<Vec<Message>, DesktopActorError> {
        let mut messages = self.finish_pointer_gesture()?;
        if self.context_kind == ContextKind::Hidden {
            return Ok(messages);
        }
        let patches = switch_context_patches(
            self.context_kind,
            ContextKind::Hidden,
            "",
            &self.context_action_ids,
            &[],
        );
        self.context_kind = ContextKind::Hidden;
        self.context_action_ids.clear();
        self.rename_text.clear();
        self.rename_source_path = None;
        if !patches.is_empty() {
            messages.push(self.emit(&patches)?);
        }
        Ok(messages)
    }

    pub(super) fn finish_observed_refresh(
        &mut self,
        request: RequestId,
        mut messages: Vec<Message>,
    ) -> Result<Vec<Message>, DesktopActorError> {
        if self.watch_refresh_pending
            && self.context_kind != ContextKind::Rename
            && self.pending_rename.is_none()
        {
            self.watch_refresh_pending = false;
            self.pending_directory = Some(PendingDirectory::new(request, None));
            messages.push(directory_request(self.app, request, 0));
        }
        Ok(messages)
    }
}
