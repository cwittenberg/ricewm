//! `RiceFiles` provider/search controls and validated mutation-plan builders.

use rice_files::{
    ArchiveFormat, CommandId, DropAction, DropPlan, EntryKind, EntryMetadata, GuestPath,
    OperationCommand, OperationPlan, OperationScope,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct FileControls {
    pub search_text: String,
    pub rename_text: String,
    pub move_text: String,
    pub preview_visible: bool,
}

impl FileControls {
    pub(super) fn create_directory(
        id: u64,
        home: &GuestPath,
        current: &GuestPath,
        name: &str,
    ) -> Result<OperationPlan, String> {
        if name.trim().is_empty() {
            return Err(String::from(rice_i18n::t("Folder name cannot be empty")));
        }
        operation_scope(home)
            .and_then(|scope| {
                let destination = current
                    .join(name.trim())
                    .map_err(rice_files::OperationError::Path)?;
                OperationCommand::create_directory(&scope, destination)
                    .and_then(|command| Ok(OperationPlan::new(command_id(id)?, command)))
            })
            .map_err(|error| rice_i18n::format("New folder is invalid: {error}", &[("error", &operation_error(&error))]))
    }

    pub(super) fn rename(
        id: u64,
        home: &GuestPath,
        source: Option<EntryMetadata>,
        name: &str,
    ) -> Result<OperationPlan, String> {
        let Some(source) = source else {
            return Err(String::from(rice_i18n::t("Select one item before renaming")));
        };
        operation_scope(home)
            .and_then(|scope| {
                let destination = source
                    .path
                    .parent()
                    .join(name.trim())
                    .map_err(rice_files::OperationError::Path)?;
                OperationCommand::rename(&scope, source, destination)
                    .and_then(|command| Ok(OperationPlan::new(command_id(id)?, command)))
            })
            .map_err(|error| rice_i18n::format("Rename is invalid: {error}", &[("error", &operation_error(&error))]))
    }

    pub(super) fn move_to(
        id: u64,
        home: &GuestPath,
        current: &GuestPath,
        sources: Vec<EntryMetadata>,
        target: &str,
    ) -> Result<OperationPlan, String> {
        if sources.is_empty() {
            return Err(String::from(rice_i18n::t("Select at least one item before moving")));
        }
        operation_scope(home)
            .and_then(|scope| {
                let destination = GuestPath::resolve(current, target.trim())
                    .map_err(rice_files::OperationError::Path)?;
                OperationCommand::move_to(&scope, sources, destination)
                    .and_then(|command| Ok(OperationPlan::new(command_id(id)?, command)))
            })
            .map_err(|error| rice_i18n::format("Move is invalid: {error}", &[("error", &operation_error(&error))]))
    }

    pub(super) fn copy_to(
        id: u64,
        home: &GuestPath,
        current: &GuestPath,
        sources: Vec<EntryMetadata>,
        target: &str,
    ) -> Result<OperationPlan, String> {
        if sources.is_empty() {
            return Err(String::from(rice_i18n::t("Select at least one item before copying")));
        }
        operation_scope(home)
            .and_then(|scope| {
                let destination = GuestPath::resolve(current, target.trim())
                    .map_err(rice_files::OperationError::Path)?;
                OperationCommand::copy(&scope, sources, destination)
                    .and_then(|command| Ok(OperationPlan::new(command_id(id)?, command)))
            })
            .map_err(|error| rice_i18n::format("Copy is invalid: {error}", &[("error", &operation_error(&error))]))
    }

    pub(super) fn trash(
        id: u64,
        home: &GuestPath,
        sources: Vec<EntryMetadata>,
    ) -> Result<OperationPlan, String> {
        if sources.is_empty() {
            return Err(String::from(rice_i18n::t("Select at least one item before moving to Trash")));
        }
        operation_scope(home)
            .and_then(|scope| {
                OperationCommand::trash(&scope, sources)
                    .and_then(|command| Ok(OperationPlan::new(command_id(id)?, command)))
            })
            .map_err(|error| rice_i18n::format("Move to Trash is invalid: {error}", &[("error", &operation_error(&error))]))
    }

    pub(super) fn delete(
        id: u64,
        home: &GuestPath,
        sources: Vec<EntryMetadata>,
    ) -> Result<OperationPlan, String> {
        if sources.is_empty() {
            return Err(String::from(rice_i18n::t("Select at least one item before deleting")));
        }
        operation_scope(home)
            .and_then(|scope| {
                OperationCommand::delete(&scope, sources)
                    .and_then(|command| Ok(OperationPlan::new(command_id(id)?, command)))
            })
            .map_err(|error| rice_i18n::format("Delete is invalid: {error}", &[("error", &operation_error(&error))]))
    }

    pub(super) fn archive(
        id: u64,
        home: &GuestPath,
        current: &GuestPath,
        sources: Vec<EntryMetadata>,
        format: ArchiveFormat,
    ) -> Result<OperationPlan, String> {
        if sources.is_empty() {
            return Err(String::from(rice_i18n::t("Select at least one item before compressing")));
        }
        let extension = match format {
            ArchiveFormat::Zip => ".zip",
            ArchiveFormat::TarGzip => ".tar.gz",
            ArchiveFormat::Gzip => ".gz",
        };
        let base = if sources.len() == 1 {
            sources[0].name.clone()
        } else {
            String::from("Archive")
        };
        operation_scope(home)
            .and_then(|scope| {
                let destination = current
                    .join(&format!("{base}{extension}"))
                    .map_err(rice_files::OperationError::Path)?;
                OperationCommand::archive(&scope, sources, destination, format)
                    .and_then(|command| Ok(OperationPlan::new(command_id(id)?, command)))
            })
            .map_err(|error| rice_i18n::format("Compression is invalid: {error}", &[("error", &operation_error(&error))]))
    }

    pub(super) fn extract(
        id: u64,
        home: &GuestPath,
        current: &GuestPath,
        sources: Vec<EntryMetadata>,
    ) -> Result<OperationPlan, String> {
        let [archive] = sources.as_slice() else {
            return Err(String::from(rice_i18n::t("Select one archive before extracting")));
        };
        if archive.kind != EntryKind::File || !is_archive_name(&archive.name) {
            return Err(String::from(rice_i18n::t("Select a ZIP, TAR, TAR.GZ, TGZ, or GZ archive")));
        }
        operation_scope(home)
            .and_then(|scope| {
                OperationCommand::extract(&scope, archive.clone(), current.clone())
                    .and_then(|command| Ok(OperationPlan::new(command_id(id)?, command)))
            })
            .map_err(|error| rice_i18n::format("Extraction is invalid: {error}", &[("error", &operation_error(&error))]))
    }

    pub(super) fn drop(
        id: u64,
        home: &GuestPath,
        sources: Vec<EntryMetadata>,
        destination: GuestPath,
        action: DropAction,
    ) -> Result<OperationPlan, String> {
        operation_scope(home)
            .and_then(|scope| {
                DropPlan::new(command_id(id)?, &scope, sources, destination, action)
                    .map(|drop| drop.operation)
            })
            .map_err(|error| rice_i18n::format("Drop is invalid: {error}", &[("error", &operation_error(&error))]))
    }
}

pub(super) fn is_archive_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".zip")
        || lower.ends_with(".tar")
        || lower.ends_with(".tar.gz")
        || lower.ends_with(".tgz")
        || lower.ends_with(".gz")
}

fn operation_scope(home: &GuestPath) -> Result<OperationScope, rice_files::OperationError> {
    let local_mount =
        GuestPath::absolute("/mnt/local").map_err(rice_files::OperationError::Path)?;
    OperationScope::new(vec![home.clone(), local_mount, GuestPath::absolute("/run/ricewm/mounts").map_err(rice_files::OperationError::Path)?])
}

fn command_id(value: u64) -> Result<CommandId, rice_files::OperationError> {
    CommandId::new(value)
}


fn operation_error(error: &rice_files::OperationError) -> String {
    use rice_files::OperationError as E;
    match error {
        E::Path(error) => navigation_error(error),
        E::TooManyRoots(count) => rice_i18n::format("Too many storage roots: {count}", &[("count", &count.to_string())]),
        E::TooManySources(count) => rice_i18n::format("Too many selected items: {count}", &[("count", &count.to_string())]),
        E::OutsideScope(path) => rice_i18n::format("The path is outside accessible storage: {path}", &[("path", path.as_str())]),
        _ => rice_i18n::t(match error {
            E::ZeroCommandId => "The file operation request is invalid",
            E::EmptyScope => "No accessible storage is available",
            E::EmptySources => "Select at least one item",
            E::RootOperation => "The storage root cannot be changed",
            E::InvalidMetadataName => "The item name does not match its path",
            E::RenameCrossesDirectory => "Rename must keep the item in the same folder",
            E::SamePath => "The source and destination are the same",
            E::TargetInsideSource => "The destination cannot be inside the source folder",
            E::GzipRequiresOneFile => "GZIP compression requires exactly one file",
            E::ArchiveMustBeFile => "The selected archive must be a file",
            _ => unreachable!(),
        }).to_owned(),
    }
}

pub(super) fn navigation_error(error: &rice_files::NavigationError) -> String {
    use rice_files::NavigationError as E;
    match error {
        E::PathTooLong(length) => rice_i18n::format("The path is {length} bytes; the maximum is {maximum}", &[("length", &length.to_string()), ("maximum", &GuestPath::MAX_BYTES.to_string())]),
        _ => rice_i18n::t(match error {
            E::NulByte => "The path contains a null character",
            E::AbsolutePathRequired => "Enter an absolute path",
            E::RelativePathRequired => "Enter a relative path",
            E::NoHistory => "There is no matching navigation history",
            E::StalePlan => "The folder changed; try the operation again",
            E::PathTooLong(_) => unreachable!(),
        }).to_owned(),
    }
}

#[cfg(test)]
mod translation_tests {
    use super::*;
    use rice_files::{NavigationError as N, OperationError as E};

    #[test]
    fn all_operation_validation_errors_translate_and_preserve_path_arguments() {
        let errors = [E::ZeroCommandId, E::EmptyScope, E::TooManyRoots(99), E::EmptySources,
            E::TooManySources(999), E::OutsideScope(GuestPath::absolute("/outside/{error}").unwrap()),
            E::RootOperation, E::InvalidMetadataName, E::RenameCrossesDirectory, E::SamePath,
            E::TargetInsideSource, E::GzipRequiresOneFile, E::ArchiveMustBeFile,
            E::Path(N::NulByte), E::Path(N::PathTooLong(9999)), E::Path(N::AbsolutePathRequired),
            E::Path(N::RelativePathRequired), E::Path(N::NoHistory), E::Path(N::StalePlan)];
        for locale in [1, 2] {
            rice_i18n::with_locale(locale, || {
                for error in &errors {
                    let translated = operation_error(error);
                    assert_ne!(translated, rice_i18n::with_locale(0, || operation_error(error)), "{error:?}");
                    if let E::OutsideScope(path) = error { assert!(translated.contains(path.as_str())); }
                }
                let root = GuestPath::absolute("/root").unwrap();
                let message = FileControls::create_directory(1, &root, &root, "../outside/{error}").unwrap_err();
                assert!(message.contains("/outside/{error}"), "{message}");
                assert!(!message.contains("OutsideScope"));
                assert_ne!(FileControls::create_directory(1, &root, &root, "").unwrap_err(), "Folder name cannot be empty");
            });
        }
    }
}
