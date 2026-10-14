//! Opt-in saved scripts in the selection context menu. The host owns execution.
use super::*;
use rice_ui::{WidgetState, IconId};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub(super) struct FileAction {
    id: String,
    title: String,
    #[serde(rename = "type")]
    action_type: String,
    #[serde(rename = "fileContext")]
    context: String,
}

pub(super) fn decode(raw: &str) -> Vec<FileAction> {
    if raw.len() > 64_000 { return Vec::new(); }
    serde_json::from_str::<Vec<serde_json::Value>>(raw).unwrap_or_default()
        .into_iter().take(100).filter_map(|value| serde_json::from_value::<FileAction>(value).ok())
        .filter(|action| action.id.starts_with("user.") && action.title.len() <= 320
            && !action.title.trim().is_empty()
            && matches!(action.action_type.as_str(), "script" | "script-file" | "app-action")
            && matches!(action.context.as_str(), "files" | "folders" | "any"))
        .collect()
}

fn matches_selection(action: &FileAction, selection: &[EntryMetadata]) -> bool {
    !selection.is_empty() && selection.len() <= 128 && selection.iter().all(|entry| match action.context.as_str() {
        "files" => entry.kind == EntryKind::File,
        "folders" => entry.kind == EntryKind::Directory,
        "any" => true,
        _ => false,
    })
}

pub(super) fn menu_patches(actor: &RiceFiles, patches: &mut Vec<Patch>) {
    let selection = actor.selected_entries();
    let actions: Vec<_> = actor.user_actions.iter().enumerate()
        .filter(|(_, action)| matches_selection(action, &selection)).collect();
    if actions.is_empty() { return; }
    let Some(position) = patches.iter().position(|patch| matches!(patch,
        Patch::Create { widget, .. } if widget.id == files_view::widget_id(files_menu::CONTEXT_MENU))) else { return; };
    let mut custom = Vec::new();
    files_view::create_with_state(&mut custom, 340_101, Some(files_menu::CONTEXT_MENU),
        WidgetKind::MenuItem, Some(rice_i18n::t("My actions")), Some(IconId::Terminal),
        WidgetState::ENABLED | WidgetState::TRAILING_ICON);
    files_view::create_with_state(&mut custom, 340_102, Some(340_101),
        WidgetKind::Menu, Some(rice_i18n::t("My actions")), None, WidgetState::ENABLED);
    // Small submenus remain reachable even with many saved actions.
    for (page, chunk) in actions.chunks(10).enumerate() {
        let parent = if actions.len() > 10 {
            let item = 340_200 + page as u64 * 2;
            let label = rice_i18n::format("Actions {first}–{last}", &[("first", &(page * 10 + 1).to_string()), ("last", &(page * 10 + chunk.len()).to_string())]);
            files_view::create_with_state(&mut custom, item, Some(340_102), WidgetKind::MenuItem,
                Some(&label), None, WidgetState::ENABLED | WidgetState::TRAILING_ICON);
            files_view::create_with_state(&mut custom, item + 1, Some(item), WidgetKind::Menu,
                Some(&label), None, WidgetState::ENABLED);
            item + 1
        } else { 340_102 };
        for (index, action) in chunk {
            let widget = if *index < 100 { 340_000 + *index as u64 } else { 350_000 + (*index - 100) as u64 };
            files_view::create_with_state(&mut custom, widget,
                Some(parent), WidgetKind::MenuItem, Some(&action.title),
                Some(IconId::Terminal), WidgetState::ENABLED);
        }
    }
    // Insert ahead of the long conventional file menu, never below the screen.
    patches.splice(position + 1..position + 1, custom);
    patches.push(Patch::Move { id: files_view::widget_id(340_101),
        parent: Some(files_view::widget_id(files_menu::CONTEXT_MENU)), index: 0 });
}

pub(super) fn execute(actor: &mut RiceFiles, index: usize, request: RequestId) -> Result<Vec<Message>, FilesActorError> {
    let selection = actor.selected_entries();
    let Some(action) = actor.user_actions.get(index).filter(|action| matches_selection(action, &selection)) else {
        return actor.emit_notice(rice_i18n::t("This action is not available for the current selection"));
    };
    let application_id = action.id.clone();
    actor.pending_application = Some(PendingApplication::Launch { request, application_id: application_id.clone() });
    Ok(vec![Message::DataProviderRequest(DataProviderRequest { request, app: actor.app,
        operation: DataProviderOperation::ApplicationLaunch {
            application_id, mime_type: "application/x-rice-file-action".into(),
            paths: selection.iter().map(|entry| entry.path.as_str().to_owned()).collect(),
            remember_default: false,
        },
    })])
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::tests::{loaded_actor, event, request};
    #[test]
    fn custom_action_pages_translate_but_user_titles_and_launch_ids_do_not() {
        let (mut actor, _) = loaded_actor();
        actor.user_actions = (0..12).map(|index| FileAction {
            id: format!("user.{index}"), title: "My actions {first}".into(),
            action_type: "script".into(), context: "any".into(),
        }).collect();
        actor.handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(40)).unwrap();
        let selected_paths = actor.selected_entries().iter().map(|entry| entry.path.as_str().to_owned()).collect::<Vec<_>>();
        for (locale, heading, first, last) in [(1, "Mis acciones", "Acciones 1–10", "Acciones 11–12"),
                                              (2, "我的操作", "操作 1–10", "操作 11–12")] {
            rice_i18n::with_locale(locale, || {
                let patches = actor.full_patches().unwrap();
                for expected in [heading, first, last, "My actions {first}"] {
                    assert!(patches.iter().any(|patch| matches!(patch, Patch::Create {widget, ..} if widget.text.as_deref() == Some(expected))), "missing {expected}");
                }
                let result = execute(&mut actor, 11, request(41)).unwrap();
                assert!(matches!(&result[0], Message::DataProviderRequest(DataProviderRequest {
                    operation: DataProviderOperation::ApplicationLaunch {application_id, paths, mime_type, ..}, ..
                }) if application_id == "user.11" && paths == &selected_paths && mime_type == "application/x-rice-file-action"));
            });
        }
    }

    #[test]
    fn context_actions_filter_selection_and_launch_one_request_with_all_paths() {
        let (mut actor, _) = loaded_actor();
        actor.user_actions = decode(r#"[{"id":"user.files","title":"File script","type":"script","fileContext":"files"},{"id":"user.folders","title":"Folder script","type":"script-file","fileContext":"folders"},{"id":"user.all","title":"Selection script","type":"script","fileContext":"any"},{"id":"user.hidden","title":"Hidden","type":"script","fileContext":"none"}]"#);
        assert_eq!(actor.user_actions.len(), 3);
        let mut patches = Vec::new();menu_patches(&actor, &mut patches);assert!(patches.is_empty());
        actor.handle_event(&event(ENTRY_BASE, UiEvent::Activate),request(40)).unwrap();
        let selection = actor.selected_entries();assert_eq!(selection.len(),1);
        assert_eq!(actor.tree.get(files_view::widget_id(files_menu::CONTEXT_MENU)).unwrap().children()[0], files_view::widget_id(340_101));
        assert_eq!(actor.tree.get(files_view::widget_id(340_102)).unwrap().children().len(), 2);

        assert!(matches_selection(&actor.user_actions[1],&selection));
        assert!(!matches_selection(&actor.user_actions[0],&selection));
        let result=execute(&mut actor,2,request(41)).unwrap();
        assert!(matches!(&result[0],Message::DataProviderRequest(DataProviderRequest { operation: DataProviderOperation::ApplicationLaunch { application_id, paths, remember_default:false, .. }, .. }) if application_id=="user.all" && paths==&vec![selection[0].path.as_str().to_owned()]));
        actor.user_actions.clear();
        assert!(!execute(&mut actor,2,request(42)).unwrap().iter().any(|m|matches!(m,Message::DataProviderRequest(_))));
    }
}

#[cfg(test)]
#[path = "../../../../tests/files/app-actions.rs"]
mod app_action_tests;
