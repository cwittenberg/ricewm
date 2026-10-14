//! Installed-application Open/Open With behavior and chooser composition.

use super::{
    FilesActorError, RiceFiles,
    files_view::{create, create_with_state},
};
use rice_files::{EntryKind, EntryMetadata, NavigationCommand, infer_media_type};
use rice_native_app_protocol::{
    APPLICATION_CATALOG_VERSION, DataProviderOperation, DataProviderRequest, DataProviderResponse,
    DataProviderResult, InstalledApplication, Message, RequestId,
};
use rice_ui::{IconId, Patch, WidgetKind, WidgetState};

// Keep the chooser separate from Places bookmarks (200_000) and other dialogs.
pub(super) const OPEN_WITH_DIALOG: u64 = 360_000;
pub(super) const OPEN_WITH_CANCEL: u64 = 360_001;
pub(super) const OPEN_WITH_REMEMBER: u64 = 360_002;
pub(super) const OPEN_WITH_APP_BASE: u64 = 360_100;
const OPEN_WITH_CONTENT: u64 = 360_003;
const OPEN_WITH_ACTIONS: u64 = 360_004;
pub(super) const OPEN_WITH_SEARCH: u64 = 360_005;
pub(super) const OPEN_WITH_LIST: u64 = 360_006;
pub(super) const OPEN_WITH_COMMAND: u64 = 360_007;
pub(super) const OPEN_WITH_RUN_COMMAND: u64 = 360_008;
const OPEN_WITH_LIST_LABEL: u64 = 360_009;
const OPEN_WITH_COMMAND_LABEL: u64 = 360_010;
pub(super) const OPEN_WITH_LIST_CONTENT: u64 = 360_011;
const OPEN_WITH_EMPTY: u64 = 360_012;
const OPEN_WITH_COMMAND_HELP: u64 = 360_013;
const OPEN_WITH_RECOMMENDED_LABEL: u64 = 360_014;
const OPEN_WITH_OTHER_LABEL: u64 = 360_015;
pub(super) const OPEN_THEME_LOAD: u64 = 360_016;
pub(super) const OPEN_THEME_EDIT: u64 = 360_017;
const OPEN_WITH_ROW_HEIGHT: u32 = 40;
const OPEN_WITH_VIEWPORT_HEIGHT: u32 = 280;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum OpenWithFocus {
    #[default]
    Search,
    Command,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct OpenWithState {
    entry: EntryMetadata,
    mime_type: String,
    candidates: Vec<InstalledApplication>,
    loading: bool,
    remember_default: bool,
    query: String,
    command: String,
    scroll: u32,
    focus: OpenWithFocus,
}

impl OpenWithState {
    fn filtered_candidates(&self) -> Vec<&InstalledApplication> {
        let query = self.query.trim().to_lowercase();
        self.candidates
            .iter()
            .filter(|application| {
                query.is_empty()
                    || application.name.to_lowercase().contains(&query)
                    || application.id.to_lowercase().contains(&query)
            })
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum PendingApplication {
    Catalog {
        request: RequestId,
        entry: EntryMetadata,
        explicit: bool,
    },
    Launch {
        request: RequestId,
        application_id: String,
    },
}

impl RiceFiles {
    pub(super) fn open_selected(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let Some(entry) = self.selected_entries().into_iter().next() else {
            return Ok(Vec::new());
        };
        if entry.kind == EntryKind::Directory {
            return self.begin_navigation(
                NavigationCommand::Enter(entry.path.as_str().to_owned()),
                request,
            );
        }
        if is_executable_program(&entry) {
            return Ok(vec![Message::Launch {
                app: self.app,
                entrypoint: String::from("rice-term"),
                arguments: vec![format!("exec {}", shell_quote(entry.path.as_str()),)],
            }]);
        }
        self.request_applications(entry, false, request)
    }

    pub(super) fn show_open_with(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let Some(entry) = self.selected_entries().into_iter().next() else {
            return Ok(Vec::new());
        };
        self.request_applications(entry, true, request)
    }

    fn request_applications(
        &mut self,
        entry: EntryMetadata,
        explicit: bool,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        if self.pending_application.as_ref().is_some_and(|pending| {
            matches!(
                pending,
                PendingApplication::Catalog {
                    entry: pending_entry,
                    explicit: pending_explicit,
                    ..
                } if pending_entry.path == entry.path && *pending_explicit == explicit
            )
        }) {
            // Native double-clicks can contain both an activation gesture and
            // a semantic Activate event. The first event owns the catalog
            // lookup; the duplicate is the same user intent and must be a
            // quiet no-op rather than an error notification (or a second
            // application launch).
            return Ok(Vec::new());
        }
        if self.pending_application.is_some() {
            return self.emit_notice(rice_i18n::t("An application lookup is already in progress"));
        }
        let mime_type = infer_media_type(&entry).to_owned();
        self.pending_application = Some(PendingApplication::Catalog {
            request,
            entry: entry.clone(),
            explicit,
        });
        let mut messages = Vec::new();
        if explicit {
            self.open_with = Some(OpenWithState {
                entry,
                mime_type: mime_type.clone(), candidates: Vec::new(), loading: true,
                remember_default: false, query: String::new(), command: String::new(),
                scroll: 0, focus: OpenWithFocus::Search,
            });
            messages.push(self.emit_full()?);
        }
        messages.push(Message::DataProviderRequest(DataProviderRequest {
            request, app: self.app,
            operation: DataProviderOperation::ApplicationCatalog { mime_type },
        }));
        Ok(messages)
    }

    pub(super) fn close_open_with(&mut self) -> Result<Vec<Message>, FilesActorError> {
        self.open_with = None;
        self.emit_full().map(|message| vec![message])
    }

    pub(super) fn toggle_open_with_default(&mut self) -> Result<Vec<Message>, FilesActorError> {
        if let Some(state) = &mut self.open_with {
            state.remember_default = !state.remember_default;
        }
        self.emit_full().map(|message| vec![message])
    }

    pub(super) fn search_open_with(
        &mut self,
        query: &str,
    ) -> Result<Vec<Message>, FilesActorError> {
        if let Some(state) = &mut self.open_with {
            state.query = query.chars().take(128).collect();
            state.scroll = 0;
            state.focus = OpenWithFocus::Search;
        }
        self.emit_full().map(|message| vec![message])
    }

    pub(super) fn scroll_open_with(
        &mut self,
        value: &str,
    ) -> Result<Vec<Message>, FilesActorError> {
        let Some((position, maximum)) = value.split_once('/') else {
            return Ok(Vec::new());
        };
        let (Ok(position), Ok(maximum)) = (position.parse::<u32>(), maximum.parse::<u32>()) else {
            return Ok(Vec::new());
        };
        if let Some(state) = &mut self.open_with {
            state.scroll = position.min(maximum).min(
                u32::try_from(rice_native_app_protocol::MAX_APPLICATIONS)
                    .unwrap_or_default()
                    .saturating_mul(OPEN_WITH_ROW_HEIGHT),
            );
        }
        self.emit_full().map(|message| vec![message])
    }

    pub(super) fn update_open_with_command(
        &mut self,
        command: &str,
    ) -> Result<Vec<Message>, FilesActorError> {
        if let Some(state) = &mut self.open_with {
            state.command = command.chars().take(1_024).collect();
            state.focus = OpenWithFocus::Command;
        }
        self.emit_full().map(|message| vec![message])
    }

    pub(super) fn run_open_with_command(&mut self) -> Result<Vec<Message>, FilesActorError> {
        let Some(state) = self.open_with.take() else {
            return Ok(Vec::new());
        };
        let command = state.command.trim().to_owned();
        if command.is_empty()
            || command
                .chars()
                .any(|character| matches!(character, '\0' | '\n' | '\r'))
        {
            self.open_with = Some(state);
            return self.emit_notice(rice_i18n::t("Enter one bash command without line breaks"));
        }
        let startup = format!("exec {command} {}", shell_quote(state.entry.path.as_str()));
        Ok(vec![
            self.emit_full()?,
            Message::Launch {
                app: self.app,
                entrypoint: String::from("rice-term"),
                arguments: vec![startup],
            },
        ])
    }

    pub(super) fn choose_theme(
        &mut self,
        load: bool,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let Some(state) = self.open_with.take() else { return Ok(Vec::new()); };
        if state.loading || !state.entry.path.as_str().to_ascii_lowercase().ends_with(".theme") {
            self.open_with = Some(state);
            return Ok(Vec::new());
        }
        let mut messages = vec![self.emit_full()?];
        if load {
            messages.push(Message::Launch {
                app: self.app,
                entrypoint: "rice-settings".into(),
                arguments: vec![format!("--load-theme={}", state.entry.path.as_str())],
            });
        } else {
            messages.extend(self.launch_application("native:rice-paper".into(), state.entry, state.mime_type, false, request)?);
        }
        Ok(messages)
    }

    pub(super) fn choose_open_with(
        &mut self,
        index: u64,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let Some(state) = self.open_with.take() else {
            return Ok(Vec::new());
        };
        let selected = usize::try_from(index)
            .ok()
            .and_then(|index| state.filtered_candidates().get(index).copied())
            .cloned();
        let Some(application) = selected else {
            self.open_with = Some(state);
            return Ok(Vec::new());
        };
        self.launch_application(
            application.id,
            state.entry,
            state.mime_type,
            state.remember_default,
            request,
        )
    }

    pub(crate) fn handle_application_response(
        &mut self,
        response: &DataProviderResponse,
        next_request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let pending = self
            .pending_application
            .take()
            .ok_or(FilesActorError::UnexpectedDataProviderResponse)?;
        match (pending, &response.result) {
            (
                PendingApplication::Catalog {
                    request,
                    entry,
                    explicit,
                },
                DataProviderResult::Applications {
                    schema_version,
                    mime_type,
                    entries,
                },
            ) if request == response.request
                && *schema_version == APPLICATION_CATALOG_VERSION
                && mime_type == infer_media_type(&entry) =>
            {
                if explicit && self.open_with.is_none() { return Ok(Vec::new()); }
                self.accept_catalog(entry, mime_type, entries, explicit, next_request)
            }
            (
                PendingApplication::Launch {
                    request,
                    application_id,
                },
                DataProviderResult::ApplicationLaunched {
                    application_id: launched,
                    ..
                },
            ) if request == response.request && application_id == *launched => {
                self.emit_full().map(|message| vec![message])
            }
            (_, DataProviderResult::Unavailable { reason }) => {
                if let Some(state) = &mut self.open_with { state.loading = false; }
                self.emit_notice(reason)
            },
            _ => Err(FilesActorError::UnexpectedDataProviderResponse),
        }
    }

    fn accept_catalog(
        &mut self,
        entry: EntryMetadata,
        mime_type: &str,
        entries: &[InstalledApplication],
        explicit: bool,
        next_request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        if !explicit && !entry.path.as_str().to_ascii_lowercase().ends_with(".theme") {
            // A saved choice takes precedence; otherwise use an installed
            // handler for the known type. Unknown types still need a chooser.
            let default = entries.iter().find(|application| application.is_default)
                .or_else(|| entries.iter().find(|application| application.matches_requested_type));
            if let Some(application) = default {
                return self.launch_application(
                    application.id.clone(),
                    entry,
                    mime_type.to_owned(),
                    false,
                    next_request,
                );
            }
        }
        let candidates = entries
            .iter()
            .take(rice_native_app_protocol::MAX_APPLICATIONS)
            .cloned()
            .collect();
        if explicit && let Some(state) = &mut self.open_with {
            state.candidates = candidates;
            state.loading = false;
            return self.emit_full().map(|message| vec![message]);
        }
        self.open_with = Some(OpenWithState {
            entry,
            mime_type: mime_type.to_owned(),
            candidates,
            loading: false,
            remember_default: false,
            query: String::new(),
            command: String::new(),
            scroll: 0,
            focus: OpenWithFocus::Search,
        });
        self.emit_full().map(|message| vec![message])
    }

    pub(super) fn launch_application(
        &mut self,
        application_id: String,
        entry: EntryMetadata,
        mime_type: String,
        remember_default: bool,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        self.pending_application = Some(PendingApplication::Launch {
            request,
            application_id: application_id.clone(),
        });
        Ok(vec![Message::DataProviderRequest(DataProviderRequest {
            request,
            app: self.app,
            operation: DataProviderOperation::ApplicationLaunch {
                application_id,
                mime_type,
                paths: vec![entry.path.as_str().to_owned()],
                remember_default,
            },
        })])
    }
}

pub(super) fn dialog_patches(patches: &mut Vec<Patch>, state: &OpenWithState) {
    if state.entry.path.as_str().to_ascii_lowercase().ends_with(".theme") {
        create(patches, OPEN_WITH_DIALOG, Some(super::ROOT), WidgetKind::Dialog,
            Some(&rice_i18n::format("Open theme “{name}”", &[("name", &state.entry.name)])), Some(IconId::Application));
        create(patches, OPEN_WITH_CONTENT, Some(OPEN_WITH_DIALOG), WidgetKind::Column, None, None);
        create(patches, OPEN_WITH_LIST_LABEL, Some(OPEN_WITH_CONTENT), WidgetKind::Label,
            Some(rice_i18n::t("Load this theme on your desktop, or edit its contents in Paper?")), None);
        create(patches, OPEN_WITH_ACTIONS, Some(OPEN_WITH_DIALOG), WidgetKind::Row, Some("panel:actions"), None);
        for (id, label) in [(OPEN_THEME_LOAD, "Load theme"), (OPEN_THEME_EDIT, "Edit in Paper"), (OPEN_WITH_CANCEL, "Cancel")] {
            create_with_state(patches, id, Some(OPEN_WITH_ACTIONS), WidgetKind::Button, Some(rice_i18n::t(label)), None,
                if id == OPEN_WITH_CANCEL || !state.loading { WidgetState::ENABLED } else { WidgetState::empty() });
        }
        return;
    }
    let candidates = state.filtered_candidates();
    let has_recommended = candidates
        .iter()
        .any(|application| application.matches_requested_type);
    let has_other = candidates
        .iter()
        .any(|application| !application.matches_requested_type);
    let section_count = usize::from(has_recommended) + usize::from(has_other);
    let total_height = u32::try_from(candidates.len().saturating_add(section_count))
        .unwrap_or(u32::MAX)
        .saturating_mul(OPEN_WITH_ROW_HEIGHT);
    create(
        patches,
        OPEN_WITH_DIALOG,
        Some(super::ROOT),
        WidgetKind::Dialog,
        Some(&rice_i18n::format("Open “{name}” With", &[("name", &state.entry.name)])),
        Some(IconId::Application),
    );
    create(
        patches,
        OPEN_WITH_CONTENT,
        Some(OPEN_WITH_DIALOG),
        WidgetKind::Column,
        None,
        None,
    );
    create_with_state(
        patches,
        OPEN_WITH_SEARCH,
        Some(OPEN_WITH_CONTENT),
        WidgetKind::TextField,
        Some(&state.query),
        Some(IconId::Search),
        WidgetState::ENABLED
            | if state.focus == OpenWithFocus::Search {
                WidgetState::FOCUSED
            } else {
                WidgetState::empty()
            },
    );
    create(
        patches,
        OPEN_WITH_LIST_LABEL,
        Some(OPEN_WITH_CONTENT),
        WidgetKind::Label,
        Some(rice_i18n::t("Applications")),
        None,
    );
    create(
        patches,
        OPEN_WITH_LIST,
        Some(OPEN_WITH_CONTENT),
        WidgetKind::ScrollView,
        Some(&format!(
            "viewport:{OPEN_WITH_VIEWPORT_HEIGHT}|{}/{total_height}",
            state.scroll
        )),
        None,
    );
    create(
        patches,
        OPEN_WITH_LIST_CONTENT,
        Some(OPEN_WITH_LIST),
        WidgetKind::List,
        None,
        None,
    );
    create(
        patches,
        OPEN_WITH_ACTIONS,
        Some(OPEN_WITH_DIALOG),
        WidgetKind::Row,
        Some("panel:actions"),
        None,
    );
    if candidates.is_empty() {
        create_with_state(
            patches,
            OPEN_WITH_EMPTY,
            Some(OPEN_WITH_LIST_CONTENT),
            WidgetKind::Label,
            Some(rice_i18n::t(if state.loading { "Loading applications…" } else { "No applications match this search" })),
            Some(IconId::Information),
            WidgetState::ENABLED | WidgetState::MUTED,
        );
    }
    for (recommended, present, label_id, label) in [
        (
            true,
            has_recommended,
            OPEN_WITH_RECOMMENDED_LABEL,
            "Recommended",
        ),
        (
            false,
            has_other,
            OPEN_WITH_OTHER_LABEL,
            "Other applications",
        ),
    ] {
        if present {
            create_with_state(
                patches,
                label_id,
                Some(OPEN_WITH_LIST_CONTENT),
                WidgetKind::Label,
                Some(rice_i18n::t(label)),
                None,
                WidgetState::ENABLED | WidgetState::EMPHASIZED,
            );
        }
        for (index, application) in candidates
            .iter()
            .enumerate()
            .filter(|(_, application)| application.matches_requested_type == recommended)
        {
            let Ok(index) = u64::try_from(index) else {
                break;
            };
            application_row(patches, index, application);
        }
    }
    create_with_state(
        patches,
        OPEN_WITH_REMEMBER,
        Some(OPEN_WITH_CONTENT),
        WidgetKind::CheckBox,
        Some(rice_i18n::t("Always use for this file type")),
        None,
        WidgetState::ENABLED
            | if state.remember_default {
                WidgetState::CHECKED
            } else {
                WidgetState::empty()
            },
    );
    create(
        patches,
        OPEN_WITH_COMMAND_LABEL,
        Some(OPEN_WITH_CONTENT),
        WidgetKind::Label,
        Some(rice_i18n::t("Run a custom Bash command")),
        Some(IconId::Terminal),
    );
    create_with_state(
        patches,
        OPEN_WITH_COMMAND_HELP,
        Some(OPEN_WITH_CONTENT),
        WidgetKind::Label,
        Some(rice_i18n::t("The selected filename is appended as a safely quoted argument.")),
        None,
        WidgetState::ENABLED | WidgetState::MUTED,
    );
    create_with_state(
        patches,
        OPEN_WITH_COMMAND,
        Some(OPEN_WITH_CONTENT),
        WidgetKind::TextField,
        Some(&state.command),
        Some(IconId::Terminal),
        WidgetState::ENABLED
            | if state.focus == OpenWithFocus::Command {
                WidgetState::FOCUSED
            } else {
                WidgetState::empty()
            },
    );
    create(
        patches,
        OPEN_WITH_RUN_COMMAND,
        Some(OPEN_WITH_ACTIONS),
        WidgetKind::Button,
        Some(rice_i18n::t("Run Command")),
        Some(IconId::Terminal),
    );
    create(
        patches,
        OPEN_WITH_CANCEL,
        Some(OPEN_WITH_ACTIONS),
        WidgetKind::Button,
        Some(rice_i18n::t("Cancel")),
        Some(IconId::Close),
    );
}

fn application_row(patches: &mut Vec<Patch>, index: u64, application: &InstalledApplication) {
    create_with_state(
        patches,
        OPEN_WITH_APP_BASE + index,
        Some(OPEN_WITH_LIST_CONTENT),
        WidgetKind::Button,
        Some(&application.name),
        Some(application_icon(application)),
        WidgetState::ENABLED
            | WidgetState::START_ALIGNED
            | if application.is_default {
                WidgetState::SELECTED
            } else {
                WidgetState::empty()
            },
    );
}

pub(super) fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn is_executable_program(entry: &EntryMetadata) -> bool {
    entry.kind == EntryKind::File
        && entry.mode & 0o111 != 0
        && matches!(
            entry.name.rsplit_once('.').map(|(_, extension)| extension),
            Some(extension) if extension.eq_ignore_ascii_case("sh")
                || extension.eq_ignore_ascii_case("bin")
                || extension.eq_ignore_ascii_case("run")
        )
}

fn application_icon(application: &InstalledApplication) -> IconId {
    match application.id.as_str() {
        "native:rice-paper" => return IconId::TextEditor,
        "native:rice-files" => return IconId::Files,
        "native:rice-view" => return IconId::ImageViewer,
        "native:rice-calc" => return IconId::Calculator,
        "native:rice-settings" => return IconId::Settings,
        "native:rice-term" => return IconId::Terminal,
        "native:rice-player" => return IconId::MediaPlay,
        _ => {}
    }
    let icon = application.icon.to_ascii_lowercase();
    if icon.contains("terminal") {
        IconId::Terminal
    } else if icon.contains("calculator") || icon.contains("calc") {
        IconId::Calculator
    } else if icon.contains("text") || icon.contains("editor") {
        IconId::TextEditor
    } else if icon.contains("image") || icon.contains("photo") {
        IconId::ImageViewer
    } else if icon.contains("folder") || icon.contains("file") {
        IconId::Files
    } else if icon.contains("settings") || icon.contains("control") {
        IconId::Settings
    } else if icon.contains("video") || icon.contains("media") || icon.contains("player") {
        IconId::MediaPlay
    } else {
        IconId::Application
    }
}

#[cfg(test)]
mod translation_tests {
    use super::*;
    #[test]
    fn chooser_localizes_titles_and_empty_states_without_touching_inputs() {
        let state=OpenWithState {
            entry:EntryMetadata::new(rice_files::GuestPath::absolute("/root/Save {name}.txt").unwrap(),EntryKind::File,0,0),
            mime_type:"text/plain".into(),candidates:Vec::new(),loading:false,remember_default:false,
            query:"Files".into(),command:"printf '%s'".into(),scroll:0,focus:OpenWithFocus::Search,
        };
        rice_i18n::with_locale(2, || {
            let mut output=Vec::new();dialog_patches(&mut output,&state);
            let text=|id| output.iter().find_map(|patch| match patch {
                Patch::Create {widget,..} if widget.id.get()==id => widget.text.as_deref(),
                _=>None,
            });
            assert_eq!(text(OPEN_WITH_DIALOG),Some("选择打开“Save {name}.txt”的方式"));
            assert_eq!(text(OPEN_WITH_EMPTY),Some("没有与此搜索匹配的应用"));
            assert_eq!(text(OPEN_WITH_SEARCH),Some("Files"));
            assert_eq!(text(OPEN_WITH_COMMAND),Some("printf '%s'"));
            assert_eq!(state.entry.path.as_str(),"/root/Save {name}.txt");
            assert_eq!(state.mime_type,"text/plain");
        });
    }
}
