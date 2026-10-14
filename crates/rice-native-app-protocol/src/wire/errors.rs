use std::{error::Error, fmt};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EncodeError {
    LimitExceeded {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },
    InvalidValue(&'static str),
}

impl fmt::Display for EncodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LimitExceeded {
                field,
                actual,
                maximum,
            } => write!(formatter, "{field} has size {actual}, maximum is {maximum}"),
            Self::InvalidValue(field) => write!(formatter, "{field} has an invalid value"),
        }
    }
}

impl Error for EncodeError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodeError {
    Truncated,
    BadMagic,
    UnsupportedVersion(u16),
    UnknownMessageKind(u16),
    UnknownTag {
        field: &'static str,
        value: u8,
    },
    InvalidValue(&'static str),
    InvalidUtf8(&'static str),
    LimitExceeded {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },
    LengthMismatch,
    TrailingBytes,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => formatter.write_str("message is truncated"),
            Self::BadMagic => formatter.write_str("message magic is invalid"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "protocol version {version} is unsupported")
            }
            Self::UnknownMessageKind(kind) => {
                write!(formatter, "message kind {kind} is unknown")
            }
            Self::UnknownTag { field, value } => {
                write!(formatter, "{field} tag {value} is unknown")
            }
            Self::InvalidValue(field) => write!(formatter, "{field} has an invalid value"),
            Self::InvalidUtf8(field) => write!(formatter, "{field} is not valid UTF-8"),
            Self::LimitExceeded {
                field,
                actual,
                maximum,
            } => write!(formatter, "{field} has size {actual}, maximum is {maximum}"),
            Self::LengthMismatch => formatter.write_str("frame payload length does not match"),
            Self::TrailingBytes => formatter.write_str("message contains trailing bytes"),
        }
    }
}

impl Error for DecodeError {}
