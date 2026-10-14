use std::{error::Error, fmt};

use rice_native_app_protocol::{AppId, WidgetId, WindowId};
use rice_ui::{PatchConversionError, TreeError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeUiError {
    UnknownWindow {
        app: AppId,
        window: WindowId,
    },
    UnknownWidget(WidgetId),
    StaleSequence {
        received: u64,
        current: u64,
    },
    OutOfOrderSequence {
        received: u64,
        expected: u64,
    },
    SequenceExhausted,
    InvalidPresentation(&'static str),
    Conversion(PatchConversionError),
    Tree(TreeError),
    ResourceLimit {
        resource: &'static str,
        maximum: usize,
    },
}

impl From<PatchConversionError> for NativeUiError {
    fn from(value: PatchConversionError) -> Self {
        Self::Conversion(value)
    }
}

impl From<TreeError> for NativeUiError {
    fn from(value: TreeError) -> Self {
        Self::Tree(value)
    }
}

impl fmt::Display for NativeUiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownWindow { app, window } => {
                write!(formatter, "native window {app}/{window} does not exist")
            }
            Self::UnknownWidget(widget) => {
                write!(formatter, "native widget {widget} does not exist")
            }
            Self::StaleSequence { received, current } => write!(
                formatter,
                "stale UI sequence {received}; current sequence is {current}"
            ),
            Self::OutOfOrderSequence { received, expected } => write!(
                formatter,
                "out-of-order UI sequence {received}; expected {expected}"
            ),
            Self::SequenceExhausted => formatter.write_str("UI sequence is exhausted"),
            Self::InvalidPresentation(field) => {
                write!(formatter, "invalid native presentation: {field}")
            }
            Self::Conversion(error) => error.fmt(formatter),
            Self::Tree(error) => error.fmt(formatter),
            Self::ResourceLimit { resource, maximum } => {
                write!(formatter, "{resource} limit of {maximum} exceeded")
            }
        }
    }
}

impl Error for NativeUiError {}
