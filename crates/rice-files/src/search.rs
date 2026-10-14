//! Bounded cancellable search Strategy and provider intent.

use crate::{EntryKind, EntryMetadata, GuestPath};
use core::fmt;

const MAX_QUERY_BYTES: usize = 1024;
const MAX_RESULTS: usize = 4096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchQuery {
    pub text: String,
    pub kind: Option<EntryKind>,
    pub minimum_size: Option<u64>,
    pub maximum_size: Option<u64>,
}

impl SearchQuery {
    /// # Errors
    ///
    /// Returns an error for empty, oversized, or contradictory criteria.
    pub fn validate(self) -> Result<Self, SearchError> {
        if self.text.trim().is_empty() || self.text.len() > MAX_QUERY_BYTES {
            return Err(SearchError::InvalidText);
        }
        if matches!((self.minimum_size, self.maximum_size), (Some(min), Some(max)) if min > max) {
            return Err(SearchError::InvalidSizeRange);
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchIntent {
    pub token: u64,
    pub root: GuestPath,
    pub query: SearchQuery,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchStatus {
    Running,
    CancelRequested,
    Complete,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchState {
    pub intent: SearchIntent,
    pub status: SearchStatus,
    results: Vec<EntryMetadata>,
}

impl SearchState {
    /// # Errors
    ///
    /// Returns an error for token zero.
    pub fn new(intent: SearchIntent) -> Result<Self, SearchError> {
        if intent.token == 0 {
            return Err(SearchError::ZeroToken);
        }
        Ok(Self {
            intent,
            status: SearchStatus::Running,
            results: Vec::new(),
        })
    }

    /// # Errors
    ///
    /// Returns an error outside the running state or at the result bound.
    pub fn append(&mut self, entry: EntryMetadata) -> Result<(), SearchError> {
        if self.status != SearchStatus::Running {
            return Err(SearchError::WrongState);
        }
        if self.results.len() == MAX_RESULTS {
            return Err(SearchError::ResultLimit);
        }
        self.results.push(entry);
        Ok(())
    }

    /// # Errors
    ///
    /// Returns an error unless the search is running.
    pub fn request_cancel(&mut self) -> Result<(), SearchError> {
        if self.status != SearchStatus::Running {
            return Err(SearchError::WrongState);
        }
        self.status = SearchStatus::CancelRequested;
        Ok(())
    }

    #[must_use]
    pub fn results(&self) -> &[EntryMetadata] {
        &self.results
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchError {
    InvalidText,
    InvalidSizeRange,
    ZeroToken,
    WrongState,
    ResultLimit,
}

impl fmt::Display for SearchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for SearchError {}
