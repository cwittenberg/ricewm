//! Validated provider locations and connect intents without network execution.

use core::fmt;

const MAX_LOCATION_BYTES: usize = 4096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderLocation {
    scheme: String,
    authority: String,
    path: String,
}

impl ProviderLocation {
    /// # Errors
    ///
    /// Returns an error for malformed, unsupported-size, or credential-bearing
    /// provider locations.
    pub fn parse(input: &str) -> Result<Self, ProviderLocationError> {
        if input.len() > MAX_LOCATION_BYTES {
            return Err(ProviderLocationError::TooLong);
        }
        let (scheme, remainder) = input
            .split_once("://")
            .ok_or(ProviderLocationError::MissingScheme)?;
        if scheme.is_empty()
            || !scheme
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
        {
            return Err(ProviderLocationError::InvalidScheme);
        }
        let (authority, path) = remainder
            .split_once('/')
            .map_or((remainder, "/"), |(authority, path)| (authority, path));
        if authority.is_empty()
            || authority.contains('@')
            || authority.chars().any(char::is_whitespace)
        {
            return Err(ProviderLocationError::InvalidAuthority);
        }
        Ok(Self {
            scheme: scheme.to_ascii_lowercase(),
            authority: authority.to_owned(),
            path: format!("/{}", path.trim_start_matches('/')),
        })
    }

    #[must_use]
    pub fn scheme(&self) -> &str {
        &self.scheme
    }

    #[must_use]
    pub fn authority(&self) -> &str {
        &self.authority
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectIntent {
    pub request: u64,
    pub location: ProviderLocation,
}

impl ConnectIntent {
    /// # Errors
    ///
    /// Returns an error when the reserved request ID zero is supplied.
    pub fn new(request: u64, location: ProviderLocation) -> Result<Self, ProviderLocationError> {
        if request == 0 {
            Err(ProviderLocationError::ZeroRequest)
        } else {
            Ok(Self { request, location })
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderLocationError {
    TooLong,
    MissingScheme,
    InvalidScheme,
    InvalidAuthority,
    ZeroRequest,
}

impl fmt::Display for ProviderLocationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ProviderLocationError {}
