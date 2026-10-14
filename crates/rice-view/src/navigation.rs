use crate::{ImageFormat, Source};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageCandidate {
    pub source: Source,
    pub format: ImageFormat,
}

impl ImageCandidate {
    #[must_use]
    pub fn from_source(source: Source) -> Option<Self> {
        let format = ImageFormat::from_extension(source.extension()?)?;
        Some(Self { source, format })
    }
}

/// Deterministically sorted view of image entries in one directory.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DirectoryImages {
    entries: Vec<ImageCandidate>,
    current: Option<usize>,
}

impl DirectoryImages {
    #[must_use]
    pub fn new(entries: impl IntoIterator<Item = ImageCandidate>) -> Self {
        let entries = normalize(entries);
        let current = (!entries.is_empty()).then_some(0);
        Self { entries, current }
    }

    #[must_use]
    pub fn entries(&self) -> &[ImageCandidate] {
        &self.entries
    }

    #[must_use]
    pub fn current(&self) -> Option<&ImageCandidate> {
        self.current.and_then(|index| self.entries.get(index))
    }

    #[must_use]
    pub fn current_index(&self) -> Option<usize> {
        self.current
    }

    pub fn select(&mut self, source: &Source) -> bool {
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.source == *source)
        {
            self.current = Some(index);
            true
        } else {
            false
        }
    }

    pub fn move_next(&mut self) -> Option<&ImageCandidate> {
        let current = self.current?;
        if current + 1 < self.entries.len() {
            self.current = Some(current + 1);
        }
        self.current()
    }

    pub fn move_previous(&mut self) -> Option<&ImageCandidate> {
        let current = self.current?;
        self.current = Some(current.saturating_sub(1));
        self.current()
    }

    /// Reconciles a directory watcher snapshot while retaining user context.
    ///
    /// If the current item disappeared, the lexicographic successor is chosen;
    /// if no successor exists, the final predecessor is chosen.
    pub fn reconcile(
        &mut self,
        entries: impl IntoIterator<Item = ImageCandidate>,
    ) -> ReconcileOutcome {
        let prior = self.current().map(|entry| entry.source.clone());
        self.entries = normalize(entries);
        if self.entries.is_empty() {
            self.current = None;
            return ReconcileOutcome::Empty;
        }
        let Some(prior) = prior else {
            self.current = Some(0);
            return ReconcileOutcome::SelectedFirst;
        };
        if let Some(index) = self.entries.iter().position(|entry| entry.source == prior) {
            self.current = Some(index);
            return ReconcileOutcome::Retained;
        }
        let prior_key = sort_key(&prior);
        let successor = self
            .entries
            .partition_point(|entry| sort_key(&entry.source) <= prior_key);
        let index = successor.min(self.entries.len() - 1);
        self.current = Some(index);
        ReconcileOutcome::AdvancedAfterRemoval
    }

    /// Removes the current entry after a successful trash operation.
    pub fn remove_current(&mut self) -> Option<Source> {
        let index = self.current?;
        let removed = self.entries.remove(index).source;
        self.current = if self.entries.is_empty() {
            None
        } else {
            Some(index.min(self.entries.len() - 1))
        };
        Some(removed)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReconcileOutcome {
    Empty,
    SelectedFirst,
    Retained,
    AdvancedAfterRemoval,
}

fn normalize(entries: impl IntoIterator<Item = ImageCandidate>) -> Vec<ImageCandidate> {
    let mut seen = BTreeSet::new();
    let mut entries: Vec<_> = entries
        .into_iter()
        .filter(|entry| seen.insert(entry.source.clone()))
        .collect();
    entries.sort_by(|left, right| {
        sort_key(&left.source)
            .cmp(&sort_key(&right.source))
            .then_with(|| left.source.cmp(&right.source))
    });
    entries
}

fn sort_key(source: &Source) -> String {
    source.as_str().to_lowercase()
}
