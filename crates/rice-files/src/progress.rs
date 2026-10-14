//! Bounded, cancellable operation progress and interruption recovery.

use crate::{CommandId, GuestPath};

const MAX_RECOVERY_RECORDS: usize = 128;
const MAX_ERROR_BYTES: usize = 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgressStatus {
    Running,
    CancelRequested,
    Cancelled,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoveryAction {
    RemovePartial,
    RestoreBackup,
    RetrySource,
}

/// Recovery work left for an adapter after interruption.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryRecord {
    pub action: RecoveryAction,
    pub path: GuestPath,
    pub related_path: Option<GuestPath>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgressEvent {
    StartItem(GuestPath),
    AdvanceBytes(u64),
    CompleteItem,
    Recovery(RecoveryRecord),
    CancelDenied,
    Cancelled,
    Complete,
    Fail(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgressState {
    pub id: CommandId,
    pub total_items: u64,
    pub completed_items: u64,
    pub total_bytes: Option<u64>,
    pub completed_bytes: u64,
    pub current_path: Option<GuestPath>,
    pub status: ProgressStatus,
    recovery: Vec<RecoveryRecord>,
    error: Option<String>,
}

impl ProgressState {
    #[must_use]
    pub fn new(id: CommandId, total_items: u64, total_bytes: Option<u64>) -> Self {
        Self {
            id,
            total_items,
            completed_items: 0,
            total_bytes,
            completed_bytes: 0,
            current_path: None,
            status: ProgressStatus::Running,
            recovery: Vec::new(),
            error: None,
        }
    }

    /// # Errors
    ///
    /// Returns [`ProgressError::WrongState`] after leaving the running state.
    pub fn request_cancel(&mut self) -> Result<(), ProgressError> {
        self.ensure_running()?;
        self.status = ProgressStatus::CancelRequested;
        Ok(())
    }

    /// # Errors
    ///
    /// Returns an error for invalid transitions, counters, or bounded fields.
    pub fn apply(&mut self, event: ProgressEvent) -> Result<(), ProgressError> {
        if !matches!(event, ProgressEvent::Cancelled) {
            self.ensure_active()?;
        }
        match event {
            ProgressEvent::StartItem(path) => self.current_path = Some(path),
            ProgressEvent::AdvanceBytes(bytes) => {
                let next = self
                    .completed_bytes
                    .checked_add(bytes)
                    .ok_or(ProgressError::CounterOverflow)?;
                if self.total_bytes.is_some_and(|total| next > total) {
                    return Err(ProgressError::ExceedsTotal);
                }
                self.completed_bytes = next;
            }
            ProgressEvent::CompleteItem => {
                if self.completed_items == self.total_items {
                    return Err(ProgressError::ExceedsTotal);
                }
                self.completed_items += 1;
                self.current_path = None;
            }
            ProgressEvent::Recovery(record) => {
                if self.recovery.len() == MAX_RECOVERY_RECORDS {
                    return Err(ProgressError::RecoveryLimit);
                }
                self.recovery.push(record);
            }
            ProgressEvent::CancelDenied => {
                if self.status != ProgressStatus::CancelRequested {
                    return Err(ProgressError::CancelNotRequested);
                }
                self.status = ProgressStatus::Running;
            }
            ProgressEvent::Cancelled => {
                if self.status != ProgressStatus::CancelRequested {
                    return Err(ProgressError::CancelNotRequested);
                }
                self.status = ProgressStatus::Cancelled;
                self.current_path = None;
            }
            ProgressEvent::Complete => {
                if self.completed_items != self.total_items
                    || self
                        .total_bytes
                        .is_some_and(|total| self.completed_bytes != total)
                {
                    return Err(ProgressError::Incomplete);
                }
                self.status = ProgressStatus::Completed;
                self.current_path = None;
            }
            ProgressEvent::Fail(message) => {
                if message.is_empty() || message.len() > MAX_ERROR_BYTES {
                    return Err(ProgressError::InvalidError);
                }
                self.error = Some(message);
                self.status = ProgressStatus::Failed;
                self.current_path = None;
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn recovery(&self) -> &[RecoveryRecord] {
        &self.recovery
    }

    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    fn ensure_running(&self) -> Result<(), ProgressError> {
        if self.status == ProgressStatus::Running {
            Ok(())
        } else {
            Err(ProgressError::WrongState)
        }
    }

    fn ensure_active(&self) -> Result<(), ProgressError> {
        if matches!(
            self.status,
            ProgressStatus::Running | ProgressStatus::CancelRequested
        ) {
            Ok(())
        } else {
            Err(ProgressError::WrongState)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgressError {
    WrongState,
    CounterOverflow,
    ExceedsTotal,
    Incomplete,
    RecoveryLimit,
    CancelNotRequested,
    InvalidError,
}
