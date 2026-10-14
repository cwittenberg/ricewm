//! Bounded local directory completion and keyboard selection.

use super::{FilesActorError, RiceFiles};
use rice_files::NavigationCommand;
use rice_native_app_protocol::{Message, RequestId};

impl RiceFiles {
    pub(super) fn update_location_text(
        &mut self,
        value: &str,
    ) -> Result<Vec<Message>, FilesActorError> {
        value.clone_into(&mut self.runtime_mut()?.location_text);
        let location = self.location().clone();
        let intent = self.runtime_mut()?.completion.plan(&location, value)?;
        let mut names = if intent.directory == *self.location() {
            self.runtime()?
                .directory_entries
                .iter()
                .filter(|entry| entry.kind == rice_files::EntryKind::Directory)
                .map(|entry| entry.name.clone())
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        for tab in self.model.windows().iter().flat_map(|window| window.tabs()) {
            let location = tab.primary.location();
            if location.parent() == intent.directory
                && let Some(name) = location.file_name()
            {
                names.push(name.to_owned());
            }
        }
        let runtime = self.runtime_mut()?;
        runtime.completion.resolve(&intent, names)?;
        runtime.completion_index = (!runtime.completion.suggestions().is_empty()).then_some(0);
        self.emit_full().map(|message| vec![message])
    }

    pub(super) fn move_completion(&mut self, delta: i8) -> Result<Vec<Message>, FilesActorError> {
        let count = self.runtime()?.completion.suggestions().len();
        if count == 0 {
            return Ok(Vec::new());
        }
        let current = self.runtime()?.completion_index.unwrap_or(0);
        self.runtime_mut()?.completion_index = Some(if delta < 0 {
            current.checked_sub(1).unwrap_or(count - 1)
        } else {
            (current + 1) % count
        });
        self.emit_full().map(|message| vec![message])
    }

    pub(super) fn activate_location(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let runtime = self.runtime()?;
        let target = runtime
            .completion_index
            .and_then(|index| runtime.completion.suggestions().get(index))
            .map_or_else(
                || runtime.location_text.clone(),
                |path| path.as_str().to_owned(),
            );
        self.runtime_mut()?.completion_index = None;
        if target.starts_with("smb://") || target.starts_with("nfs://") {
            self.remote.open_url(&target);
            return self.emit_full().map(|message| vec![message]);
        }
        self.begin_navigation(NavigationCommand::Enter(target), request)
    }

    pub(super) fn activate_completion(
        &mut self,
        index: u64,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let Some(path) = usize::try_from(index)
            .ok()
            .and_then(|index| self.runtime().ok()?.completion.suggestions().get(index))
            .cloned()
        else {
            return Ok(Vec::new());
        };
        path.as_str()
            .clone_into(&mut self.runtime_mut()?.location_text);
        self.runtime_mut()?.completion_index = None;
        self.begin_navigation(NavigationCommand::Enter(path.as_str().to_owned()), request)
    }
}
