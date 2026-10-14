//! Registry for the native applications included in the standalone edition.
use crate::{calc::RiceCalc, desktop::RiceDesktop, error::*, files::RiceFiles};
use rice_native_app_protocol::*;
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Actor {
    RiceCalc(RiceCalc),
    RiceFiles(Box<RiceFiles>),
    RiceDesktop(Box<RiceDesktop>),
}
impl Actor {
    pub(crate) fn launch(
        app: AppId,
        entrypoint: &str,
        args: &[String],
        request: RequestId,
    ) -> Result<(Self, Vec<Message>), RuntimeError> {
        match entrypoint {
            crate::files::ENTRYPOINT => RiceFiles::launch_at(app, args, request)
                .map(|(a, m)| (Self::RiceFiles(Box::new(a)), m))
                .map_err(map_files_error),
            crate::desktop::ENTRYPOINT => RiceDesktop::launch(app, args, request)
                .map(|(a, m)| (Self::RiceDesktop(Box::new(a)), m))
                .map_err(map_desktop_error),
            crate::calc::ENTRYPOINT => RiceCalc::launch(app, args)
                .map(|(a, m)| (Self::RiceCalc(a), vec![m]))
                .map_err(map_calc_error),
            _ => Err(RuntimeError::UnknownEntrypoint),
        }
    }
    pub(crate) fn document_state(&self) -> Option<serde_json::Value> {
        match self {
            Self::RiceFiles(a) => a.document_state(),
            _ => None,
        }
    }
    pub(crate) fn data_request_slots(&self) -> usize {
        1
    }
    pub(crate) fn handle_event(
        &mut self,
        e: &UiEventEnvelope,
        r: RequestId,
    ) -> Result<Vec<Message>, RuntimeError> {
        match self {
            Self::RiceFiles(a) => {
                let mut messages = a.handle_event(e, r).map_err(map_files_error)?;
                a.sync_rice_bus_subscription(&mut messages)
                    .map_err(map_files_error)?;
                Ok(messages)
            }
            Self::RiceDesktop(a) => a.handle_event(e, r).map_err(map_desktop_error),
            Self::RiceCalc(a) => a.handle_event(e, r).map_err(map_calc_error),
        }
    }
    pub(crate) fn request_close(
        &mut self,
        r: &CloseRequest,
        _: RequestId,
    ) -> Result<Vec<Message>, RuntimeError> {
        Ok(vec![Message::CloseResponse(CloseResponse {
            request: r.request,
            app: r.app,
            decision: CloseDecision::Approved,
        })])
    }
    pub(crate) fn handle_drag(
        &mut self,
        e: &DragEnvelope,
        r: RequestId,
    ) -> Result<Vec<Message>, RuntimeError> {
        match self {
            Self::RiceFiles(a) => a.handle_drag(e, r).map_err(map_files_error),
            Self::RiceDesktop(a) => a.handle_external_drag(e, r).map_err(map_desktop_error),
            _ => Err(RuntimeError::UnexpectedMessage),
        }
    }
    pub(crate) fn handle_drag_source(
        &mut self,
        e: &DragSourceEvent,
    ) -> Result<Vec<Message>, RuntimeError> {
        match self {
            Self::RiceFiles(a) => a.finish_source_drag(e).map_err(map_files_error),
            Self::RiceDesktop(a) => a.finish_source_drag(e).map_err(map_desktop_error),
            _ => Err(RuntimeError::UnexpectedMessage),
        }
    }
    pub(crate) fn handle_settings_response(
        &mut self,
        e: &SettingsResponse,
        _: RequestId,
    ) -> Result<Vec<Message>, RuntimeError> {
        match self {
            Self::RiceFiles(a) => a.handle_settings_response(e).map_err(map_files_error),
            Self::RiceDesktop(a) => a.handle_settings_response(e).map_err(map_desktop_error),
            _ => Err(RuntimeError::UnexpectedMessage),
        }
    }
    pub(crate) fn handle_media_response(
        &mut self,
        e: &MediaResponse,
        r: RequestId,
    ) -> Result<Vec<Message>, RuntimeError> {
        match self {
            Self::RiceFiles(a) => a
                .handle_media_response_with_request(e, r)
                .map_err(map_files_error),
            Self::RiceDesktop(a) => a.handle_media_response(e, r).map_err(map_desktop_error),
            _ => Err(RuntimeError::UnexpectedMessage),
        }
    }
    pub(crate) fn handle_data_response(
        &mut self,
        e: &DataProviderResponse,
        rs: &[RequestId],
    ) -> Result<Vec<Message>, RuntimeError> {
        match self {
            Self::RiceFiles(a) => a.handle_data_response(e, rs[0]).map_err(map_files_error),
            Self::RiceDesktop(a) => a
                .handle_application_response(e, rs[0])
                .map_err(map_desktop_error),
            Self::RiceCalc(a) => a.handle_data_response(e).map_err(map_calc_error),
        }
    }
    pub(crate) fn handle_vfs_response(
        &mut self,
        e: &VfsResponse,
        _request: RequestId,
    ) -> Result<Vec<Message>, RuntimeError> {
        match self {
            Self::RiceFiles(a) => {
                let mut messages = a
                    .handle_vfs_response_with_request(e, _request)
                    .map_err(map_files_error)?;
                a.sync_rice_bus_subscription(&mut messages)
                    .map_err(map_files_error)?;
                Ok(messages)
            }
            Self::RiceDesktop(a) => a.handle_vfs_response(e).map_err(map_desktop_error),
            _ => Err(RuntimeError::UnexpectedMessage),
        }
    }
    pub(crate) fn handle_clipboard_response(
        &mut self,
        e: &ClipboardResponse,
    ) -> Result<Vec<Message>, RuntimeError> {
        match self {
            Self::RiceFiles(a) => a.handle_clipboard_response(e).map_err(map_files_error),
            Self::RiceDesktop(a) => a.handle_clipboard_response(e).map_err(map_desktop_error),
            _ => Err(RuntimeError::UnexpectedMessage),
        }
    }
    pub(crate) fn handle_open(
        &mut self,
        _: &[String],
        _: RequestId,
    ) -> Result<Vec<Message>, RuntimeError> {
        Err(RuntimeError::OpenUnsupported)
    }
    pub(crate) fn handle_host_event(
        &mut self,
        e: &HostEvent,
        r: RequestId,
    ) -> Result<Vec<Message>, RuntimeError> {
        if let HostEventKind::RiceBus { topic, .. } = &e.event {
            if topic == "org.ricewm.locale/changed" {
                return match self {
                    Self::RiceFiles(a) => a.emit_full().map(|m| vec![m]).map_err(map_files_error),
                    Self::RiceDesktop(a) => a.refresh_language().map_err(map_desktop_error),
                    Self::RiceCalc(a) => a.emit().map(|m| vec![m]).map_err(map_calc_error),
                };
            }
        }
        match self {
            Self::RiceFiles(a) => a.handle_host_event(e, r).map_err(map_files_error),
            Self::RiceDesktop(a) => a.handle_host_event(e, r).map_err(map_desktop_error),
            Self::RiceCalc(a) => a.handle_host_event(e).map_err(map_calc_error),
        }
    }
}
