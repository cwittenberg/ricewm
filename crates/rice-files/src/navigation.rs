//! Guest POSIX path normalization and navigation command plans.

use core::fmt;

/// A normalized absolute path in the emulated guest.
///
/// Construction is purely lexical: it never follows symlinks or reads either
/// the host or guest filesystem.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GuestPath(String);

impl GuestPath {
    pub const MAX_BYTES: usize = 4096;

    #[must_use]
    pub fn root() -> Self {
        Self(String::from("/"))
    }

    /// # Errors
    ///
    /// Returns an error for relative, oversized, or NUL-containing input.
    pub fn absolute(input: &str) -> Result<Self, NavigationError> {
        if !input.starts_with('/') {
            return Err(NavigationError::AbsolutePathRequired);
        }
        Self::resolve(&Self::root(), input)
    }

    /// Resolves an absolute or relative input against a normalized base.
    ///
    /// # Errors
    ///
    /// Returns an error when the input is oversized or contains a NUL byte.
    pub fn resolve(base: &Self, input: &str) -> Result<Self, NavigationError> {
        validate_path_text(input)?;
        let mut components = if input.starts_with('/') {
            Vec::new()
        } else {
            base.components().map(str::to_owned).collect()
        };
        for component in input.split('/') {
            match component {
                "" | "." => {}
                ".." => {
                    components.pop();
                }
                value => components.push(value.to_owned()),
            }
        }
        let normalized = if components.is_empty() {
            String::from("/")
        } else {
            format!("/{}", components.join("/"))
        };
        if normalized.len() > Self::MAX_BYTES {
            return Err(NavigationError::PathTooLong(normalized.len()));
        }
        Ok(Self(normalized))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn is_root(&self) -> bool {
        self.0 == "/"
    }

    #[must_use]
    pub fn components(&self) -> impl DoubleEndedIterator<Item = &str> {
        self.0.split('/').filter(|component| !component.is_empty())
    }

    #[must_use]
    pub fn file_name(&self) -> Option<&str> {
        self.components().next_back()
    }

    #[must_use]
    pub fn parent(&self) -> Self {
        if self.is_root() {
            return Self::root();
        }
        let boundary = self.0.rfind('/').unwrap_or_default();
        if boundary == 0 {
            Self::root()
        } else {
            Self(self.0[..boundary].to_owned())
        }
    }

    /// # Errors
    ///
    /// Returns an error for absolute, oversized, or NUL-containing children.
    pub fn join(&self, child: &str) -> Result<Self, NavigationError> {
        if child.starts_with('/') {
            return Err(NavigationError::RelativePathRequired);
        }
        Self::resolve(self, child)
    }

    #[must_use]
    pub fn is_within(&self, root: &Self) -> bool {
        self == root
            || (!root.is_root()
                && self.0.starts_with(root.as_str())
                && self.0.as_bytes().get(root.0.len()) == Some(&b'/'))
            || root.is_root()
    }
}

impl fmt::Display for GuestPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

fn validate_path_text(input: &str) -> Result<(), NavigationError> {
    if input.as_bytes().contains(&0) {
        return Err(NavigationError::NulByte);
    }
    if input.len() > GuestPath::MAX_BYTES {
        return Err(NavigationError::PathTooLong(input.len()));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationError {
    NulByte,
    PathTooLong(usize),
    AbsolutePathRequired,
    RelativePathRequired,
    NoHistory,
    StalePlan,
}

impl fmt::Display for NavigationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NulByte => formatter.write_str("guest path contains a NUL byte"),
            Self::PathTooLong(length) => write!(
                formatter,
                "guest path is {length} bytes; maximum is {}",
                GuestPath::MAX_BYTES
            ),
            Self::AbsolutePathRequired => formatter.write_str("an absolute guest path is required"),
            Self::RelativePathRequired => formatter.write_str("a relative path is required"),
            Self::NoHistory => formatter.write_str("no matching navigation history exists"),
            Self::StalePlan => formatter.write_str("navigation plan no longer matches pane state"),
        }
    }
}

impl std::error::Error for NavigationError {}

/// User navigation action. `Enter` accepts absolute or relative guest paths.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NavigationCommand {
    Enter(String),
    Back,
    Forward,
    Up,
    Home,
    Reload,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NavigationMode {
    Push,
    Back,
    Forward,
    Reload,
}

/// Request for a replaceable guest-VFS adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationIntent {
    pub target: GuestPath,
    pub follow_symlinks: bool,
}

/// Immutable plan that is committed only after the adapter resolves a directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationPlan {
    pub intent: NavigationIntent,
    pub(crate) mode: NavigationMode,
    pub(crate) history_revision: u64,
}

impl NavigationPlan {
    #[must_use]
    pub fn target(&self) -> &GuestPath {
        &self.intent.target
    }
}

/// Bounded browser-like location history.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LocationHistory {
    current: GuestPath,
    back: Vec<GuestPath>,
    forward: Vec<GuestPath>,
    revision: u64,
}

impl LocationHistory {
    const MAX_ENTRIES: usize = 128;

    pub(crate) fn new(location: GuestPath) -> Self {
        Self {
            current: location,
            back: Vec::new(),
            forward: Vec::new(),
            revision: 0,
        }
    }

    pub(crate) fn current(&self) -> &GuestPath {
        &self.current
    }

    pub(crate) const fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn back(&self) -> Option<&GuestPath> {
        self.back.last()
    }

    pub(crate) fn forward(&self) -> Option<&GuestPath> {
        self.forward.last()
    }

    pub(crate) fn commit(&mut self, mode: NavigationMode, resolved: GuestPath) {
        match mode {
            NavigationMode::Push => {
                if self.current != resolved {
                    push_bounded(&mut self.back, self.current.clone());
                    self.current = resolved;
                    self.forward.clear();
                }
            }
            NavigationMode::Back => {
                if self.back.pop().is_some() {
                    push_bounded(&mut self.forward, self.current.clone());
                    self.current = resolved;
                }
            }
            NavigationMode::Forward => {
                if self.forward.pop().is_some() {
                    push_bounded(&mut self.back, self.current.clone());
                    self.current = resolved;
                }
            }
            NavigationMode::Reload => {
                self.current = resolved;
            }
        }
        self.revision = self.revision.saturating_add(1);
    }
}

fn push_bounded(items: &mut Vec<GuestPath>, item: GuestPath) {
    if items.len() == LocationHistory::MAX_ENTRIES {
        items.remove(0);
    }
    items.push(item);
}
