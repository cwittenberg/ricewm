use super::*;
use super::super::tests::{loaded_actor, event, request};

#[test]
fn app_actions_remain_reachable_after_one_hundred_saved_user_actions() {
    let (mut actor, _) = loaded_actor();
    actor.user_actions = (0..100).map(|index| FileAction {
        id: format!("user.{index}"), title: format!("User {index}"),
        action_type: "script".into(), context: "any".into(),
    }).collect();
    actor.user_actions.extend(decode(r#"[{"id":"user.app-proof","title":"Inspect selected file","type":"app-action","fileContext":"any"}]"#));
    actor.handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(40)).unwrap();
    let patches = actor.full_patches().unwrap();
    assert!(patches.iter().any(|patch| matches!(patch, Patch::Create {widget, ..}
        if widget.id == files_view::widget_id(350_000) && widget.text.as_deref() == Some("Inspect selected file"))));
    let result = actor.handle_event(&event(350_000, UiEvent::Activate), request(41)).unwrap();
    assert!(matches!(&result[0], Message::DataProviderRequest(DataProviderRequest {
        operation: DataProviderOperation::ApplicationLaunch {application_id, remember_default:false, ..}, ..
    }) if application_id == "user.app-proof"));
}
