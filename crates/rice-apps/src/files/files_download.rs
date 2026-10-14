//! Host downloads for selected files, with ZIP staging for trees and batches.

use rice_files::{ArchiveFormat, EntryKind, GuestPath};
use rice_native_app_protocol::{HostOperation, HostRequest, Message, RequestId};

use super::{FileControls, FilesActorError, RiceFiles, files_preferences};

pub(super) fn activate(
    actor: &mut RiceFiles,
    request: RequestId,
) -> Result<Vec<Message>, FilesActorError> {
    let sources = actor.selected_entries();
    if sources.is_empty() {
        return actor.emit_notice(rice_i18n::t("Select at least one item before downloading"));
    }
    if let [source] = sources.as_slice()
        && source.kind == EntryKind::File
    {
        return Ok(download_messages(
            actor,
            source.path.clone(),
            source.name.clone(),
            request,
        ));
    }

    let current = actor.location().clone();
    match FileControls::archive(
        request.get(),
        &actor.home,
        &current,
        sources,
        ArchiveFormat::Zip,
    ) {
        Ok(plan) => {
            actor.download_after_mutation = true;
            let result = actor.begin_mutation("Preparing host download", &plan, request);
            if result.is_err() {
                actor.download_after_mutation = false;
            }
            result
        }
        Err(notice) => actor.emit_notice(&notice),
    }
}

pub(super) fn completed(
    actor: &mut RiceFiles,
    path: GuestPath,
    request: RequestId,
) -> Vec<Message> {
    let filename = path.file_name().unwrap_or("Download.zip").to_owned();
    download_messages(actor, path, filename, request)
}

fn download_messages(
    actor: &mut RiceFiles,
    path: GuestPath,
    filename: String,
    request: RequestId,
) -> Vec<Message> {
    let show_storage_tip = !actor.download_tip_shown;
    let mut messages = vec![Message::HostRequest(HostRequest {
        app: actor.app,
        window: actor.window,
        operation: HostOperation::DownloadPath {
            path: path.as_str().to_owned(),
            filename,
            show_storage_tip,
        },
    })];
    if show_storage_tip
        && let Some(persist) = files_preferences::mark_download_tip_shown(actor, request)
    {
        messages.push(persist);
    }
    messages
}
