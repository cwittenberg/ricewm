//! Bounded media-preview request and response state.

use super::{FilesActorError, RiceFiles};
use rice_files::{EntryMetadata, infer_media_type};
use rice_native_app_protocol::{
    HostEvent, HostEventKind, HostOperation, HostRequest, MAX_MEDIA_SOURCE_BYTES, MediaCommand,
    MediaKind, MediaMetadata, MediaRequest, MediaResponse, MediaResult, Message, RequestId,
    VfsOperation, VfsRequest, VfsResponse, VfsResult,
};

const PREVIEW_WIDTH: u16 = 720;
const PREVIEW_HEIGHT: u16 = 480;
const MAX_RETIRED_PREVIEW_REQUESTS: usize = 16;
const MAX_TEXT_PREVIEW_BYTES: u32 = 64 * 1024;
const MAX_THUMBNAILS: usize = 160;
const THUMBNAIL_WIDTH: u16 = 384;
const THUMBNAIL_HEIGHT: u16 = 288;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum PreviewState {
    Idle,
    Loading {
        request: RequestId,
        name: String,
        mime_type: String,
    },
    Ready {
        request: RequestId,
        name: String,
        path: String,
        metadata: MediaMetadata,
    },
    VideoLoading {
        name: String,
        path: String,
    },
    Video {
        session: u64,
        name: String,
        path: String,
        duration_millis: u64,
        position_millis: u64,
        playing: bool,
    },
    TextLoading {
        request: RequestId,
        name: String,
        path: String,
        mime_type: String,
    },
    TextReady {
        name: String,
        path: String,
        mime_type: String,
        text: String,
    },
    OwnedError {
        name: String,
        message: &'static str,
    },
    Error {
        name: String,
        message: String,
    },
}

impl RiceFiles {
    pub(super) fn toggle_preview(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let visible = !self.runtime()?.controls.preview_visible;
        self.runtime_mut()?.controls.preview_visible = visible;
        if !visible {
            let stop = self.stop_preview_message();
            self.retire_pending_preview();
            self.preview_state = PreviewState::Idle;
            let mut messages = vec![self.emit_full()?];
            if let Some(stop) = stop {
                messages.push(stop);
            }
            return Ok(messages);
        }
        let selected = self.selected_caret_entry();
        self.refresh_preview(request, selected.as_ref())
    }

    pub(super) fn refresh_preview(
        &mut self,
        request: RequestId,
        entry: Option<&EntryMetadata>,
    ) -> Result<Vec<Message>, FilesActorError> {
        let stop = self.stop_preview_message();
        self.retire_pending_preview();
        let Some(entry) = entry else {
            self.preview_state = PreviewState::Idle;
            let mut messages = vec![self.emit_full()?];
            if let Some(stop) = stop {
                messages.push(stop);
            }
            return Ok(messages);
        };
        let mime_type = infer_media_type(entry);
        if mime_type.starts_with("video/") {
            let path = entry.path.as_str().to_owned();
            let visible = self.runtime()?.controls.preview_visible;
            self.preview_state = if visible {
                PreviewState::VideoLoading {
                    name: entry.name.clone(),
                    path: path.clone(),
                }
            } else {
                PreviewState::Idle
            };
            let mut messages = vec![self.emit_full()?];
            if let Some(stop) = stop {
                messages.push(stop);
            }
            if visible {
                messages.push(Message::HostRequest(HostRequest {
                    app: self.app,
                    window: self.window,
                    operation: HostOperation::MediaOpen {
                        replace_session: 0,
                        path,
                    },
                }));
            }
            return Ok(messages);
        }
        if is_text_preview(entry, mime_type) {
            self.preview_state = PreviewState::TextLoading {
                request,
                name: entry.name.clone(),
                path: entry.path.as_str().to_owned(),
                mime_type: mime_type.to_owned(),
            };
            let patch = self.emit_full()?;
            let mut messages = vec![patch];
            if let Some(stop) = stop {
                messages.push(stop);
            }
            messages.push(Message::VfsRequest(VfsRequest {
                request,
                app: self.app,
                operation: VfsOperation::Read {
                    path: entry.path.as_str().to_owned(),
                    offset: 0,
                    length: MAX_TEXT_PREVIEW_BYTES,
                },
            }));
            return Ok(messages);
        }
        if !rice_files::supports_thumbnail(mime_type) {
            let notice = "No embedded preview is available for this file";
            self.preview_state = PreviewState::OwnedError {
                name: entry.name.clone(),
                message: notice,
            };
            let mut messages = vec![self.emit_full()?];
            if let Some(stop) = stop {
                messages.push(stop);
            }
            return Ok(messages);
        }
        self.preview_state = PreviewState::Loading {
            request,
            name: entry.name.clone(),
            mime_type: mime_type.to_owned(),
        };
        let patch = self.emit_full()?;
        let mut messages = vec![patch];
        if let Some(stop) = stop {
            messages.push(stop);
        }
        messages.push(Message::MediaRequest(MediaRequest {
            request,
            app: self.app,
            path: entry.path.as_str().to_owned(),
            mime_type: mime_type.to_owned(),
            maximum_source_bytes: MAX_MEDIA_SOURCE_BYTES,
            target_width: PREVIEW_WIDTH,
            target_height: PREVIEW_HEIGHT,
            frame_millis: 0,
        }));
        Ok(messages)
    }

    #[cfg(test)]
    pub(crate) fn handle_media_response(
        &mut self,
        response: &MediaResponse,
    ) -> Result<Vec<Message>, FilesActorError> {
        self.handle_media_response_with_request(response, response.request)
    }

    pub(crate) fn handle_media_response_with_request(
        &mut self,
        response: &MediaResponse,
        next_request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        if let Some(index) = self
            .retired_preview_requests
            .iter()
            .position(|request| *request == response.request)
        {
            self.retired_preview_requests.remove(index);
            return Ok(Vec::new());
        }
        if self
            .thumbnail_pending
            .as_ref()
            .is_some_and(|(request, _, _)| *request == response.request)
        {
            return self.handle_thumbnail_response(response, next_request);
        }
        let PreviewState::Loading {
            request,
            name,
            mime_type,
        } = &self.preview_state
        else {
            return Err(FilesActorError::UnexpectedMediaResponse);
        };
        if response.request != *request {
            return Err(FilesActorError::UnexpectedMediaResponse);
        }
        let name = name.clone();
        let mime_type = mime_type.clone();
        let path = self
            .selected_caret_entry()
            .as_ref()
            .map(|entry| entry.path.as_str().to_owned())
            .unwrap_or_default();
        let error_notice = match &response.result {
            MediaResult::Error(error) => Some(error.message.clone()),
            MediaResult::Ready(_) => None,
        };
        self.preview_state = match &response.result {
            MediaResult::Ready(metadata)
                if metadata.mime_type == mime_type && metadata.width > 0 && metadata.height > 0 =>
            {
                PreviewState::Ready {
                    request: response.request,
                    name,
                    path,
                    metadata: metadata.clone(),
                }
            }
            MediaResult::Ready(_) => {
                return Err(FilesActorError::UnexpectedMediaResponse);
            }
            MediaResult::Error(error) => PreviewState::Error {
                name,
                message: error.message.clone(),
            },
        };
        let mut messages = vec![self.emit_full()?];
        if let Some(notice) = error_notice {
            messages.push(self.system_notification("error", &notice));
        }
        Ok(messages)
    }

    pub(super) fn toggle_embedded_video(&mut self) -> Result<Vec<Message>, FilesActorError> {
        let PreviewState::Video {
            session, playing, ..
        } = &mut self.preview_state
        else {
            return Ok(Vec::new());
        };
        *playing = !*playing;
        let session = *session;
        Ok(vec![
            self.preview_control(session, MediaCommand::TogglePause, 0),
            self.update_preview_controls()?,
        ])
    }

    pub(super) fn seek_embedded_video(
        &mut self,
        value: &str,
        _request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let Some(target) = value
            .split_once('/')
            .and_then(|(position, _)| position.parse::<u64>().ok())
        else {
            return Ok(Vec::new());
        };
        let PreviewState::Video {
            session,
            duration_millis,
            ..
        } = self.preview_state
        else {
            return Ok(Vec::new());
        };
        let target = if duration_millis > 0 {
            target.min(duration_millis)
        } else {
            target
        };
        if let PreviewState::Video {
            position_millis, ..
        } = &mut self.preview_state
        {
            *position_millis = target;
        }
        Ok(vec![
            self.preview_control(
                session,
                MediaCommand::SeekMillis,
                target.min(i64::MAX as u64) as i64,
            ),
            self.update_preview_controls()?,
        ])
    }

    pub(crate) fn handle_preview_host_event(
        &mut self,
        event: &HostEvent,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        if event.app != self.app {
            return Ok(Vec::new());
        }
        if let HostEventKind::WindowState { fullscreen, .. } = &event.event {
            self.window_fullscreen = *fullscreen;
            if self.preview_fullscreen && !fullscreen {
                self.preview_fullscreen = false;
                return Ok(vec![self.emit_full()?]);
            }
        }
        match &event.event {
            HostEventKind::WindowState {
                window,
                focused: true,
                visible: true,
                ..
            } if *window == self.window => {
                return Ok(self.refresh_mount_catalog(request).into_iter().collect());
            }
            HostEventKind::MediaOpened {
                path,
                session,
                duration_millis,
                error,
            } => {
                let name = match &self.preview_state {
                    PreviewState::VideoLoading {
                        name,
                        path: expected,
                    } if path == expected => name.clone(),
                    _ => {
                        return Ok(if *session > 0 {
                            vec![self.preview_control(*session, MediaCommand::Stop, 0)]
                        } else {
                            Vec::new()
                        });
                    }
                };
                let position = self.preview_positions.get(path).copied().unwrap_or(0);
                // Completed movies (or files replaced with a shorter movie)
                // start again rather than reopening at EOF.
                let position = if *duration_millis > 0 && position >= *duration_millis {
                    0
                } else {
                    position
                };
                let opened = error.is_empty() && *session > 0;
                self.preview_state = if opened {
                    PreviewState::Video {
                        session: *session,
                        name,
                        path: path.clone(),
                        duration_millis: *duration_millis,
                        position_millis: position,
                        playing: true,
                    }
                } else if error.is_empty() {
                    PreviewState::OwnedError { name, message: "Movie could not be opened" }
                } else {
                    PreviewState::Error { name, message: error.clone() }
                };
                let mut messages = Vec::new();
                if opened && position > 0 {
                    messages.push(self.preview_control(
                        *session, MediaCommand::SeekMillis, position.min(i64::MAX as u64) as i64,
                    ));
                }
                messages.push(self.emit_full()?);
                return Ok(messages);
            }
            HostEventKind::RiceBus { topic, payload } if topic == "org.rice.media.state" => {
                let data: serde_json::Value = serde_json::from_str(payload).unwrap_or_default();
                let incoming = data["session"]
                    .as_str()
                    .and_then(|value| value.parse::<u64>().ok());
                if let PreviewState::Video {
                    session,
                    position_millis,
                    duration_millis,
                    playing,
                    ..
                } = &mut self.preview_state
                {
                    if incoming != Some(*session) {
                        return Ok(Vec::new());
                    }
                    let state = &data["state"];
                    if let Some(seconds) = state["currentTime"]
                        .as_f64()
                        .filter(|v| v.is_finite() && *v >= 0.0)
                    {
                        *position_millis = (seconds * 1000.0) as u64;
                    }
                    if let Some(seconds) = state["duration"]
                        .as_f64()
                        .filter(|v| v.is_finite() && *v > 0.0)
                    {
                        *duration_millis = (seconds * 1000.0) as u64;
                    }
                    if let Some(paused) = state["paused"].as_bool() {
                        *playing = !paused;
                    }
                    return Ok(vec![self.update_preview_controls()?]);
                }
            }
            HostEventKind::MediaEnded { session: ended } => {
                if let PreviewState::Video {
                    session, playing, ..
                } = &mut self.preview_state
                {
                    if session == ended {
                        *playing = false;
                        return Ok(vec![self.update_preview_controls()?]);
                    }
                }
            }
            _ => {}
        }
        Ok(Vec::new())
    }

    pub(super) fn seek_preview_key(
        &mut self,
        forward: bool,
        modifiers: u16,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let PreviewState::Video {
            duration_millis, ..
        } = self.preview_state
        else {
            return Ok(Vec::new());
        };
        let step = if modifiers & 1 != 0 {
            duration_millis / 100 * 15 + duration_millis % 100 * 15 / 100
        } else {
            5_000
        };
        let step = step.min(i64::MAX as u64) as i64;
        self.seek_preview_relative(if forward { step } else { -step }, request)
    }

    pub(super) fn seek_preview_relative(
        &mut self,
        delta: i64,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let PreviewState::Video {
            position_millis,
            duration_millis,
            ..
        } = self.preview_state
        else {
            return Ok(Vec::new());
        };
        let position = position_millis.saturating_add_signed(delta);
        self.seek_embedded_video(&format!("{position}/{duration_millis}"), request)
    }

    pub(super) fn toggle_preview_fullscreen(&mut self) -> Result<Vec<Message>, FilesActorError> {
        if !matches!(self.preview_state, PreviewState::Video { .. }) {
            return Ok(Vec::new());
        }
        if !self.preview_fullscreen {
            self.preview_restore_fullscreen = self.window_fullscreen;
        }
        self.preview_fullscreen = !self.preview_fullscreen;
        let fullscreen = self.preview_fullscreen || self.preview_restore_fullscreen;
        Ok(vec![
            self.emit_full()?,
            Message::HostRequest(HostRequest {
                app: self.app,
                window: self.window,
                operation: HostOperation::SetFullscreen(fullscreen),
            }),
        ])
    }

    pub(super) fn open_preview_in_player(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let PreviewState::Video { path, playing, .. } = &self.preview_state else {
            return Ok(Vec::new());
        };
        let Some(entry) = self
            .selected_entries()
            .into_iter()
            .find(|entry| entry.path.as_str() == path)
        else {
            return Ok(Vec::new());
        };
        let mime_type = infer_media_type(&entry).to_owned();
        let mut messages = if *playing {
            self.toggle_embedded_video()?
        } else {
            Vec::new()
        };
        if self.preview_fullscreen {
            messages.extend(self.toggle_preview_fullscreen()?);
        }
        messages.extend(self.launch_application(
            "native:rice-player".into(),
            entry,
            mime_type,
            false,
            request,
        )?);
        Ok(messages)
    }

    fn update_preview_controls(&mut self) -> Result<Message, FilesActorError> {
        let patches = super::files_preview_view::video_control_updates(&self.preview_state);
        self.tree.apply(&patches)?;
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or(FilesActorError::SequenceExhausted)?;
        self.patch_message(&patches)
    }

    fn preview_control(&self, session: u64, command: MediaCommand, value: i64) -> Message {
        Message::HostRequest(HostRequest {
            app: self.app,
            window: self.window,
            operation: HostOperation::MediaControl {
                session,
                command,
                value,
            },
        })
    }

    pub(super) fn dismiss_preview(&mut self) -> Result<Vec<Message>, FilesActorError> {
        let stop = self.stop_preview_message();
        self.retire_pending_preview();
        self.preview_state = PreviewState::Idle;
        let was_fullscreen = self.preview_fullscreen;
        self.preview_fullscreen = false;
        let mut messages = vec![self.emit_full()?];
        if was_fullscreen {
            messages.push(Message::HostRequest(HostRequest {
                app: self.app,
                window: self.window,
                operation: HostOperation::SetFullscreen(self.preview_restore_fullscreen),
            }));
        }
        if let Some(stop) = stop {
            messages.push(stop);
        }
        Ok(messages)
    }

    fn stop_preview_message(&mut self) -> Option<Message> {
        match &self.preview_state {
            PreviewState::Video { session, path, position_millis, .. } => {
                // Keep checkpoints by full path across selection, folder/tab
                // changes, and hiding the preview within this Files window.
                self.preview_positions.insert(path.clone(), *position_millis);
                Some(self.preview_control(*session, MediaCommand::Stop, 0))
            }
            _ => None,
        }
    }

    fn retire_pending_preview(&mut self) {
        let pending = match &self.preview_state {
            PreviewState::Loading { request, .. } => Some(*request),
            PreviewState::TextLoading { request, .. } => Some(*request),
            _ => None,
        };
        if let Some(request) = pending {
            self.retired_preview_requests.push(request);
            if self.retired_preview_requests.len() > MAX_RETIRED_PREVIEW_REQUESTS {
                self.retired_preview_requests.remove(0);
            }
        }
    }

    pub(super) fn owns_text_preview_request(&self, request: RequestId) -> bool {
        matches!(
            self.preview_state,
            PreviewState::TextLoading { request: pending, .. } if pending == request
        )
    }

    pub(super) fn handle_text_preview_response(
        &mut self,
        response: &VfsResponse,
    ) -> Result<Vec<Message>, FilesActorError> {
        let PreviewState::TextLoading {
            request,
            name,
            path,
            mime_type,
        } = &self.preview_state
        else {
            return Err(FilesActorError::UnexpectedVfsResponse);
        };
        if *request != response.request {
            return Err(FilesActorError::UnexpectedVfsResponse);
        }
        let error_notice = match &response.result {
            VfsResult::Error(error) => Some(error.message.clone()),
            _ => None,
        };
        self.preview_state = match &response.result {
            VfsResult::Data(bytes) => PreviewState::TextReady {
                name: name.clone(),
                path: path.clone(),
                mime_type: mime_type.clone(),
                text: String::from_utf8_lossy(bytes).into_owned(),
            },
            VfsResult::Error(error) => PreviewState::Error {
                name: name.clone(),
                message: error.message.clone(),
            },
            _ => return Err(FilesActorError::UnexpectedVfsResponse),
        };
        let mut messages = vec![self.emit_full()?];
        if let Some(notice) = error_notice {
            messages.push(self.system_notification("error", &notice));
        }
        Ok(messages)
    }

    pub(super) fn reset_thumbnail_batch(
        &mut self,
        entries: &[EntryMetadata],
        request: RequestId,
        prune_cache: bool,
    ) -> Option<Message> {
        if let Some((pending, _, _)) = self.thumbnail_pending.take() {
            self.retired_preview_requests.push(pending);
            if self.retired_preview_requests.len() > 16 {
                self.retired_preview_requests.remove(0);
            }
        }
        if !self.options.thumbnails {
            self.thumbnail_cache.clear();
            self.thumbnail_queue.clear();
            return None;
        }
        let candidates = entries
            .iter()
            .filter(|entry| entry.kind == rice_files::EntryKind::File)
            .filter(|entry| self.options.show_hidden || !entry.name.starts_with('.'))
            .filter_map(|entry| {
                let mime = infer_media_type(entry);
                rice_files::supports_thumbnail(mime)
                    .then(|| (entry.path.clone(), mime.to_owned()))
            })
            .take(MAX_THUMBNAILS)
            .collect::<Vec<_>>();
        let visible_paths = candidates
            .iter()
            .map(|(path, _)| path.clone())
            .collect::<std::collections::BTreeSet<_>>();
        if prune_cache {
            self.thumbnail_cache
                .retain(|path, _| visible_paths.contains(path));
        }
        let available = MAX_THUMBNAILS.saturating_sub(self.thumbnail_cache.len());
        self.thumbnail_queue.clear();
        self.thumbnail_queue.extend(
            candidates
                .into_iter()
                .filter(|(path, _)| !self.thumbnail_cache.contains_key(path))
                .take(available),
        );
        self.start_next_thumbnail(request)
    }

    fn handle_thumbnail_response(
        &mut self,
        response: &MediaResponse,
        next_request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let Some((request, path, mime_type)) = self.thumbnail_pending.take() else {
            return Err(FilesActorError::UnexpectedMediaResponse);
        };
        if request != response.request {
            self.thumbnail_pending = Some((request, path, mime_type));
            return Err(FilesActorError::UnexpectedMediaResponse);
        }
        let expected_kind = if mime_type.starts_with("video/") {
            MediaKind::Video
        } else {
            MediaKind::Image
        };
        if let MediaResult::Ready(metadata) = &response.result
            && metadata.mime_type == mime_type
            && metadata.kind == expected_kind
            && metadata.width > 0
            && metadata.height > 0
        {
            self.thumbnail_cache.insert(path, response.request);
        }
        let mut messages = vec![self.emit_full()?];
        if let Some(next) = self.start_next_thumbnail(next_request) {
            messages.push(next);
        }
        Ok(messages)
    }

    fn start_next_thumbnail(&mut self, request: RequestId) -> Option<Message> {
        let (path, mime_type) = self.thumbnail_queue.pop_front()?;
        let frame_millis = if mime_type.starts_with("video/") {
            1_000
        } else {
            0
        };
        self.thumbnail_pending = Some((request, path.clone(), mime_type.clone()));
        Some(Message::MediaRequest(MediaRequest {
            request,
            app: self.app,
            path: path.as_str().to_owned(),
            mime_type,
            maximum_source_bytes: self.options.maximum_bytes(),
            target_width: THUMBNAIL_WIDTH,
            target_height: THUMBNAIL_HEIGHT,
            frame_millis,
        }))
    }
}

fn is_text_preview(entry: &EntryMetadata, mime_type: &str) -> bool {
    if mime_type.starts_with("text/") || mime_type == "application/json" {
        return true;
    }
    let extension = entry
        .name
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase());
    extension.as_deref().is_some_and(|extension| {
        matches!(
            extension,
            "txt"
                | "csv"
                | "nfo"
                | "c"
                | "h"
                | "cc"
                | "cpp"
                | "rs"
                | "py"
                | "js"
                | "mjs"
                | "ts"
                | "tsx"
                | "jsx"
                | "java"
                | "go"
                | "sh"
                | "bash"
                | "zsh"
                | "toml"
                | "yaml"
                | "yml"
                | "xml"
                | "html"
                | "css"
                | "md"
                | "log"
                | "ini"
                | "conf"
        )
    })
}
