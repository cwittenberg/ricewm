//! Validated file-operation commands, conflict strategy, and bounded progress.

use crate::{EntryKind, EntryMetadata, GuestPath};
use core::fmt;

const MAX_SOURCES: usize = 4096;
const MAX_ROOTS: usize = 64;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CommandId(u64);

impl CommandId {
    /// # Errors
    ///
    /// Returns [`OperationError::ZeroCommandId`] for the reserved zero value.
    pub fn new(value: u64) -> Result<Self, OperationError> {
        if value == 0 {
            Err(OperationError::ZeroCommandId)
        } else {
            Ok(Self(value))
        }
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Roots selected by the user or granted by a provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationScope {
    roots: Vec<GuestPath>,
}

impl OperationScope {
    /// # Errors
    ///
    /// Returns an error when no roots or too many roots are supplied.
    pub fn new(mut roots: Vec<GuestPath>) -> Result<Self, OperationError> {
        if roots.is_empty() {
            return Err(OperationError::EmptyScope);
        }
        if roots.len() > MAX_ROOTS {
            return Err(OperationError::TooManyRoots(roots.len()));
        }
        roots.sort();
        roots.dedup();
        let mut minimal: Vec<GuestPath> = Vec::new();
        for root in roots {
            if !minimal.iter().any(|existing| root.is_within(existing)) {
                minimal.push(root);
            }
        }
        Ok(Self { roots: minimal })
    }

    #[must_use]
    pub fn allows(&self, path: &GuestPath) -> bool {
        self.roots.iter().any(|root| path.is_within(root))
    }

    #[must_use]
    pub fn roots(&self) -> &[GuestPath] {
        &self.roots
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum OperationKind {
    Create {
        destination: GuestPath,
        kind: EntryKind,
    },
    Rename {
        source: EntryMetadata,
        destination: GuestPath,
    },
    Copy {
        sources: Vec<EntryMetadata>,
        target_directory: GuestPath,
    },
    Move {
        sources: Vec<EntryMetadata>,
        target_directory: GuestPath,
    },
    Link {
        sources: Vec<EntryMetadata>,
        target_directory: GuestPath,
    },
    Trash(Vec<EntryMetadata>),
    Delete(Vec<EntryMetadata>),
    Restore {
        source: EntryMetadata,
        original_destination: GuestPath,
    },
    Archive {
        sources: Vec<EntryMetadata>,
        destination: GuestPath,
        format: ArchiveFormat,
    },
    Extract {
        archive: EntryMetadata,
        destination: GuestPath,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveFormat {
    Zip,
    TarGzip,
    Gzip,
}

/// Immutable command; variants can only be produced by validating constructors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationCommand {
    kind: OperationKind,
}

impl OperationCommand {
    /// # Errors
    ///
    /// Returns an error when `destination` is outside the granted scope.
    pub fn create_file(
        scope: &OperationScope,
        destination: GuestPath,
    ) -> Result<Self, OperationError> {
        Self::create(scope, destination, EntryKind::File)
    }

    /// # Errors
    ///
    /// Returns an error when `destination` is outside the granted scope.
    pub fn create_directory(
        scope: &OperationScope,
        destination: GuestPath,
    ) -> Result<Self, OperationError> {
        Self::create(scope, destination, EntryKind::Directory)
    }

    fn create(
        scope: &OperationScope,
        destination: GuestPath,
        kind: EntryKind,
    ) -> Result<Self, OperationError> {
        validate_destination(scope, &destination)?;
        Ok(Self {
            kind: OperationKind::Create { destination, kind },
        })
    }

    /// # Errors
    ///
    /// Returns an error for invalid metadata, scope, or rename destinations.
    pub fn rename(
        scope: &OperationScope,
        source: EntryMetadata,
        destination: GuestPath,
    ) -> Result<Self, OperationError> {
        validate_source(scope, &source)?;
        validate_destination(scope, &destination)?;
        if source.path.parent() != destination.parent() {
            return Err(OperationError::RenameCrossesDirectory);
        }
        if source.path == destination {
            return Err(OperationError::SamePath);
        }
        Ok(Self {
            kind: OperationKind::Rename {
                source,
                destination,
            },
        })
    }

    /// # Errors
    ///
    /// Returns an error for invalid sources, scope, or recursive targets.
    pub fn copy(
        scope: &OperationScope,
        sources: Vec<EntryMetadata>,
        target_directory: GuestPath,
    ) -> Result<Self, OperationError> {
        Self::transfer(scope, sources, target_directory, TransferKind::Copy)
    }

    /// # Errors
    ///
    /// Returns an error for invalid sources, scope, or recursive targets.
    pub fn move_to(
        scope: &OperationScope,
        sources: Vec<EntryMetadata>,
        target_directory: GuestPath,
    ) -> Result<Self, OperationError> {
        Self::transfer(scope, sources, target_directory, TransferKind::Move)
    }

    /// # Errors
    ///
    /// Returns an error for invalid sources, scope, or recursive targets.
    pub fn link(
        scope: &OperationScope,
        sources: Vec<EntryMetadata>,
        target_directory: GuestPath,
    ) -> Result<Self, OperationError> {
        Self::transfer(scope, sources, target_directory, TransferKind::Link)
    }

    fn transfer(
        scope: &OperationScope,
        sources: Vec<EntryMetadata>,
        target_directory: GuestPath,
        transfer: TransferKind,
    ) -> Result<Self, OperationError> {
        validate_destination(scope, &target_directory)?;
        let sources = normalize_sources(scope, sources)?;
        for source in &sources {
            let destination = target_directory
                .join(&source.name)
                .map_err(OperationError::Path)?;
            if destination == source.path {
                return Err(OperationError::SamePath);
            }
            if source.kind == EntryKind::Directory && target_directory.is_within(&source.path) {
                return Err(OperationError::TargetInsideSource);
            }
        }
        let kind = match transfer {
            TransferKind::Copy => OperationKind::Copy {
                sources,
                target_directory,
            },
            TransferKind::Move => OperationKind::Move {
                sources,
                target_directory,
            },
            TransferKind::Link => OperationKind::Link {
                sources,
                target_directory,
            },
        };
        Ok(Self { kind })
    }

    /// # Errors
    ///
    /// Returns an error for empty, excessive, invalid, or out-of-scope sources.
    pub fn trash(
        scope: &OperationScope,
        sources: Vec<EntryMetadata>,
    ) -> Result<Self, OperationError> {
        Ok(Self {
            kind: OperationKind::Trash(normalize_sources(scope, sources)?),
        })
    }

    /// # Errors
    ///
    /// Returns an error for empty, excessive, invalid, or out-of-scope sources.
    pub fn delete(
        scope: &OperationScope,
        sources: Vec<EntryMetadata>,
    ) -> Result<Self, OperationError> {
        Ok(Self {
            kind: OperationKind::Delete(normalize_sources(scope, sources)?),
        })
    }

    /// # Errors
    ///
    /// Returns an error for invalid metadata, scope, or restore destinations.
    pub fn restore(
        scope: &OperationScope,
        source: EntryMetadata,
        original_destination: GuestPath,
    ) -> Result<Self, OperationError> {
        validate_source(scope, &source)?;
        validate_destination(scope, &original_destination)?;
        if source.path == original_destination {
            return Err(OperationError::SamePath);
        }
        Ok(Self {
            kind: OperationKind::Restore {
                source,
                original_destination,
            },
        })
    }

    /// Builds an archive from one or more selected files or directories.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid sources, destination, or multi-item gzip.
    pub fn archive(
        scope: &OperationScope,
        sources: Vec<EntryMetadata>,
        destination: GuestPath,
        format: ArchiveFormat,
    ) -> Result<Self, OperationError> {
        validate_destination(scope, &destination)?;
        let sources = normalize_sources(scope, sources)?;
        if sources.iter().any(|source| source.path == destination) {
            return Err(OperationError::SamePath);
        }
        if format == ArchiveFormat::Gzip
            && (sources.len() != 1 || sources[0].kind != EntryKind::File)
        {
            return Err(OperationError::GzipRequiresOneFile);
        }
        Ok(Self {
            kind: OperationKind::Archive {
                sources,
                destination,
                format,
            },
        })
    }

    /// Extracts one selected archive into a directory.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid archive or destination.
    pub fn extract(
        scope: &OperationScope,
        archive: EntryMetadata,
        destination: GuestPath,
    ) -> Result<Self, OperationError> {
        validate_source(scope, &archive)?;
        validate_destination(scope, &destination)?;
        if archive.kind != EntryKind::File {
            return Err(OperationError::ArchiveMustBeFile);
        }
        Ok(Self {
            kind: OperationKind::Extract {
                archive,
                destination,
            },
        })
    }
}

#[derive(Clone, Copy)]
enum TransferKind {
    Copy,
    Move,
    Link,
}

/// A single capability call for the composition-root adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdapterIntent {
    Create {
        path: GuestPath,
        kind: EntryKind,
    },
    Rename {
        from: GuestPath,
        to: GuestPath,
    },
    Copy {
        source: EntryMetadata,
        to: GuestPath,
    },
    Move {
        source: EntryMetadata,
        to: GuestPath,
    },
    Link {
        source: EntryMetadata,
        to: GuestPath,
    },
    Trash(EntryMetadata),
    Delete(EntryMetadata),
    Restore {
        source: EntryMetadata,
        to: GuestPath,
    },
    Archive {
        sources: Vec<EntryMetadata>,
        to: GuestPath,
        format: ArchiveFormat,
    },
    Extract {
        archive: EntryMetadata,
        to: GuestPath,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationPlan {
    id: CommandId,
    command: OperationCommand,
    intents: Vec<AdapterIntent>,
}

impl OperationPlan {
    #[must_use]
    pub fn new(id: CommandId, command: OperationCommand) -> Self {
        let intents = intents_for(&command.kind);
        Self {
            id,
            command,
            intents,
        }
    }

    #[must_use]
    pub const fn id(&self) -> CommandId {
        self.id
    }

    #[must_use]
    pub fn command(&self) -> &OperationCommand {
        &self.command
    }

    #[must_use]
    pub fn intents(&self) -> &[AdapterIntent] {
        &self.intents
    }
}

fn intents_for(kind: &OperationKind) -> Vec<AdapterIntent> {
    match kind {
        OperationKind::Create { destination, kind } => vec![AdapterIntent::Create {
            path: destination.clone(),
            kind: *kind,
        }],
        OperationKind::Rename {
            source,
            destination,
        } => vec![AdapterIntent::Rename {
            from: source.path.clone(),
            to: destination.clone(),
        }],
        OperationKind::Copy {
            sources,
            target_directory,
        } => transfer_intents(sources, target_directory, TransferKind::Copy),
        OperationKind::Move {
            sources,
            target_directory,
        } => transfer_intents(sources, target_directory, TransferKind::Move),
        OperationKind::Link {
            sources,
            target_directory,
        } => transfer_intents(sources, target_directory, TransferKind::Link),
        OperationKind::Trash(sources) => {
            sources.iter().cloned().map(AdapterIntent::Trash).collect()
        }
        OperationKind::Delete(sources) => {
            sources.iter().cloned().map(AdapterIntent::Delete).collect()
        }
        OperationKind::Restore {
            source,
            original_destination,
        } => vec![AdapterIntent::Restore {
            source: source.clone(),
            to: original_destination.clone(),
        }],
        OperationKind::Archive {
            sources,
            destination,
            format,
        } => vec![AdapterIntent::Archive {
            sources: sources.clone(),
            to: destination.clone(),
            format: *format,
        }],
        OperationKind::Extract {
            archive,
            destination,
        } => vec![AdapterIntent::Extract {
            archive: archive.clone(),
            to: destination.clone(),
        }],
    }
}

fn transfer_intents(
    sources: &[EntryMetadata],
    target: &GuestPath,
    transfer: TransferKind,
) -> Vec<AdapterIntent> {
    sources
        .iter()
        .map(|source| {
            let destination = target
                .join(&source.name)
                .expect("validated source names remain valid path components");
            match transfer {
                TransferKind::Copy => AdapterIntent::Copy {
                    source: source.clone(),
                    to: destination,
                },
                TransferKind::Move => AdapterIntent::Move {
                    source: source.clone(),
                    to: destination,
                },
                TransferKind::Link => AdapterIntent::Link {
                    source: source.clone(),
                    to: destination,
                },
            }
        })
        .collect()
}

fn validate_destination(
    scope: &OperationScope,
    destination: &GuestPath,
) -> Result<(), OperationError> {
    if !scope.allows(destination) {
        return Err(OperationError::OutsideScope(destination.clone()));
    }
    if destination.is_root() {
        return Err(OperationError::RootOperation);
    }
    Ok(())
}

fn validate_source(scope: &OperationScope, source: &EntryMetadata) -> Result<(), OperationError> {
    if !scope.allows(&source.path) {
        return Err(OperationError::OutsideScope(source.path.clone()));
    }
    if source.path.is_root() {
        return Err(OperationError::RootOperation);
    }
    if source.name != source.path.file_name().unwrap_or("/") {
        return Err(OperationError::InvalidMetadataName);
    }
    Ok(())
}

fn normalize_sources(
    scope: &OperationScope,
    mut sources: Vec<EntryMetadata>,
) -> Result<Vec<EntryMetadata>, OperationError> {
    if sources.is_empty() {
        return Err(OperationError::EmptySources);
    }
    if sources.len() > MAX_SOURCES {
        return Err(OperationError::TooManySources(sources.len()));
    }
    for source in &sources {
        validate_source(scope, source)?;
    }
    sources.sort_by(|left, right| left.path.cmp(&right.path));
    sources.dedup_by(|left, right| left.path == right.path);
    let mut minimal: Vec<EntryMetadata> = Vec::new();
    for source in sources {
        if !minimal.iter().any(|parent| {
            parent.kind == EntryKind::Directory && source.path.is_within(&parent.path)
        }) {
            minimal.push(source);
        }
    }
    Ok(minimal)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConflictAction {
    Skip,
    Replace,
    Rename(GuestPath),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConflictDecision {
    Skip,
    Replace,
    Rename(GuestPath),
    ApplyAll(ConflictAction),
    Cancel,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ConflictStrategy {
    apply_all: Option<ConflictAction>,
}

impl ConflictStrategy {
    #[must_use]
    pub fn automatic(&self) -> Option<&ConflictAction> {
        self.apply_all.as_ref()
    }

    /// # Errors
    ///
    /// Returns an error for cancellation or an invalid rename/apply-all choice.
    pub fn decide(
        &mut self,
        destination: &GuestPath,
        decision: ConflictDecision,
    ) -> Result<ConflictAction, ConflictError> {
        if decision == ConflictDecision::Cancel {
            return Err(ConflictError::Cancelled);
        }
        if let Some(action) = &self.apply_all {
            return Ok(action.clone());
        }
        match decision {
            ConflictDecision::Skip => Ok(ConflictAction::Skip),
            ConflictDecision::Replace => Ok(ConflictAction::Replace),
            ConflictDecision::Rename(path) => validate_conflict_rename(destination, path),
            ConflictDecision::ApplyAll(action) => {
                if matches!(action, ConflictAction::Rename(_)) {
                    return Err(ConflictError::RenameCannotApplyAll);
                }
                self.apply_all = Some(action.clone());
                Ok(action)
            }
            ConflictDecision::Cancel => unreachable!("cancel handled before sticky strategy"),
        }
    }
}

fn validate_conflict_rename(
    destination: &GuestPath,
    renamed: GuestPath,
) -> Result<ConflictAction, ConflictError> {
    if renamed == *destination || renamed.parent() != destination.parent() {
        Err(ConflictError::InvalidRename)
    } else {
        Ok(ConflictAction::Rename(renamed))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConflictError {
    InvalidRename,
    RenameCannotApplyAll,
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationError {
    Path(crate::NavigationError),
    ZeroCommandId,
    EmptyScope,
    TooManyRoots(usize),
    EmptySources,
    TooManySources(usize),
    OutsideScope(GuestPath),
    RootOperation,
    InvalidMetadataName,
    RenameCrossesDirectory,
    SamePath,
    TargetInsideSource,
    GzipRequiresOneFile,
    ArchiveMustBeFile,
}

impl fmt::Display for OperationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for OperationError {}
