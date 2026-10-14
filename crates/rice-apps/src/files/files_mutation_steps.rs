//! Translation from validated RiceFiles operation intents to VFS mutation steps.

use rice_files::{AdapterIntent, EntryKind, GuestPath, OperationPlan};
use rice_native_app_protocol::VfsOperation;

use super::FilesActorError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MutationStep {
    pub(super) path: GuestPath,
    pub(super) operation: VfsOperation,
    pub(super) expectation: StepExpectation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StepExpectation {
    Done,
    MissingDestination,
}

pub(super) fn steps_from_plan(plan: &OperationPlan) -> Result<Vec<MutationStep>, FilesActorError> {
    plan.intents()
        .iter()
        .map(step_from_intent)
        .collect::<Result<Vec<_>, _>>()
        .map(|steps| steps.into_iter().flatten().collect())
}

fn step_from_intent(intent: &AdapterIntent) -> Result<Vec<MutationStep>, FilesActorError> {
    match intent {
        AdapterIntent::Create { path, kind } if *kind == EntryKind::Directory => {
            Ok(vec![MutationStep {
                path: path.clone(),
                operation: VfsOperation::CreateDirectory {
                    path: path.as_str().to_owned(),
                },
                expectation: StepExpectation::Done,
            }])
        }
        AdapterIntent::Rename { from, to } => Ok(vec![rename_step(from, to)]),
        AdapterIntent::Move { source, to } => Ok(vec![rename_step(&source.path, to)]),
        AdapterIntent::Copy { source, to } => Ok(vec![MutationStep {
            path: source.path.clone(),
            operation: VfsOperation::Copy {
                from: source.path.as_str().to_owned(),
                to: to.as_str().to_owned(),
                recursive: source.kind == EntryKind::Directory,
            },
            expectation: StepExpectation::Done,
        }]),
        AdapterIntent::Link { source, to } => Ok(vec![MutationStep {
            path: source.path.clone(),
            operation: VfsOperation::Link {
                from: source.path.as_str().to_owned(),
                to: to.as_str().to_owned(),
            },
            expectation: StepExpectation::Done,
        }]),
        AdapterIntent::Trash(source) => trash_steps(source),
        AdapterIntent::Delete(source) => Ok(vec![MutationStep {
            path: source.path.clone(),
            operation: VfsOperation::Remove {
                path: source.path.as_str().to_owned(),
                recursive: source.kind == EntryKind::Directory,
            },
            expectation: StepExpectation::Done,
        }]),
        AdapterIntent::Archive {
            sources,
            to,
            format,
        } => Ok(vec![MutationStep {
            path: to.clone(),
            operation: VfsOperation::Archive {
                sources: sources
                    .iter()
                    .map(|source| source.path.as_str().to_owned())
                    .collect(),
                destination: to.as_str().to_owned(),
                format: match format {
                    rice_files::ArchiveFormat::Zip => rice_native_app_protocol::ArchiveFormat::Zip,
                    rice_files::ArchiveFormat::TarGzip => {
                        rice_native_app_protocol::ArchiveFormat::TarGzip
                    }
                    rice_files::ArchiveFormat::Gzip => {
                        rice_native_app_protocol::ArchiveFormat::Gzip
                    }
                },
            },
            expectation: StepExpectation::Done,
        }]),
        AdapterIntent::Extract { archive, to } => Ok(vec![MutationStep {
            path: archive.path.clone(),
            operation: VfsOperation::Extract {
                archive: archive.path.as_str().to_owned(),
                destination: to.as_str().to_owned(),
            },
            expectation: StepExpectation::Done,
        }]),
        _ => Err(FilesActorError::UnexpectedVfsResponse),
    }
}

fn trash_steps(source: &rice_files::EntryMetadata) -> Result<Vec<MutationStep>, FilesActorError> {
    let trash = GuestPath::absolute("/root/.local/share/Trash/files")?;
    let destination = trash.join(&source.name)?;
    if destination == source.path {
        return Err(FilesActorError::UnexpectedVfsResponse);
    }
    Ok(vec![
        MutationStep {
            path: destination.clone(),
            operation: VfsOperation::Stat {
                path: destination.as_str().to_owned(),
            },
            expectation: StepExpectation::MissingDestination,
        },
        rename_step(&source.path, &destination),
    ])
}

fn rename_step(from: &GuestPath, to: &GuestPath) -> MutationStep {
    MutationStep {
        path: from.clone(),
        operation: VfsOperation::Rename {
            from: from.as_str().to_owned(),
            to: to.as_str().to_owned(),
        },
        expectation: StepExpectation::Done,
    }
}

pub(super) fn destination_of(operation: &VfsOperation) -> Option<GuestPath> {
    match operation {
        VfsOperation::Rename { to, .. }
        | VfsOperation::Copy { to, .. }
        | VfsOperation::Link { to, .. } => GuestPath::absolute(to).ok(),
        VfsOperation::Archive { destination, .. } => GuestPath::absolute(destination).ok(),
        _ => None,
    }
}
