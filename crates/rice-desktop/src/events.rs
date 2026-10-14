//! Bounded event coalescing for desktop refreshes.

use std::collections::VecDeque;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct EventPath(String);

impl EventPath {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MountId(String);

impl MountId {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MountRecord {
    pub id: MountId,
    pub label: String,
    pub root: EventPath,
    pub read_only: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MonitorEvent {
    Created(EventPath),
    Deleted(EventPath),
    Renamed { from: EventPath, to: EventPath },
    MetadataChanged(EventPath),
    MountArrived(MountRecord),
    MountRemoved(MountId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoalescedEvent {
    Created(EventPath),
    Deleted(EventPath),
    Renamed {
        from: EventPath,
        to: EventPath,
    },
    MetadataChanged(EventPath),
    MountArrived(MountRecord),
    MountRemoved(MountId),
    /// The adapter must re-enumerate once instead of applying incomplete data.
    RescanRequired {
        dropped_at_least: usize,
    },
}

/// A bounded Unit-of-Work for monitor events.
///
/// Coalescing is deliberately independent of wall-clock time. The host owns
/// debounce timing and drains this deterministic batch when its timer fires.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopEventCoalescer {
    capacity: usize,
    pending: VecDeque<CoalescedEvent>,
    dropped: usize,
}

impl DesktopEventCoalescer {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            pending: VecDeque::new(),
            dropped: 0,
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    #[must_use]
    pub const fn is_overflowed(&self) -> bool {
        self.dropped != 0
    }

    pub fn push(&mut self, event: MonitorEvent) {
        if self.dropped != 0 {
            self.dropped = self.dropped.saturating_add(1);
            return;
        }
        match event {
            MonitorEvent::Created(path) => self.push_created(path),
            MonitorEvent::Deleted(path) => self.push_deleted(path),
            MonitorEvent::Renamed { from, to } => self.push_renamed(from, to),
            MonitorEvent::MetadataChanged(path) => self.push_metadata(path),
            MonitorEvent::MountArrived(mount) => self.push_mount_arrived(mount),
            MonitorEvent::MountRemoved(id) => self.push_mount_removed(id),
        }
        if self.pending.len() > self.capacity {
            self.dropped = self.pending.len();
            self.pending.clear();
        }
    }

    pub fn drain(&mut self) -> Vec<CoalescedEvent> {
        if self.dropped != 0 {
            let dropped = std::mem::take(&mut self.dropped);
            self.pending.clear();
            return vec![CoalescedEvent::RescanRequired {
                dropped_at_least: dropped,
            }];
        }
        self.pending.drain(..).collect()
    }

    fn push_created(&mut self, path: EventPath) {
        if let Some(index) = self.path_event_index(&path) {
            match &self.pending[index] {
                CoalescedEvent::Deleted(_) => {
                    self.pending[index] = CoalescedEvent::MetadataChanged(path);
                }
                CoalescedEvent::Created(_) | CoalescedEvent::MetadataChanged(_) => {}
                _ => self.pending.push_back(CoalescedEvent::Created(path)),
            }
        } else {
            self.pending.push_back(CoalescedEvent::Created(path));
        }
    }

    fn push_deleted(&mut self, path: EventPath) {
        if let Some(index) = self.path_event_index(&path) {
            match self.pending[index].clone() {
                CoalescedEvent::Created(_) => {
                    self.pending.remove(index);
                }
                CoalescedEvent::Renamed { from, to } if to == path => {
                    self.pending[index] = CoalescedEvent::Deleted(from);
                }
                _ => self.pending[index] = CoalescedEvent::Deleted(path),
            }
        } else {
            self.pending.push_back(CoalescedEvent::Deleted(path));
        }
    }

    fn push_renamed(&mut self, from: EventPath, to: EventPath) {
        if from == to {
            self.push_metadata(to);
            return;
        }
        if let Some(index) = self.rename_target_index(&from) {
            let CoalescedEvent::Renamed { from: original, .. } = self.pending[index].clone() else {
                unreachable!();
            };
            self.pending[index] = CoalescedEvent::Renamed { from: original, to };
            return;
        }
        if let Some(index) = self.path_event_index(&from) {
            match self.pending[index].clone() {
                CoalescedEvent::Created(_) => {
                    self.pending[index] = CoalescedEvent::Created(to);
                }
                CoalescedEvent::Deleted(_) => {
                    self.pending.push_back(CoalescedEvent::Renamed { from, to });
                }
                _ => {
                    self.pending[index] = CoalescedEvent::Renamed { from, to };
                }
            }
        } else {
            self.pending.push_back(CoalescedEvent::Renamed { from, to });
        }
    }

    fn push_metadata(&mut self, path: EventPath) {
        if let Some(index) = self.path_event_index(&path) {
            if !matches!(
                self.pending[index],
                CoalescedEvent::Created(_)
                    | CoalescedEvent::Deleted(_)
                    | CoalescedEvent::Renamed { .. }
            ) {
                self.pending[index] = CoalescedEvent::MetadataChanged(path);
            }
        } else {
            self.pending
                .push_back(CoalescedEvent::MetadataChanged(path));
        }
    }

    fn push_mount_arrived(&mut self, mount: MountRecord) {
        if let Some(index) = self.mount_event_index(&mount.id) {
            self.pending[index] = CoalescedEvent::MountArrived(mount);
        } else {
            self.pending.push_back(CoalescedEvent::MountArrived(mount));
        }
    }

    fn push_mount_removed(&mut self, id: MountId) {
        if let Some(index) = self.mount_event_index(&id) {
            if matches!(self.pending[index], CoalescedEvent::MountArrived(_)) {
                self.pending.remove(index);
            } else {
                self.pending[index] = CoalescedEvent::MountRemoved(id);
            }
        } else {
            self.pending.push_back(CoalescedEvent::MountRemoved(id));
        }
    }

    fn path_event_index(&self, path: &EventPath) -> Option<usize> {
        self.pending.iter().position(|event| match event {
            CoalescedEvent::Created(candidate)
            | CoalescedEvent::Deleted(candidate)
            | CoalescedEvent::MetadataChanged(candidate) => candidate == path,
            CoalescedEvent::Renamed { to, .. } => to == path,
            _ => false,
        })
    }

    fn rename_target_index(&self, path: &EventPath) -> Option<usize> {
        self.pending
            .iter()
            .position(|event| matches!(event, CoalescedEvent::Renamed { to, .. } if to == path))
    }

    fn mount_event_index(&self, id: &MountId) -> Option<usize> {
        self.pending.iter().position(|event| match event {
            CoalescedEvent::MountArrived(record) => &record.id == id,
            CoalescedEvent::MountRemoved(candidate) => candidate == id,
            _ => false,
        })
    }
}

impl Default for DesktopEventCoalescer {
    fn default() -> Self {
        Self::new(256)
    }
}
