//! Shared text-field context menu. Clipboard access stays in the host bridge.
use super::*;

pub(super) const BAR: u64 = 0x7fff_ff00;
pub(super) const MENU: u64 = BAR + 1;
pub(super) const FIRST: u64 = BAR + 2;
pub(super) const LAST: u64 = BAR + 5;
fn id(value: u64) -> WidgetId {
    WidgetId::new(value).expect("text menu ID")
}

pub(super) fn command(target: WidgetId) -> bool {
    (FIRST..=LAST).contains(&target.get())
}

pub(super) fn restore_focus(window: &mut NativeWindow, target: WidgetId, event: &UiEvent) {
    if command(target) && matches!(event, UiEvent::Activate) {
        window.focused = window.text_edit.map(|edit| edit.widget);
        window.opened_menu = None;
        window.opened_submenus.clear();
        window.popup_anchor = None;
    }
}

pub(super) fn open(
    window: &mut NativeWindow,
    target: WidgetId,
    point: Point,
) -> Result<(), NativeUiError> {
    let Some(field) = window.tree.get(target) else {
        return Ok(());
    };
    let password = field.kind == WidgetKind::PasswordField;
    let selected = window
        .text_edit
        .is_some_and(|edit| edit.widget == target && edit.anchor != edit.caret);
    let mut patches = Vec::new();
    if window.tree.get(id(BAR)).is_some() {
        patches.push(Patch::Remove { id: id(BAR) });
    }
    let items = [
        (
            BAR,
            Some(target),
            WidgetKind::MenuBar,
            "context".to_owned(),
            true,
        ),
        (
            MENU,
            Some(id(BAR)),
            WidgetKind::Menu,
            "Text".to_owned(),
            true,
        ),
        (
            FIRST,
            Some(id(MENU)),
            WidgetKind::MenuItem,
            rice_ui::menu_item("Cut", Some("Ctrl+X")),
            selected && !password,
        ),
        (
            FIRST + 1,
            Some(id(MENU)),
            WidgetKind::MenuItem,
            rice_ui::menu_item("Copy", Some("Ctrl+C")),
            selected && !password,
        ),
        (
            FIRST + 2,
            Some(id(MENU)),
            WidgetKind::MenuItem,
            rice_ui::menu_item("Paste", Some("Ctrl+V")),
            true,
        ),
        (
            FIRST + 3,
            Some(id(MENU)),
            WidgetKind::MenuItem,
            rice_ui::menu_item("Select All", Some("Ctrl+A")),
            true,
        ),
    ];
    for (value, parent, kind, text, enabled) in items {
        patches.push(Patch::Create {
            widget: rice_ui::WidgetSpec {
                id: id(value),
                kind,
                state: if enabled {
                    WidgetState::ENABLED
                } else {
                    WidgetState::empty()
                },
                text: Some(text),
                icon: None,
            },
            parent,
            index: if value >= FIRST {
                (value - FIRST) as usize
            } else {
                0
            },
        });
    }
    window.tree.apply(&patches)?;
    window.opened_menu = Some(id(MENU));
    window.opened_submenus.clear();
    window.popup_anchor = Some(point);
    window.focused = Some(target);
    window.geometry.clear();
    window.hit_targets.clear();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_context_menu_preserves_selection_and_restores_field_for_clipboard() {
        let app = AppId::new(1).unwrap();
        let window = WindowId::new(1).unwrap();
        let field = id(2);
        let mut host = NativeUiHost::new(Theme::ricewm_default());
        let mut state = NativeWindow::default();
        state
            .tree
            .apply(&[Patch::Create {
                widget: rice_ui::WidgetSpec {
                    id: field,
                    kind: WidgetKind::TextField,
                    state: WidgetState::ENABLED,
                    text: Some("/root/Documents".into()),
                    icon: None,
                },
                parent: None,
                index: 0,
            }])
            .unwrap();
        state.focused = Some(field);
        state.text_edit = Some(TextEditState {
            widget: field,
            anchor: 6,
            caret: 15,
        });
        host.windows.insert((app, window), state);
        let constraints = Constraints::new(
            Size {
                width: 800,
                height: 600,
            },
            Size {
                width: 800,
                height: 600,
            },
        )
        .unwrap();
        host.render(app, window, constraints).unwrap();
        let point = Point { x: 30, y: 10 };
        host.pointer_event(app, window, PointerEventKind::Down, point, 3, 0)
            .unwrap();
        host.pointer_event(app, window, PointerEventKind::Up, point, 3, 0)
            .unwrap();
        assert_eq!(
            host.selected_text(app, window).as_deref(),
            Some("Documents")
        );
        let rendered = host.render(app, window, constraints).unwrap();
        assert!(
            rendered
                .commands
                .iter()
                .any(|command| matches!(command,DrawCommand::Text {text,..} if text=="Copy"))
        );
        let copy = host.windows[&(app, window)]
            .hit_targets
            .iter()
            .find(|hit| hit.widget == id(FIRST + 1))
            .unwrap()
            .rect
            .origin;
        let copy = Point {
            x: copy.x + 3,
            y: copy.y + 3,
        };
        host.pointer_event(app, window, PointerEventKind::Down, copy, 1, 0)
            .unwrap();
        let event = host
            .pointer_event(app, window, PointerEventKind::Up, copy, 1, 0)
            .unwrap()
            .unwrap();
        assert_eq!(event.target, id(FIRST + 1));
        assert_eq!(event.event, UiEvent::Activate);
        assert_eq!(
            host.selected_text(app, window).as_deref(),
            Some("Documents")
        );
        assert!(host.windows[&(app, window)].opened_menu.is_none());
        // Text menus must never become the file/background menu.
        assert_eq!(
            input::contextual_menu(&host.windows[&(app, window)].tree, None),
            None
        );
    }
}
