//! Validated opaque identifiers shared by mailbox models and provider adapters.

use core::fmt;

const MAX_ID_BYTES: usize = 512;
const MAX_TOKEN_BYTES: usize = 4 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdError {
    Empty,
    TooLong { length: usize, maximum: usize },
    ControlCharacter,
    ZeroOperation,
}

impl fmt::Display for IdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("identifier is empty"),
            Self::TooLong { length, maximum } => {
                write!(
                    formatter,
                    "identifier length {length} exceeds {maximum} bytes"
                )
            }
            Self::ControlCharacter => {
                formatter.write_str("identifier contains a control character")
            }
            Self::ZeroOperation => formatter.write_str("operation identifiers start at one"),
        }
    }
}

impl std::error::Error for IdError {}

fn validated(value: impl Into<String>, maximum: usize) -> Result<String, IdError> {
    let value = value.into();
    if value.is_empty() {
        return Err(IdError::Empty);
    }
    if value.len() > maximum {
        return Err(IdError::TooLong {
            length: value.len(),
            maximum,
        });
    }
    if value.chars().any(char::is_control) {
        return Err(IdError::ControlCharacter);
    }
    Ok(value)
}

macro_rules! opaque_id {
    ($name:ident, $maximum:expr) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            /// Creates an opaque identifier without assigning provider-specific
            /// meaning to its contents.
            ///
            /// # Errors
            ///
            /// Returns [`IdError`] when the value is empty, too long, or
            /// contains a control character.
            pub fn new(value: impl Into<String>) -> Result<Self, IdError> {
                validated(value, $maximum).map(Self)
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }

            #[must_use]
            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

opaque_id!(AccountId, MAX_ID_BYTES);
opaque_id!(ProviderId, MAX_ID_BYTES);
opaque_id!(LabelId, MAX_ID_BYTES);
opaque_id!(MessageId, MAX_ID_BYTES);
opaque_id!(ThreadId, MAX_ID_BYTES);
opaque_id!(DraftId, MAX_ID_BYTES);
opaque_id!(PageToken, MAX_TOKEN_BYTES);
opaque_id!(SyncCursor, MAX_TOKEN_BYTES);
opaque_id!(ProviderRevision, MAX_TOKEN_BYTES);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OperationId(u64);

impl OperationId {
    /// Creates a nonzero operation identifier.
    ///
    /// # Errors
    ///
    /// Returns [`IdError::ZeroOperation`] when `value` is zero.
    pub fn new(value: u64) -> Result<Self, IdError> {
        (value != 0)
            .then_some(Self(value))
            .ok_or(IdError::ZeroOperation)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for OperationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}
