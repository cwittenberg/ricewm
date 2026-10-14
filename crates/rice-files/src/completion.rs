//! Bounded typed-directory completion over adapter-supplied names.

use crate::{GuestPath, NavigationError};

pub const MAX_COMPLETIONS: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionIntent {
    pub generation: u64,
    pub directory: GuestPath,
    pub prefix: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DirectoryCompletion {
    generation: u64,
    suggestions: Vec<GuestPath>,
}

impl DirectoryCompletion {
    /// Plans an adapter enumeration for the parent portion of typed input.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid guest path text.
    pub fn plan(
        &mut self,
        base: &GuestPath,
        input: &str,
    ) -> Result<CompletionIntent, NavigationError> {
        let (parent, prefix) = input
            .rsplit_once('/')
            .map_or(("", input), |(parent, prefix)| (parent, prefix));
        let directory = if input.starts_with('/') && parent.is_empty() {
            GuestPath::root()
        } else {
            GuestPath::resolve(base, parent)?
        };
        self.generation = self.generation.saturating_add(1);
        self.suggestions.clear();
        Ok(CompletionIntent {
            generation: self.generation,
            directory,
            prefix: prefix.to_owned(),
        })
    }

    /// Accepts names from the matching adapter response.
    ///
    /// Stale generations are ignored.
    ///
    /// # Errors
    ///
    /// Returns an error when an adapter name cannot form a valid guest path.
    pub fn resolve(
        &mut self,
        intent: &CompletionIntent,
        names: impl IntoIterator<Item = String>,
    ) -> Result<bool, NavigationError> {
        if intent.generation != self.generation {
            return Ok(false);
        }
        let prefix = intent.prefix.to_ascii_lowercase();
        let mut suggestions = names
            .into_iter()
            .filter(|name| {
                !name.is_empty()
                    && !name.contains('/')
                    && !matches!(name.as_str(), "." | "..")
                    && name.to_ascii_lowercase().starts_with(&prefix)
            })
            .map(|name| intent.directory.join(&name))
            .collect::<Result<Vec<_>, _>>()?;
        suggestions.sort();
        suggestions.dedup();
        suggestions.truncate(MAX_COMPLETIONS);
        self.suggestions = suggestions;
        Ok(true)
    }

    #[must_use]
    pub fn suggestions(&self) -> &[GuestPath] {
        &self.suggestions
    }
}
