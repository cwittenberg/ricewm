use crate::{calc::CalcActorError, desktop::DesktopActorError, files::FilesActorError};
use rice_native_app_protocol::{AppId, DecodeError, EncodeError, RequestId};
use rice_ui::{PatchConversionError, TreeError};
use std::{error::Error, fmt};
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeError {
    Decode(DecodeError),
    Encode(EncodeError),
    HostNotReady,
    ActorLimit,
    DuplicateActor(AppId),
    UnknownActor(AppId),
    UnknownVfsRequest(RequestId),
    UnknownDataProviderRequest(RequestId),
    DuplicateVfsRequest(RequestId),
    DuplicateDataProviderRequest(RequestId),
    UnknownProviderRequest(RequestId),
    DuplicateProviderRequest(RequestId),
    RequestIdExhausted,
    UnknownEntrypoint,
    OpenUnsupported,
    UnexpectedMessage,
    CapabilityMismatch,
    InvalidTree(TreeError),
    InvalidPatch(PatchConversionError),
    InvalidDesktopModel(rice_desktop::ModelError),
    InvalidCalculation(rice_calc::CalculatorError),
    InvalidFilesModel(rice_files::ModelError),
    InvalidNavigation(rice_files::NavigationError),
    InvalidVfsDirectory,
    SequenceExhausted,
    QueueFull,
    OutputTooSmall { required: usize, available: usize },
}
impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl Error for RuntimeError {}
impl From<DecodeError> for RuntimeError {
    fn from(e: DecodeError) -> Self {
        Self::Decode(e)
    }
}
impl From<EncodeError> for RuntimeError {
    fn from(e: EncodeError) -> Self {
        Self::Encode(e)
    }
}

pub(crate) fn map_calc_error(error: CalcActorError) -> RuntimeError {
    match error {
        CalcActorError::Tree(error) => RuntimeError::InvalidTree(error),
        CalcActorError::Conversion(error) => RuntimeError::InvalidPatch(error),
        CalcActorError::Calculation(error) => RuntimeError::InvalidCalculation(error),
        CalcActorError::InvalidProviderData | CalcActorError::UnexpectedProviderResponse => {
            RuntimeError::UnexpectedMessage
        }
        CalcActorError::SequenceExhausted => RuntimeError::SequenceExhausted,
    }
}

pub(crate) fn map_desktop_error(error: DesktopActorError) -> RuntimeError {
    match error {
        DesktopActorError::Model(error) => RuntimeError::InvalidDesktopModel(error),
        DesktopActorError::Tree(error) => RuntimeError::InvalidTree(error),
        DesktopActorError::Conversion(error) => RuntimeError::InvalidPatch(error),
        DesktopActorError::UnexpectedVfsResponse
        | DesktopActorError::InvalidClipboard
        | DesktopActorError::InvalidDirectoryEntry
        | DesktopActorError::EntryLimit => RuntimeError::InvalidVfsDirectory,
        DesktopActorError::UnexpectedClipboardResponse
        | DesktopActorError::UnexpectedDataProviderResponse
        | DesktopActorError::UnexpectedSettingsResponse
        | DesktopActorError::UnexpectedHostEvent => RuntimeError::UnexpectedMessage,
        DesktopActorError::SequenceExhausted => RuntimeError::SequenceExhausted,
    }
}

pub(crate) fn map_files_error(error: FilesActorError) -> RuntimeError {
    match error {
        FilesActorError::Tree(error) => RuntimeError::InvalidTree(error),
        FilesActorError::Conversion(error) => RuntimeError::InvalidPatch(error),
        FilesActorError::Navigation(error) => RuntimeError::InvalidNavigation(error),
        FilesActorError::Model(error) => RuntimeError::InvalidFilesModel(error),
        FilesActorError::UnexpectedVfsResponse
        | FilesActorError::UnexpectedMediaResponse
        | FilesActorError::UnexpectedClipboardResponse
        | FilesActorError::UnexpectedDrag
        | FilesActorError::UnexpectedDataProviderResponse
        | FilesActorError::UnexpectedSettingsResponse
        | FilesActorError::UnexpectedHostEvent
        | FilesActorError::InvalidSettings
        | FilesActorError::SettingsTooLarge
        | FilesActorError::InvalidDirectoryEntry
        | FilesActorError::EntryLimit => RuntimeError::InvalidVfsDirectory,
        FilesActorError::SequenceExhausted => RuntimeError::SequenceExhausted,
    }
}
