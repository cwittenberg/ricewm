//! Bounded native application actors for `RiceWM`.
//!
//! This first slice ships the `RiceUI` Control Gallery. Applications own models
//! and emit retained-tree patches; all widget primitives and rendering remain
//! in `rice-ui` and `rice-display`.

#![deny(unsafe_op_in_unsafe_fn)]

mod abi;
mod actor;
mod calc;
mod desktop;
mod error;
mod time_format;

mod files;
mod file_tree_ui;
pub use abi::*;
pub use error::RuntimeError;

use actor::Actor;
use rice_native_app_protocol::{
    AppId, HostCapabilities, MAX_MESSAGE_BYTES, Message, RequestId, UiEventEnvelope, VfsResponse,
};
use std::collections::{BTreeMap, VecDeque};

pub const MAX_ACTORS: usize = 64;
pub const MAX_OUTBOUND_MESSAGES: usize = 256;
pub const MAX_OUTBOUND_BYTES: usize = 4 * MAX_MESSAGE_BYTES;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct OutboundQueue {
    frames: VecDeque<Vec<u8>>,
    bytes: usize,
}

impl OutboundQueue {
    fn enqueue(&mut self, messages: &[Message]) -> Result<(), RuntimeError> {
        let frames = messages
            .iter()
            .map(Message::encode)
            .collect::<Result<Vec<_>, _>>()?;
        let additional_bytes = frames.iter().map(Vec::len).sum::<usize>();
        if self.frames.len().saturating_add(frames.len()) > MAX_OUTBOUND_MESSAGES
            || self.bytes.saturating_add(additional_bytes) > MAX_OUTBOUND_BYTES
        {
            return Err(RuntimeError::QueueFull);
        }
        self.bytes += additional_bytes;
        self.frames.extend(frames);
        Ok(())
    }

    fn pending(&self) -> usize {
        self.frames.front().map_or(0, Vec::len)
    }

    fn drain(&mut self, destination: &mut [u8]) -> Result<usize, RuntimeError> {
        let Some(frame) = self.frames.front() else {
            return Ok(0);
        };
        if destination.len() < frame.len() {
            return Err(RuntimeError::OutputTooSmall {
                required: frame.len(),
                available: destination.len(),
            });
        }
        let written = frame.len();
        destination[..written].copy_from_slice(frame);
        self.frames.pop_front();
        self.bytes -= written;
        Ok(written)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Runtime {
    actors: BTreeMap<AppId, Actor>,
    locale: u8,
    outbound: OutboundQueue,
    pending_media: BTreeMap<RequestId, AppId>,
    pending_data: BTreeMap<RequestId, AppId>,
    pending_clipboard: BTreeMap<RequestId, AppId>,
    pending_vfs: BTreeMap<RequestId, AppId>,
    pending_settings: BTreeMap<RequestId, AppId>,
    next_request_id: u64,
    initialized_capabilities: Option<HostCapabilities>,
    host_ready: bool,
}

impl Default for Runtime {
    fn default() -> Self {
        Self {
            actors: BTreeMap::new(),
            locale: 0,
            outbound: OutboundQueue::default(),
            pending_media: BTreeMap::new(),
            pending_data: BTreeMap::new(),
            pending_clipboard: BTreeMap::new(),
            pending_vfs: BTreeMap::new(),
            pending_settings: BTreeMap::new(),
            next_request_id: 1,
            initialized_capabilities: None,
            host_ready: false,
        }
    }
}

impl Runtime {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_capabilities(capabilities: HostCapabilities) -> Self {
        Self {
            initialized_capabilities: Some(capabilities),
            ..Self::default()
        }
    }

    /// Decode and apply one complete RAPP frame.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed or directionally invalid protocol input,
    /// lifecycle violations, invalid UI state, or a breached runtime bound.
    ///
    /// Actors that own external engine sessions must not be cloned as a
    /// rollback mechanism: the clone aliases the same guest-engine handles,
    /// doubles large page/resource state, and can exhaust Wasm memory during
    /// ordinary input. Message handlers validate before committing their
    /// retained state and provider request maps supply stale-frame safety.
    pub fn ingest(&mut self, bytes: &[u8]) -> Result<usize, RuntimeError> {
        let message = Message::decode(bytes)?;
        if let Message::Launch { arguments, .. } = &message {
            let locale = arguments.iter().find_map(|arg| arg.strip_prefix("--ricewm-locale=")).unwrap_or("en");
            self.locale = rice_i18n::locale_code(locale);
        }
        if let Message::HostEvent(event) = &message {
            if let rice_native_app_protocol::HostEventKind::RiceBus { topic, payload } = &event.event {
                if topic == "org.ricewm.locale/changed" {
                    self.locale = rice_i18n::locale_code(payload);
                }
            }
        }
        rice_i18n::with_locale(self.locale, || self.handle(message))?;
        Ok(bytes.len())
    }

    #[must_use]
    pub fn pending(&self) -> usize {
        self.outbound.pending()
    }

    /// Copy the next complete outbound RAPP frame into `destination`.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError::OutputTooSmall`] without consuming the frame
    /// when `destination` cannot hold it.
    pub fn drain(&mut self, destination: &mut [u8]) -> Result<usize, RuntimeError> {
        self.outbound.drain(destination)
    }

    #[must_use]
    pub fn actor_count(&self) -> usize {
        self.actors.len()
    }

    pub(crate) fn document_state(&self, app: AppId) -> Option<serde_json::Value> {
        self.actors.get(&app)?.document_state()
    }

    /// Command dispatcher for decoded host messages.
    fn handle(&mut self, message: Message) -> Result<(), RuntimeError> {
        match message {
            Message::HostHello { capabilities, .. } => {
                if self
                    .initialized_capabilities
                    .is_some_and(|initialized| initialized != capabilities)
                {
                    return Err(RuntimeError::CapabilityMismatch);
                }
                self.host_ready = true;
                Ok(())
            }
            Message::Launch {
                app,
                entrypoint,
                arguments,
            } => self.launch(app, &entrypoint, &arguments),
            Message::Close { app, reason: _ } => {
                self.actors
                    .remove(&app)
                    .ok_or(RuntimeError::UnknownActor(app))?;
                self.pending_media.retain(|_, owner| *owner != app);
                self.pending_data.retain(|_, owner| *owner != app);
                self.pending_clipboard.retain(|_, owner| *owner != app);
                self.pending_vfs.retain(|_, owner| *owner != app);
                self.pending_settings.retain(|_, owner| *owner != app);
                Ok(())
            }
            Message::CloseRequest(request) => self.route_close_request(&request),
            Message::UiEvent(event) => self.route_event(&event),
            Message::MediaResponse(response) => self.route_media_response(&response),
            Message::DataProviderResponse(response) => self.route_data_response(&response),
            Message::ClipboardResponse(response) => self.route_clipboard_response(&response),
            Message::VfsResponse(response) => self.route_vfs_response(&response),
            Message::SettingsResponse(response) => self.route_settings_response(&response),
            Message::HostEvent(event) => self.route_host_event(&event),
            Message::Open { app, paths } => self.route_open(app, &paths),
            Message::Drag(drag) => self.route_drag(&drag),
            Message::DragSource(event) => self.route_drag_source(&event),
            Message::MediaRequest(_)
            | Message::DataProviderRequest(_)
            | Message::VfsRequest(_)
            | Message::SettingsRequest(_)
            | Message::UiPatch(_)
            | Message::WebRenderPatch(_)
            | Message::ClipboardRequest(_)
            | Message::HostRequest(_)
            | Message::CloseResponse(_) => Err(RuntimeError::UnexpectedMessage),
        }
    }

    fn route_close_request(
        &mut self,
        request: &rice_native_app_protocol::CloseRequest,
    ) -> Result<(), RuntimeError> {
        let next_request = self.allocate_request()?;
        let actor = self
            .actors
            .get_mut(&request.app)
            .ok_or(RuntimeError::UnknownActor(request.app))?;
        let messages = actor.request_close(request, next_request)?;
        self.register_requests(request.app, &messages)?;
        self.outbound.enqueue(&messages)
    }

    fn launch(
        &mut self,
        app: AppId,
        entrypoint: &str,
        arguments: &[String],
    ) -> Result<(), RuntimeError> {
        if !self.host_ready {
            return Err(RuntimeError::HostNotReady);
        }
        if self.actors.contains_key(&app) {
            return Err(RuntimeError::DuplicateActor(app));
        }
        if self.actors.len() >= MAX_ACTORS {
            return Err(RuntimeError::ActorLimit);
        }
        let request = self.allocate_request()?;
        let arguments = arguments.iter().filter(|arg| !arg.starts_with("--ricewm-locale=")).cloned().collect::<Vec<_>>();
        let (actor, initial) = Actor::launch(
            app,
            entrypoint,
            &arguments,
            request,
        )?;
        self.register_requests(app, &initial)?;
        self.outbound.enqueue(&initial)?;
        self.actors.insert(app, actor);
        Ok(())
    }

    fn route_event(&mut self, event: &UiEventEnvelope) -> Result<(), RuntimeError> {
        let request = self.allocate_request()?;
        let actor = self
            .actors
            .get_mut(&event.app)
            .ok_or(RuntimeError::UnknownActor(event.app))?;
        let messages = actor.handle_event(event, request)?;
        let messages = self.capability_messages(messages);
        self.register_requests(event.app, &messages)?;
        self.outbound.enqueue(&messages)?;
        Ok(())
    }

    fn route_drag(
        &mut self,
        drag: &rice_native_app_protocol::DragEnvelope,
    ) -> Result<(), RuntimeError> {
        let request = self.allocate_request()?;
        let actor = self
            .actors
            .get_mut(&drag.app)
            .ok_or(RuntimeError::UnknownActor(drag.app))?;
        let messages = actor.handle_drag(drag, request)?;
        self.register_requests(drag.app, &messages)?;
        self.outbound.enqueue(&messages)
    }

    fn route_drag_source(
        &mut self,
        event: &rice_native_app_protocol::DragSourceEvent,
    ) -> Result<(), RuntimeError> {
        if !matches!(
            event.state,
            rice_native_app_protocol::DragSourceState::Finished(_)
        ) {
            return Err(RuntimeError::UnexpectedMessage);
        }
        let actor = self
            .actors
            .get_mut(&event.app)
            .ok_or(RuntimeError::UnknownActor(event.app))?;
        let messages = actor.handle_drag_source(event)?;
        self.register_requests(event.app, &messages)?;
        self.outbound.enqueue(&messages)
    }

    fn route_vfs_response(&mut self, response: &VfsResponse) -> Result<(), RuntimeError> {
        let next_request = self.allocate_request()?;
        let terminal = !matches!(
            response.result,
            rice_native_app_protocol::VfsResult::Progress(_)
        );
        let app = *self
            .pending_vfs
            .get(&response.request)
            .ok_or(RuntimeError::UnknownVfsRequest(response.request))?;
        let actor = self
            .actors
            .get_mut(&app)
            .ok_or(RuntimeError::UnknownActor(app))?;
        let messages = actor.handle_vfs_response(response, next_request)?;
        // Progress is a response frame, but it does not finish the provider
        // request. Keep its owner registered so the later Done, Error, or
        // Cancelled frame can reach the actor that owns the operation.
        if terminal {
            self.pending_vfs.remove(&response.request);
        }
        self.register_requests(app, &messages)?;
        self.outbound.enqueue(&messages)
    }

    fn route_media_response(
        &mut self,
        response: &rice_native_app_protocol::MediaResponse,
    ) -> Result<(), RuntimeError> {
        let next_request = self.allocate_request()?;
        let app = *self
            .pending_media
            .get(&response.request)
            .ok_or(RuntimeError::UnknownProviderRequest(response.request))?;
        let actor = self
            .actors
            .get_mut(&app)
            .ok_or(RuntimeError::UnknownActor(app))?;
        let messages = actor.handle_media_response(response, next_request)?;
        self.pending_media.remove(&response.request);
        self.register_requests(app, &messages)?;
        self.outbound.enqueue(&messages)
    }

    fn route_data_response(
        &mut self,
        response: &rice_native_app_protocol::DataProviderResponse,
    ) -> Result<(), RuntimeError> {
        let app = *self
            .pending_data
            .get(&response.request)
            .ok_or(RuntimeError::UnknownDataProviderRequest(response.request))?;
        let slots = self
            .actors
            .get(&app)
            .ok_or(RuntimeError::UnknownActor(app))?
            .data_request_slots();
        let mut requests = Vec::with_capacity(slots);
        for _ in 0..slots {
            requests.push(self.allocate_request()?);
        }
        let actor = self
            .actors
            .get_mut(&app)
            .ok_or(RuntimeError::UnknownActor(app))?;
        let messages = actor.handle_data_response(response, &requests)?;
        self.pending_data.remove(&response.request);
        self.register_requests(app, &messages)?;
        self.outbound.enqueue(&messages)
    }

    fn route_clipboard_response(
        &mut self,
        response: &rice_native_app_protocol::ClipboardResponse,
    ) -> Result<(), RuntimeError> {
        let app = *self
            .pending_clipboard
            .get(&response.request)
            .ok_or(RuntimeError::UnknownProviderRequest(response.request))?;
        let actor = self
            .actors
            .get_mut(&app)
            .ok_or(RuntimeError::UnknownActor(app))?;
        let messages = actor.handle_clipboard_response(response)?;
        self.pending_clipboard.remove(&response.request);
        self.register_requests(app, &messages)?;
        self.outbound.enqueue(&messages)
    }

    fn route_settings_response(
        &mut self,
        response: &rice_native_app_protocol::SettingsResponse,
    ) -> Result<(), RuntimeError> {
        let next_request = self.allocate_request()?;
        let app = *self
            .pending_settings
            .get(&response.request)
            .ok_or(RuntimeError::UnknownVfsRequest(response.request))?;
        let actor = self
            .actors
            .get_mut(&app)
            .ok_or(RuntimeError::UnknownActor(app))?;
        let messages = actor.handle_settings_response(response, next_request)?;
        self.pending_settings.remove(&response.request);
        self.register_requests(app, &messages)?;
        self.outbound.enqueue(&messages)
    }

    fn route_open(&mut self, app: AppId, paths: &[String]) -> Result<(), RuntimeError> {
        let request = self.allocate_request()?;
        let actor = self
            .actors
            .get_mut(&app)
            .ok_or(RuntimeError::UnknownActor(app))?;
        let messages = actor.handle_open(paths, request)?;
        self.register_requests(app, &messages)?;
        self.outbound.enqueue(&messages)
    }

    fn route_host_event(
        &mut self,
        event: &rice_native_app_protocol::HostEvent,
    ) -> Result<(), RuntimeError> {
        let request = self.allocate_request()?;
        let actor = self
            .actors
            .get_mut(&event.app)
            .ok_or(RuntimeError::UnknownActor(event.app))?;
        let messages = actor.handle_host_event(event, request)?;
        self.register_requests(event.app, &messages)?;
        self.outbound.enqueue(&messages)
    }

    fn allocate_request(&mut self) -> Result<RequestId, RuntimeError> {
        let request =
            RequestId::new(self.next_request_id).ok_or(RuntimeError::RequestIdExhausted)?;
        self.next_request_id = self
            .next_request_id
            .checked_add(1)
            .ok_or(RuntimeError::RequestIdExhausted)?;
        Ok(request)
    }

    fn capability_messages(&self, mut messages: Vec<Message>) -> Vec<Message> {
        let drag_available = self
            .initialized_capabilities
            .is_some_and(|capabilities| capabilities.contains(HostCapabilities::DRAG_AND_DROP));
        if !drag_available {
            messages.retain(|message| !matches!(message, Message::DragSource(_)));
        }
        messages
    }

    fn register_requests(&mut self, app: AppId, messages: &[Message]) -> Result<(), RuntimeError> {
        for message in messages {
            if let Message::MediaRequest(request) = message
                && (request.app != app || self.pending_media.insert(request.request, app).is_some())
            {
                return Err(RuntimeError::DuplicateProviderRequest(request.request));
            }
            if let Message::DataProviderRequest(request) = message
                && (request.app != app || self.pending_data.insert(request.request, app).is_some())
            {
                return Err(RuntimeError::DuplicateDataProviderRequest(request.request));
            }
            if let Message::VfsRequest(request) = message
                && (request.app != app || self.pending_vfs.insert(request.request, app).is_some())
            {
                return Err(RuntimeError::DuplicateVfsRequest(request.request));
            }
            if let Message::ClipboardRequest(request) = message {
                let request = match request {
                    rice_native_app_protocol::ClipboardRequest::Read { request, .. }
                    | rice_native_app_protocol::ClipboardRequest::Write { request, .. }
                    | rice_native_app_protocol::ClipboardRequest::WriteFormats { request, .. } => *request,
                };
                if self.pending_clipboard.insert(request, app).is_some() {
                    return Err(RuntimeError::DuplicateProviderRequest(request));
                }
            }
            if let Message::SettingsRequest(request) = message
                && (request.app != app
                    || self.pending_settings.insert(request.request, app).is_some())
            {
                return Err(RuntimeError::DuplicateVfsRequest(request.request));
            }
        }
        Ok(())
    }
}


