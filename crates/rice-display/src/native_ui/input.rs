use super::*;
use rice_ui::{EditorText, EditorWrap, Theme, Widget, WidgetState};

const MAX_OPEN_SUBMENUS: usize = 4;
// The browser uses this internal modifier only for native text-editor word
// selection. Window/content double-click activation arrives as semantic
// button 6 from the DOM `dblclick` handler and must not be synthesized here.
const DOUBLE_CLICK_MODIFIER: u16 = 0x8000;

// Section tracking is opt-in; ordinary scroll consumers keep their existing
// position/maximum wire value. Resolve against layout, including wrapped text
// and expanded accordions, instead of application-authored pixel thresholds.
fn document_scroll_value(
    native_window: &NativeWindow,
    scroll: WidgetId,
    position: u32,
    maximum: u32,
    theme: &Theme,
) -> String {
    let plain = format!("{position}/{maximum}");
    let Some(ids) = native_window.tree.get(scroll).and_then(|widget| widget.text.as_deref())
        .and_then(|text| text.strip_prefix("sections:"))
        .and_then(|text| text.split_once('|').map(|(ids, _)| ids)) else { return plain; };
    let Some(viewport) = super::layout::scroll_view_geometry(
        &native_window.tree, scroll, &native_window.geometry, theme,
    ) else { return plain; };
    let mut active = None;
    for id in ids.split(',').take(64).filter_map(|id| id.parse::<u64>().ok()).filter_map(WidgetId::new) {
        if enclosing_scroll_view(&native_window.tree, id) != Some(scroll) { continue; }
        let Some(rect) = native_window.geometry.get(&id) else { continue; };
        let top = i64::from(rect.origin.y) - i64::from(viewport.track.origin.y) + i64::from(viewport.position);
        if active.is_none() || top <= i64::from(position) + 32 || position == maximum {
            active = Some(id);
        }
    }
    active.map_or(plain.clone(), |id| format!("{plain};section:{}", id.get()))
}

impl NativeUiHost {
    /// Read supported widget operations without focusing or activating it.
    /// Bits: 1 semantic activation, 2 editable text, 4 pointer/keyboard focus.
    pub fn widget_actions(&self, app: AppId, window: WindowId, target: WidgetId) -> u32 {
        let Some(native_window)=self.windows.get(&(app,window)) else { return 0; };
        if native_window.grid_resync_shadow.is_some() { return 0; }
        let Some(widget)=native_window.tree.get(target) else { return 0; };
        let enabled=widget.state.contains(WidgetState::ENABLED);
        let (focusable,activatable)=super::render::widget_interaction(widget.kind,enabled);
        let editable=enabled && match widget.kind {
            WidgetKind::TextField|WidgetKind::PasswordField => true,
            WidgetKind::TextEditor => true,
            _ => false,
        };
        u32::from(activatable || (enabled && native_window.navigation_headers.contains_key(&target)))
            | (u32::from(editable)<<1) | (u32::from(focusable)<<2)
    }

    /// Discover the native overflowing ScrollView that would receive a wheel
    /// at this rendered point. Read-only, window-owned and modal-aware.
    pub fn scroll_owner_at_point(&self, app: AppId, window: WindowId, point: Point) -> Option<WidgetId> {
        let native_window = self.windows.get(&(app, window))?;
        if native_window.grid_resync_shadow.is_some() || native_window.opened_menu.is_some() { return None; }
        let target = self.hit_test(app, window, point);
        if let Some(dialog) = super::layout_overlay::dialog_widgets(&native_window.tree).last().copied() {
            if !target.is_some_and(|target| super::layout_overlay::widget_belongs_to(&native_window.tree, target.widget, dialog)) { return None; }
        }
        if target.and_then(|target| native_window.tree.get(target.widget)).is_some_and(|widget| {
            let document = widget.kind == WidgetKind::WebSurface
                && widget.text.as_deref().is_some_and(|text| text.starts_with("panel:document-height:"));
            !document && widget.state.contains(WidgetState::ENABLED) && matches!(widget.kind,
                WidgetKind::WebSurface | WidgetKind::SceneSurface | WidgetKind::HostSurface
                | WidgetKind::TerminalSurface | WidgetKind::TerminalGrid | WidgetKind::TerminalRow)
        }) { return None; }
        let scroll = target.and_then(|target| enclosing_scroll_view(&native_window.tree, target.widget))
            .or_else(|| scroll_view_at_point(&native_window.tree, &native_window.geometry, point))?;
        let theme = effective_window_theme(&native_window.tree, self.theme);
        let geometry = super::layout::scroll_view_geometry(&native_window.tree, scroll, &native_window.geometry, &theme)?;
        (geometry.content_extent > geometry.viewport_extent).then_some(scroll)
    }

    /// Creates an activation event for an exact enabled semantic widget.
    ///
    /// This is the non-visual accessibility/automation equivalent of a
    /// primary-button release. It deliberately shares the retained widget
    /// validation used by keyboard activation and never depends on geometry.
    pub fn activate_event(
        &mut self,
        app: AppId,
        window: WindowId,
        target: WidgetId,
    ) -> Result<Option<UiEventEnvelope>, NativeUiError> {
        let native_window = self
            .windows
            .get_mut(&(app, window))
            .ok_or(NativeUiError::UnknownWindow { app, window })?;
        if native_window.grid_resync_shadow.is_some() { return Ok(None); }
        if native_window.navigation_headers.contains_key(&target) {
            native_window.navigation_open = if native_window.navigation_open == Some(target) {None} else {Some(target)};
            native_window.navigation_offset = 0;
            return Ok(None);
        }
        if native_window.navigation_open.is_some_and(|sidebar| super::layout_overlay::widget_belongs_to(&native_window.tree, target, sidebar)) {
            native_window.navigation_open = None;
        }

        let widget = native_window
            .tree
            .get(target)
            .ok_or(NativeUiError::UnknownWidget(target))?;
        let (_, activatable) = super::render::widget_interaction(
            widget.kind,
            widget.state.contains(WidgetState::ENABLED),
        );
        Ok(activatable.then_some(UiEventEnvelope {
            app,
            window,
            target,
            event: UiEvent::Activate,
        }))
    }

    /// Maps pointer input to a semantic widget event after hit testing.
    ///
    /// A primary-button down moves host focus. A primary-button up activates an
    /// activatable target; all other pointer input remains a semantic pointer
    /// event for the hit widget.
    ///
    /// # Errors
    ///
    /// Returns [`NativeUiError::UnknownWindow`] when the target window is absent.
    pub fn pointer_event(
        &mut self,
        app: AppId,
        window: WindowId,
        kind: PointerEventKind,
        point: Point,
        button: u8,
        modifiers: u16,
    ) -> Result<Option<UiEventEnvelope>, NativeUiError> {
        self.pointer_event_measured(app, window, kind, point, button, modifiers, None)
    }

    pub(crate) fn pointer_event_measured(
        &mut self, app: AppId, window: WindowId, kind: PointerEventKind,
        point: Point, button: u8, modifiers: u16,
        measured: Option<&super::editor_measurement::EditorMeasurement>,
    ) -> Result<Option<UiEventEnvelope>, NativeUiError> {
        if measured.is_some_and(|sample| !self.validate_editor_measurement(app, window, kind, point, sample)) {
            return Err(NativeUiError::InvalidPresentation("stale editor measurement"));
        }
        let measured_caret = measured.map(|sample| sample.caret);
        if self.windows.get(&(app, window)).is_some_and(|value| value.grid_resync_shadow.is_some()) {
            // The SDK may already hold newer geometry. Do not interpret input
            // against an intentionally retained older view while recovering.
            return Ok(None);
        }
        let theme = self
            .windows
            .get(&(app, window))
            .map_or(self.theme, |native_window| {
                super::effective_window_theme(&native_window.tree, self.theme)
            });
        // A pointer can leave the browser canvas before its button-up reaches
        // us. Never let that stale capture turn a later click or ordinary
        // hover into an unexpected splitter jump.
        if matches!(kind, PointerEventKind::Down | PointerEventKind::Leave) {
            let native_window = self
                .windows
                .get_mut(&(app, window))
                .ok_or(NativeUiError::UnknownWindow { app, window })?;
            native_window.scroll_drag = None;
            native_window.text_drag = None;
        }
        let captured = self
            .windows
            .get(&(app, window))
            .and_then(|native_window| {
                native_window
                    .scroll_drag
                    .or_else(|| native_window.text_drag.map(|drag| drag.widget))
            })
            .and_then(|widget| {
                self.windows
                    .get(&(app, window))?
                    .hit_targets
                    .iter()
                    .find(|target| target.widget == widget)
                    .copied()
            });
        let target = captured.or_else(|| self.hit_test(app, window, point));
        let native_window = self
            .windows
            .get_mut(&(app, window))
            .ok_or(NativeUiError::UnknownWindow { app, window })?;
        // Help text may own hover without stealing its form row's click.
        let target = target.map(|target| {
            if target.activatable || !matches!(kind, PointerEventKind::Down | PointerEventKind::Up)
                && !(kind == PointerEventKind::Move && native_window.pressed.is_some()) {
                return target;
            }
            let mut ancestor = native_window.tree.get(target.widget)
                .filter(|widget| widget.kind == WidgetKind::Label)
                .and_then(|widget| widget.parent);
            while let Some(id) = ancestor {
                let Some(widget) = native_window.tree.get(id) else { break; };
                if let Some(control) = super::layout_tokens::control_for(widget.text.as_deref())
                    .and_then(WidgetId::new)
                {
                    return native_window.hit_targets.iter()
                        .find(|hit| hit.widget == control && hit.activatable && hit.rect.contains(point))
                        .copied().unwrap_or(target);
                }
                ancestor = widget.parent;
            }
            target
        });
        let previously_focused = native_window.focused;
        let previously_hovered = native_window.hovered;
        let previously_pressed = native_window.pressed;
        if let Some(sidebar) = native_window.navigation_open {
            if kind == PointerEventKind::Down && button == 1
                && native_window.geometry.get(&sidebar).is_some_and(|rect| rect.contains(point))
            {
                native_window.navigation_drag = Some((point, native_window.navigation_offset));
                native_window.navigation_scrolled = false;
            }
            if let Some((origin, offset)) = native_window.navigation_drag {
                if kind == PointerEventKind::Move && (point.y - origin.y).unsigned_abs() > 8 && native_window.navigation_maximum > 0 {
                    native_window.navigation_offset = (i64::from(offset) + i64::from(origin.y) - i64::from(point.y)).clamp(0, i64::from(native_window.navigation_maximum)) as u32;
                    native_window.navigation_scrolled = true;
                    native_window.pressed = None;
                    return Ok(None);
                }
                if matches!(kind, PointerEventKind::Up | PointerEventKind::Leave) {
                    native_window.navigation_drag = None;
                    if native_window.navigation_scrolled { native_window.navigation_scrolled=false;return Ok(None); }
                }
            }
            if kind == PointerEventKind::Wheel && matches!(button,4|5) && native_window.geometry.get(&sidebar).is_some_and(|rect| rect.contains(point)) {
                native_window.navigation_offset = if button==4 {native_window.navigation_offset.saturating_sub(48)} else {native_window.navigation_offset.saturating_add(48).min(native_window.navigation_maximum)};
                return Ok(None);
            }
        }
        if let Some(target) = target.filter(|target| native_window.navigation_headers.get(&target.widget).is_some_and(|rect| rect.contains(point))) {
            if kind == PointerEventKind::Down && button == 1 {
                native_window.pressed = Some(target.widget);
                native_window.focused = Some(target.widget);
            } else if kind == PointerEventKind::Up && button == 1 && previously_pressed == Some(target.widget) {
                native_window.navigation_open = if native_window.navigation_open == Some(target.widget) { None } else { Some(target.widget) };
                native_window.navigation_offset = 0;
                native_window.pressed = None;
            }
            return Ok(None);
        }
        if kind == PointerEventKind::Up && button == 1
            && let (Some(sidebar), Some(target)) = (native_window.navigation_open, target)
            && previously_pressed == Some(target.widget)
            && target.activatable
            && super::layout_overlay::widget_belongs_to(&native_window.tree, target.widget, sidebar)
        { native_window.navigation_open = None; }
        if kind == PointerEventKind::Down && button == 1 {
            native_window.pressed = target
                .filter(|target| target.activatable)
                .map(|target| target.widget);
        } else if matches!(kind, PointerEventKind::Up | PointerEventKind::Leave) {
            native_window.pressed = None;
        } else if kind == PointerEventKind::Move && native_window.pressed.is_some() {
            // A held button stays depressed only while the pointer remains
            // over the original command, matching conventional desktop UI.
            native_window.pressed = native_window
                .pressed
                .filter(|pressed| target.is_some_and(|target| target.widget == *pressed));
        }
        if kind == PointerEventKind::Down && button == 1 {
            native_window.text_selection_all = None;
        }
        native_window.hovered = if kind == PointerEventKind::Leave {
            None
        } else {
            target.map(|target| target.widget)
        };
        native_window.hover_point = if kind == PointerEventKind::Leave {
            None
        } else {
            Some(point)
        };
        let text_target = target.filter(|target| {
            native_window.tree.get(target.widget).is_some_and(|widget| {
                matches!(
                    widget.kind,
                    WidgetKind::TextField | WidgetKind::PasswordField
                ) && widget.state.contains(WidgetState::ENABLED)
            })
        });
        if button == 3
            && let Some(target) = text_target
        {
            if kind == PointerEventKind::Down {
                native_window.focused = Some(target.widget);
                if native_window
                    .text_edit
                    .is_none_or(|edit| edit.widget != target.widget)
                {
                    let widget = native_window.tree.get(target.widget).expect("text field");
                    let value = widget.text.as_deref().unwrap_or_default();
                    let caret = text_caret_from_pointer(
                        value,
                        target.rect,
                        point.x,
                        widget.icon.is_some(),
                        &theme, measured_caret,
                    );
                    native_window.text_edit = Some(TextEditState {
                        widget: target.widget,
                        anchor: caret,
                        caret,
                    });
                }
                return Ok(Some(UiEventEnvelope {
                    app,
                    window,
                    target: target.widget,
                    event: UiEvent::Focus { focused: true },
                }));
            }
            if kind == PointerEventKind::Up {
                text_menu::open(native_window, target.widget, point)?;
                return Ok(None);
            }
        }
        let context_request = kind == PointerEventKind::Up && button == 3;
        let context_menu = context_request
            .then(|| contextual_menu(&native_window.tree, target.map(|target| target.widget)))
            .flatten();
        if context_request { native_window.popup_context_target = target.map(|target| target.widget); }
        if let Some(menu) = context_menu {
            native_window.opened_menu = Some(menu);
            native_window.opened_submenus.clear();
            native_window.popup_anchor = Some(point);
            native_window.focused = menu_keyboard_items(&native_window.tree, menu).first().copied();
        } else if context_request {
            // The application may create its contextual menu in response to
            // this event. Retain the trusted pointer anchor for that one patch.
            native_window.popup_anchor = Some(point);
        } else if native_window.opened_menu.is_none() {
            native_window.popup_anchor = None;
        }
        let target_owns_wheel = target
            .and_then(|target| native_window.tree.get(target.widget))
            .is_some_and(|widget| {
                // Passive scene previews still contribute hit geometry, but
                // cannot consume input intended for their scrolling gallery.
                // Measured HTML documents expand inside the native scroll
                // view. Every message in a conversation shares that scroll;
                // only independently scrolling web surfaces own wheel input.
                let document = widget.kind == WidgetKind::WebSurface
                    && widget.text.as_deref().is_some_and(|text| text.starts_with("panel:document-height:"));
                !document && widget.state.contains(WidgetState::ENABLED) && matches!(
                    widget.kind,
                    WidgetKind::WebSurface | WidgetKind::SceneSurface
                        | WidgetKind::TerminalSurface
                        | WidgetKind::TerminalGrid
                        | WidgetKind::TerminalRow
                        | WidgetKind::HostSurface
                )
            });
        if kind == PointerEventKind::Wheel && matches!(button,4|5) && !target_owns_wheel
            && native_window.opened_menu.is_none()
            && let Some(viewport) = scroll_view_at_point(&native_window.tree,&native_window.geometry,point)
            && native_window.tree.get(viewport).and_then(|widget| widget.text.as_deref())
                .is_some_and(|text| text.split(':').any(|part| part=="document-viewport"))
        {
            return Ok(Some(UiEventEnvelope {app,window,target:viewport,event:UiEvent::Pointer {
                kind,x:point.x,y:point.y,button,modifiers,
            }}));
        }
        if kind == PointerEventKind::Wheel
            && matches!(button, 4 | 5)
            && let Some(menu) = native_window.opened_submenus.iter().rev().copied()
                .chain(native_window.opened_menu).find(|menu| {
                    menu_popup_rect(&native_window.tree, *menu, &native_window.geometry, &theme)
                        .is_some_and(|popup| popup.contains(point))
                })
        {
            let visible_rows = native_window.tree.get(menu).map_or(0, |menu| {
                super::layout::menu_content_children(&native_window.tree, menu)
                    .iter()
                    .filter(|child| native_window.geometry.contains_key(child))
                    .count()
            });
            let clipped_maximum = native_window
                .tree
                .get(menu)
                .map_or(0, |menu| super::layout::menu_content_children(&native_window.tree, menu).len().saturating_sub(visible_rows));
            let maximum = super::layout::maximum_menu_scroll_row(&native_window.tree, menu)
                .max(clipped_maximum);
            // Compound menus delegate scrolling to their retained ScrollView.
            // A zero-row menu must not swallow that wheel input merely because
            // the pointer is inside the popup background.
            if maximum > 0 {
                let current = native_window
                    .menu_scroll_rows
                    .get(&menu)
                    .copied()
                    .unwrap_or(0);
                let next = if button == 4 {
                    current.saturating_sub(1)
                } else {
                    current.saturating_add(1).min(maximum)
                };
                native_window.menu_scroll_rows.insert(menu, next);
                return Ok(None);
            }
        }
        let wheel_scroll_view = (kind == PointerEventKind::Wheel
            && matches!(button, 4 | 5)
            // Match the desktop/browser convention: Ctrl+wheel belongs to
            // the application as a zoom gesture, not to the enclosing native
            // scroll view. The original modifier bits are preserved on the
            // pointer event delivered below.
            && modifiers & 4 == 0
            && !target_owns_wheel)
            .then(|| {
                target
                    .and_then(|target| enclosing_scroll_view(&native_window.tree, target.widget))
                    .or_else(|| {
                        scroll_view_at_point(&native_window.tree, &native_window.geometry, point)
                    })
            })
            .flatten();
        if let Some(scroll_view) = wheel_scroll_view {
            let (position, maximum) = super::layout::scroll_view_geometry(
                &native_window.tree,
                scroll_view,
                &native_window.geometry,
                &theme,
            )
            .map_or((0, 0), |geometry| {
                let maximum = geometry
                    .content_extent
                    .saturating_sub(geometry.viewport_extent);
                let current = geometry.position.min(maximum);
                let position = if button == 4 {
                    current.saturating_sub(96)
                } else {
                    current.saturating_add(96).min(maximum)
                };
                (position, maximum)
            });
            if maximum > 0 {
                return Ok(Some(UiEventEnvelope {
                    app,
                    window,
                    target: scroll_view,
                    event: UiEvent::ValueChanged(document_scroll_value(native_window, scroll_view, position, maximum, &theme)),
                }));
            }
        }
        let departed_web_surface = previously_hovered.filter(|previous| {
            native_window.hovered != Some(*previous)
                && native_window
                    .tree
                    .get(*previous)
                    .is_some_and(|widget| matches!(widget.kind, WidgetKind::WebSurface | WidgetKind::SceneSurface))
        });
        if matches!(kind, PointerEventKind::Move | PointerEventKind::Leave)
            && let Some(surface) = departed_web_surface
        {
            return Ok(Some(UiEventEnvelope {
                app,
                window,
                target: surface,
                event: UiEvent::Pointer {
                    kind: PointerEventKind::Leave,
                    x: 0,
                    y: 0,
                    button: 0,
                    modifiers,
                },
            }));
        }
        let Some(target) = target else {
            if kind == PointerEventKind::Down && button == 1
                && let Some(popup) = super::layout_overlay::dialog_widgets(&native_window.tree).last().copied()
                && native_window.tree.get(popup).is_some_and(|widget| widget.kind == WidgetKind::Popover)
                && native_window.geometry.get(&popup).is_some_and(|rect| !rect.contains(point))
            {
                return Ok(Some(UiEventEnvelope { app, window, target: popup,
                    event: UiEvent::Key { pressed: true, key_code: 9, modifiers: 0, text: None } }));
            }
            if kind == PointerEventKind::Down && button == 1 && context_menu.is_none() {
                native_window.opened_menu = None;
                native_window.opened_submenus.clear();
                native_window.popup_anchor = None;
            }
            return Ok(None);
        };
        let target_kind = native_window
            .tree
            .get(target.widget)
            .map(|widget| widget.kind);
        if matches!(kind, PointerEventKind::Move | PointerEventKind::Up)
            && target_kind == Some(WidgetKind::TextEditor)
            && let Some(drag) = native_window.text_drag
            && drag.widget == target.widget
        {
            if kind == PointerEventKind::Up {
                native_window.text_drag = None;
                if !drag.moved { return Ok(None); }
            } else {
                native_window.text_drag = Some(TextDragState { moved: true, ..drag });
            }
            let widget = native_window.tree.get(target.widget).expect("captured editor");
            let rect = native_window.geometry.get(&target.widget).copied().unwrap_or(target.rect);
            if let Some((_, caret)) = text_editor_selection_from_pointer_measured(
                widget.text.as_deref().unwrap_or_default(), rect, point, 0, &theme, measured_caret,
            ) {
                return Ok(Some(UiEventEnvelope { app, window, target: target.widget,
                    event: UiEvent::TextSelection { anchor: drag.anchor as u32, caret } }));
            }
        }
        let target_uses_pointer_selection =
            native_window.tree.get(target.widget).is_some_and(|widget| {
                widget.kind != WidgetKind::TreeItem
                    && widget.state.contains(WidgetState::KEY_NAVIGATION)
            });
        if matches!(kind, PointerEventKind::Move | PointerEventKind::Up)
            && let Some(drag) = native_window.text_drag
            && drag.widget == target.widget
            && matches!(
                target_kind,
                Some(WidgetKind::TextField | WidgetKind::PasswordField)
            )
        {
            if kind == PointerEventKind::Up && !drag.moved {
                native_window.text_drag = None;
                return Ok(None);
            }
            let widget = native_window.tree.get(target.widget);
            let value = widget
                .and_then(|widget| widget.text.as_deref())
                .unwrap_or_default();
            let caret = text_caret_from_pointer(
                value,
                target.rect,
                point.x,
                widget.is_some_and(|widget| widget.icon.is_some()),
                &theme, measured_caret,
            );
            let edit = TextEditState {
                widget: target.widget,
                anchor: drag.anchor,
                caret,
            };
            native_window.text_edit = Some(edit);
            native_window.text_selection_all = ((edit.anchor == 0 && edit.caret == value.len())
                || (edit.caret == 0 && edit.anchor == value.len()))
            .then_some(target.widget);
            if kind == PointerEventKind::Move {
                native_window.text_drag = Some(TextDragState {
                    widget: drag.widget,
                    anchor: drag.anchor,
                    moved: true,
                });
            } else {
                native_window.text_drag = None;
            }
            return Ok(None);
        }
        // Once a menubar menu is open, conventional desktop menus track the
        // pointer across their top-level headings. This deliberately happens
        // on motion rather than waiting for another click.
        if kind == PointerEventKind::Move
            && native_window.opened_menu.is_some()
            && target_kind == Some(WidgetKind::Menu)
            && native_window.opened_menu != Some(target.widget)
        {
            native_window.opened_menu = Some(target.widget);
            native_window.opened_submenus.clear();
            native_window.popup_anchor = None;
            native_window.focused = Some(target.widget);
            return Ok(None);
        }
        if kind == PointerEventKind::Move && native_window.opened_menu.is_some() {
            // Opt-in compound popup grids use spatial cells rather than direct
            // menu rows. Keep semantic focus with hover so arrows continue at
            // the previewed cell without a preliminary click.
            if target_kind == Some(WidgetKind::MenuItem)
                && popup_grid_owner(&native_window.tree, target.widget).is_some()
                && native_window.opened_menu.is_some_and(|menu| menu_contains(&native_window.tree, menu, target.widget))
            {
                native_window.focused = Some(target.widget);
            }
            if let Some(item) = native_window
                .tree
                .get(target.widget)
                .filter(|widget| widget.kind == WidgetKind::MenuItem && widget.state.contains(WidgetState::ENABLED))
                && let Some(owner) = item.parent
                && let Some(depth) = menu_chain_depth(
                    native_window.opened_menu,
                    &native_window.opened_submenus,
                    owner,
                )
            {
                native_window.focused = Some(target.widget);
                native_window.opened_submenus.truncate(depth);
                if let Some(submenu) = item.children().iter().copied().find(|child| {
                    native_window
                        .tree
                        .get(*child)
                        .is_some_and(|child| child.kind == WidgetKind::Menu)
                }) && native_window.opened_submenus.len() < MAX_OPEN_SUBMENUS
                {
                    native_window.opened_submenus.push(submenu);
                }
            }
        }
        let scroll_gesture = matches!(
            target_kind,
            Some(
                WidgetKind::ScrollBar
                    | WidgetKind::ProgressBar
                    | WidgetKind::Slider
                    | WidgetKind::ScrollView
            )
        ) && ((kind == PointerEventKind::Down && button == 1)
            || (kind == PointerEventKind::Move && native_window.scroll_drag.is_some())
            || (kind == PointerEventKind::Up
                && button == 1
                && native_window.scroll_drag.is_some()));
        if scroll_gesture {
            if kind == PointerEventKind::Down {
                native_window.scroll_drag = Some(target.widget);
                native_window.focused = Some(target.widget);
            }
            let text = native_window
                .tree
                .get(target.widget)
                .and_then(|widget| widget.text.as_deref());
            if let Some(value) = text.filter(|value| {
                value.starts_with("color-plane:#") || value.starts_with("color-hue:#")
            }) {
                let color = super::scroll::continuous_color_value(value, target.rect, point);
                let prefix = if value.starts_with("color-hue:") {"color-hue:"} else {"color-plane:"};
                native_window.tree.apply(&[Patch::Update {id:target.widget,state:None,text:ValueChange::Set(format!("{prefix}{color}")),icon:ValueChange::Keep,
                }])?;
                if kind == PointerEventKind::Up {
                    native_window.scroll_drag = None;
                    return Ok(Some(UiEventEnvelope {app, window, target:target.widget, event:UiEvent::ValueChanged(color),
                    }));
                }
                return Ok(None);
            }
            if text.is_some_and(|value| value.starts_with("color-picker:#")) {
                let color = super::scroll::color_picker_value(target.rect, point);
                native_window.tree.apply(&[Patch::Update {
                    id: target.widget,
                    state: None,
                    text: ValueChange::Set(format!("color-picker:{color}")),
                    icon: ValueChange::Keep,
                }])?;
                if kind == PointerEventKind::Up {
                    native_window.scroll_drag = None;
                } else {
                    return Ok(None);
                }
                return Ok(Some(UiEventEnvelope {
                    app,
                    window,
                    target: target.widget,
                    event: UiEvent::ValueChanged(color),
                }));
            }
            let scroll_view = (target_kind == Some(WidgetKind::ScrollView))
                .then(|| {
                    super::layout::scroll_view_geometry(
                        &native_window.tree,
                        target.widget,
                        &native_window.geometry,
                        &theme,
                    )
                })
                .flatten();
            let (position, total) = if let Some(scroll_view) = scroll_view {
                (
                    super::scroll::scroll_view_pointer_position(
                        target.rect,
                        point,
                        scroll_view.viewport_extent,
                        scroll_view.content_extent,
                    ),
                    scroll_view
                        .content_extent
                        .saturating_sub(scroll_view.viewport_extent),
                )
            } else {
                let (_, total) = super::scroll::fraction(
                    text.and_then(|value| value.strip_prefix("seek:")).or(text),
                );
                let position = if target_kind == Some(WidgetKind::Slider) {
                    super::scroll::progress_position(target.rect, point, 1000)
                } else if target_kind == Some(WidgetKind::ProgressBar) {
                    super::scroll::progress_position(target.rect, point, total)
                } else if target_kind == Some(WidgetKind::ScrollBar) && super::scroll::viewport(text).is_some() {
                    let flat = native_window.tree.get(target.widget).is_some_and(|widget| widget.state.contains(WidgetState::FLAT));
                    super::scroll::metadata_pointer_position(target.rect, point, text, flat)
                } else {
                    super::scroll::pointer_position(target.rect, point, total)
                };
                (
                    position,
                    if target_kind == Some(WidgetKind::Slider) {
                        1000
                    } else {
                        total
                    },
                )
            };
            if kind == PointerEventKind::Up {
                native_window.scroll_drag = None;
            }
            if target_kind == Some(WidgetKind::Slider) {
                let continuous = text.is_some_and(|value| value.split(';').any(|part| part == "continuous"));
                let metadata = text
                    .and_then(|value| value.split_once(';'))
                    .map(|(_, metadata)| metadata.to_owned());
                let slider_text = metadata.map_or_else(
                    || format!("{position}/{total}"),
                    |metadata| format!("{position}/{total};{metadata}"),
                );
                native_window.tree.apply(&[Patch::Update {
                    id: target.widget,
                    state: None,
                    text: ValueChange::Set(slider_text),
                    icon: ValueChange::Keep,
                }])?;
                if kind != PointerEventKind::Up && !continuous {
                    return Ok(None);
                }
            }
            return Ok(Some(UiEventEnvelope {
                app,
                window,
                target: target.widget,
                event: UiEvent::ValueChanged(document_scroll_value(native_window, target.widget, position, total, &theme)),
            }));
        }
        // Positioned surfaces expose coordinates in the enclosing canvas,
        // not in a moving child's rectangle. Keep capture through the drag.
        let arrangement = native_window.tree.get(target.widget)
            // A positioned editor owns text selection, not canvas dragging.
            // Keep measured caret input intact inside document overlays.
            .filter(|widget| !matches!(widget.kind, WidgetKind::TextField | WidgetKind::PasswordField | WidgetKind::TextEditor))
            .and_then(|widget| widget.parent).and_then(|parent| native_window.tree.get(parent))
            .filter(|parent| {
                super::layout_tokens::positioned_item(parent.text.as_deref()).is_some()
            })
            .and_then(|parent| parent.parent).and_then(|parent| native_window.geometry.get(&parent)).copied();
        if let Some(rect) = arrangement {
            if kind == PointerEventKind::Down && button == 1 { native_window.scroll_drag=Some(target.widget); }
            if native_window.scroll_drag==Some(target.widget) && matches!(kind,PointerEventKind::Down|PointerEventKind::Move|PointerEventKind::Up) {
                if kind==PointerEventKind::Up { native_window.scroll_drag=None; }
                let x=(i64::from(point.x-rect.origin.x)*1000/i64::from(rect.size.width.max(1))).clamp(-2000,3000) as i32;
                let y=(i64::from(point.y-rect.origin.y)*1000/i64::from(rect.size.height.max(1))).clamp(-2000,3000) as i32;
                return Ok(Some(UiEventEnvelope {app,window,target:target.widget,event:UiEvent::Pointer{kind,x,y,button:1,modifiers,
                    },
                }));
            }
        }
        // Reorder grips capture motion like sliders, but keep application data
        // in the actor. Handles in one named group identify their drop slots.
        let reorder = native_window.tree.get(target.widget)
            .and_then(|widget| widget.text.as_deref())
            .and_then(|text| text.strip_prefix("drag-handle:"))
            .and_then(|text| text.rsplit_once(':'))
            .map(|(group, index)| (group.to_owned(), index.to_owned()));
        if let Some((group, source)) = reorder {
            if kind == PointerEventKind::Down && button == 1 {
                native_window.scroll_drag = Some(target.widget);
            }
            if native_window.scroll_drag == Some(target.widget) && matches!(kind, PointerEventKind::Move | PointerEventKind::Up) {
                let prefix = format!("drag-handle:{group}:");
                let destination = native_window.hit_targets.iter().filter_map(|candidate| {
                    let text = native_window.tree.get(candidate.widget)?.text.as_deref()?;
                    let index = text.strip_prefix(&prefix)?;
                    let cx = i64::from(candidate.rect.origin.x) + i64::from(candidate.rect.size.width)/2;
                    let cy = i64::from(candidate.rect.origin.y) + i64::from(candidate.rect.size.height)/2;
                    Some(((cx-i64::from(point.x)).pow(2)+(cy-i64::from(point.y)).pow(2), index.to_owned(),
                        ))
                }).min_by_key(|item|item.0).map_or(source, |item|item.1);
                let action = if kind == PointerEventKind::Up { native_window.scroll_drag = None; "drop" } else { "preview" };
                return Ok(Some(UiEventEnvelope {app,window,target:target.widget,event:UiEvent::ValueChanged(format!("{action}:{destination}")),
                }));
            }
        }
        let table_gesture = target_kind == Some(WidgetKind::Table)
            && native_window.tree.get(target.widget).is_some_and(|widget| {
                super::layout_tokens::resizable_table(widget.text.as_deref())
            });
        let split_gesture = (target_kind == Some(WidgetKind::SplitPane) || table_gesture)
            && ((kind == PointerEventKind::Down && button == 1)
                || (kind == PointerEventKind::Move
                    && native_window.scroll_drag == Some(target.widget))
                || (kind == PointerEventKind::Up
                    && button == 1
                    && native_window.scroll_drag == Some(target.widget)));
        if split_gesture {
            if kind == PointerEventKind::Down {
                native_window.scroll_drag = Some(target.widget);
            } else if kind == PointerEventKind::Up {
                native_window.scroll_drag = None;
            }
        }
        if kind == PointerEventKind::Up && button == 1 && target_kind == Some(WidgetKind::Menu) {
            // KEY_NAVIGATION opts a toolbar menu into the SDK HoldMenuButton
            // contract: a short click is its default command, not disclosure.
            if native_window.tree.get(target.widget).is_some_and(|widget|
                widget.state.contains(WidgetState::KEY_NAVIGATION))
            {
                if native_window.opened_menu == Some(target.widget) {
                    return Ok(Some(UiEventEnvelope { app, window, target: target.widget,
                        event: UiEvent::Pointer { kind, x: point.x, y: point.y, button, modifiers } }));
                }
                return Ok((previously_pressed == Some(target.widget)).then_some(UiEventEnvelope {
                    app, window, target: target.widget, event: UiEvent::Activate,
                }));
            }
            native_window.opened_menu = if native_window.opened_menu == Some(target.widget) {
                None
            } else {
                Some(target.widget)
            };
            native_window.opened_submenus.clear();
            native_window.menu_scroll_rows.insert(target.widget, 0);
            native_window.popup_anchor = None;
            native_window.focused = Some(target.widget);
            return Ok(None);
        }
        if kind == PointerEventKind::Down
            && button == 1
            && target_kind != Some(WidgetKind::Menu)
            && !target_kind.is_some_and(is_popup_action_kind)
            && native_window.opened_menu.is_some_and(|menu| {
                !menu_contains(&native_window.tree, menu, target.widget)
            })
        {
            native_window.opened_menu = None;
            native_window.opened_submenus.clear();
            native_window.popup_anchor = None;
        }
        if kind == PointerEventKind::Down && button == 1 && target.focusable {
            native_window.focused = Some(target.widget);
            if let Some(widget) = native_window.tree.get(target.widget)
                && matches!(
                    widget.kind,
                    WidgetKind::TextField | WidgetKind::PasswordField
                )
            {
                let value = widget.text.as_deref().unwrap_or_default();
                let pointer_caret = text_caret_from_pointer(
                    value,
                    target.rect,
                    point.x,
                    widget.icon.is_some(),
                    &theme, measured_caret,
                );
                if widget.state.contains(WidgetState::SELECT_ALL_ON_FOCUS)
                    && previously_focused != Some(target.widget)
                {
                    native_window.text_selection_all = Some(target.widget);
                    native_window.text_edit = Some(TextEditState {
                        widget: target.widget,
                        anchor: 0,
                        caret: value.len(),
                    });
                } else {
                    let caret = pointer_caret;
                    let anchor = if modifiers & 1 != 0 {
                        native_window
                            .text_edit
                            .filter(|edit| edit.widget == target.widget)
                            .map_or(caret, |edit| edit.anchor)
                    } else {
                        caret
                    };
                    native_window.text_edit = Some(TextEditState {
                        widget: target.widget,
                        anchor,
                        caret,
                    });
                }
                native_window.text_drag = Some(TextDragState {
                    widget: target.widget,
                    anchor: pointer_caret,
                    moved: false,
                });
            } else if !text_menu::command(target.widget) {
                native_window.text_edit = None;
                native_window.text_drag = None;
            }
        }
        let editor_selection = (kind == PointerEventKind::Down
            && button == 1
            && target_kind == Some(WidgetKind::TextEditor))
        .then(|| {
            let widget = native_window.tree.get(target.widget)?;
            let rect = native_window.geometry.get(&target.widget).copied()?;
            text_editor_selection_from_pointer_measured(
                widget.text.as_deref()?,
                rect,
                point,
                modifiers,
                &theme,
                measured_caret,
            )
        })
        .flatten();
        if let Some((anchor, _)) = editor_selection {
            native_window.text_drag = Some(TextDragState { widget: target.widget, anchor: anchor as usize, moved: false });
        }
        // Pointer moves over a native editor carry the exact character under
        // the mouse as a semantic sentinel. RicePaper uses it for local-module
        // navigation without moving the caret or estimating canvas metrics a
        // second time in the application actor.
        let editor_hover = (kind == PointerEventKind::Move
            && target_kind == Some(WidgetKind::TextEditor))
        .then(|| {
            let widget = native_window.tree.get(target.widget)?;
            let rect = native_window.geometry.get(&target.widget).copied()?;
            text_editor_selection_from_pointer_measured(widget.text.as_deref()?, rect, point, 0, &theme, measured_caret)
                .and_then(|(_, caret)| i32::try_from(caret).ok())
        })
        .flatten();
        let opens_submenu = native_window.tree.get(target.widget).is_some_and(|widget| {
            widget.kind == WidgetKind::MenuItem
                && widget.children().iter().any(|child| {
                    native_window
                        .tree
                        .get(*child)
                        .is_some_and(|child| child.kind == WidgetKind::Menu)
                })
        });
        if kind == PointerEventKind::Up
            && button == 1
            && target_kind.is_some_and(is_popup_action_kind)
            && !opens_submenu
        {
            if let Some(menu) = native_window.opened_menu.take() {
                native_window.focused = Some(menu);
            }
            native_window.opened_submenus.clear();
            native_window.popup_anchor = None;
        }
        if kind == PointerEventKind::Up && button == 1 && opens_submenu {
            if let Some(item) = native_window.tree.get(target.widget)
                && let Some(owner) = item.parent
                && let Some(depth) = menu_chain_depth(
                    native_window.opened_menu,
                    &native_window.opened_submenus,
                    owner,
                )
            {
                native_window.opened_submenus.truncate(depth);
                if let Some(submenu) = item.children().iter().copied().find(|child| {
                    native_window
                        .tree
                        .get(*child)
                        .is_some_and(|child| child.kind == WidgetKind::Menu)
                }) && native_window.opened_submenus.len() < MAX_OPEN_SUBMENUS
                {
                    native_window.opened_submenus.push(submenu);
                }
            }
            return Ok(None);
        }
        let pointer_tree_toggle = kind == PointerEventKind::Up
            && button == 1
            && tree_disclosure_hit(
                &native_window.tree,
                target.widget,
                target.rect,
                point,
                &theme,
            );
        if pointer_tree_toggle {
            toggle_tree_item(native_window, target.widget)?;
        }
        let event = if let Some(caret) = editor_hover {
            UiEvent::Pointer {
                kind,
                x: caret,
                y: -1,
                button,
                modifiers,
            }
        } else if let Some((anchor, caret)) = editor_selection {
            UiEvent::TextSelection { anchor, caret }
        } else if kind == PointerEventKind::Up
            && button == 1
            && target.activatable
            // A primary release is an activation only when this exact
            // control owned the matching press. Apart from matching desktop
            // toolkit semantics, this makes a duplicated/lost-capture
            // pointer-up harmless: it cannot toggle a checkbox twice or
            // activate a second tree row after a held drag.
            && previously_pressed == Some(target.widget)
            && !target_uses_pointer_selection
            && !pointer_tree_toggle
        {
            UiEvent::Activate
        } else {
            let vector_point = (target_kind == Some(WidgetKind::SceneSurface)).then(|| {
                native_window.web_scenes.get(&target.widget).map(|scene| {
                    // Hit targets are clipped for ownership; scene coordinates
                    // must use the full zoomed/panned page, including its origin.
                    let surface = native_window.geometry.get(&target.widget).copied().unwrap_or(target.rect);
                    super::scene_surface::logical_point(scene, surface, point)
                })
            }).flatten();
            let x = if let Some((x, _)) = vector_point {
                super::scene_surface::pointer_coordinate(x)
            } else if target_kind == Some(WidgetKind::SplitPane) || table_gesture {
                let split_rect = native_window
                    .geometry
                    .get(&target.widget)
                    .copied()
                    .unwrap_or(target.rect);
                let local = point
                    .x
                    .saturating_sub(split_rect.origin.x)
                    .max(0)
                    .cast_unsigned();
                let local = if target_kind == Some(WidgetKind::SplitPane)
                    && native_window.tree.get(target.widget).is_some_and(|widget| {
                        super::render::split_is_reversed(&native_window.tree, widget, &native_window.geometry)
                    })
                {
                    split_rect.size.width.saturating_sub(local.min(split_rect.size.width))
                } else {
                    local.min(split_rect.size.width)
                };
                i32::try_from(
                    u64::from(local).saturating_mul(1_000)
                        / u64::from(split_rect.size.width.max(1)),
                )
                .unwrap_or(1_000)
            } else if matches!(target_kind, Some(WidgetKind::WebSurface | WidgetKind::SceneSurface)) {
                let local = point.x.saturating_sub(target.rect.origin.x);
                native_window
                    .web_scenes
                    .get(&target.widget)
                    .map_or(local, |scene| {
                        let scaled = i64::from(local)
                            .saturating_mul(i64::from(scene.viewport_width))
                            .checked_div(i64::from(target.rect.size.width.max(1)))
                            .unwrap_or(i64::from(local));
                        i32::try_from(scaled).unwrap_or(if scaled < 0 {
                            i32::MIN
                        } else {
                            i32::MAX
                        })
                    })
            } else if target_kind == Some(WidgetKind::VirtualGrid) {
                let rect = native_window.geometry.get(&target.widget).copied().unwrap_or(target.rect);
                point.x.saturating_sub(rect.origin.x)
            } else if target_kind == Some(WidgetKind::TerminalGrid) {
                point.x.saturating_sub(target.rect.origin.x)
            } else {
                point.x
            };
            let y = if let Some((_, y)) = vector_point {
                super::scene_surface::pointer_coordinate(y)
            } else if target_kind == Some(WidgetKind::SplitPane) {
                let rect = native_window.geometry.get(&target.widget).copied().unwrap_or(target.rect);
                let local = point.y.saturating_sub(rect.origin.y).max(0).cast_unsigned();
                i32::try_from(u64::from(local.min(rect.size.height)) * 1_000
                    / u64::from(rect.size.height.max(1)),
                ).unwrap_or(1_000)
            } else if matches!(target_kind, Some(WidgetKind::WebSurface | WidgetKind::SceneSurface)) {
                let local = point.y.saturating_sub(target.rect.origin.y);
                native_window
                    .web_scenes
                    .get(&target.widget)
                    .map_or(local, |scene| {
                        // Browser surfaces preserve their aspect ratio from width.
                        let scaled = i64::from(local)
                            .saturating_mul(i64::from(scene.viewport_width))
                            .checked_div(i64::from(target.rect.size.width.max(1)))
                            .unwrap_or(i64::from(local));
                        i32::try_from(scaled).unwrap_or(if scaled < 0 {
                            i32::MIN
                        } else {
                            i32::MAX
                        })
                    })
            } else if target_kind == Some(WidgetKind::VirtualGrid) {
                let rect = native_window.geometry.get(&target.widget).copied().unwrap_or(target.rect);
                point.y.saturating_sub(rect.origin.y)
            } else if target_kind == Some(WidgetKind::TerminalGrid) {
                point.y.saturating_sub(target.rect.origin.y)
            } else {
                point.y
            };
            UiEvent::Pointer {
                kind,
                x,
                y,
                button: if split_gesture { 1 } else { button },
                modifiers,
            }
        };
        if pointer_tree_toggle {
            native_window.geometry.clear();
            native_window.hit_targets.clear();
        }
        text_menu::restore_focus(native_window, target.widget, &event);
        Ok(Some(UiEventEnvelope {
            app,
            window,
            target: target.widget,
            event,
        }))
    }

    /// Scroll a spatial-navigation destination into view before its key event
    /// reaches the actor. Use the same scroll update as a wheel or scrollbar,
    /// so the actor retains the position through subsequent media/UI patches.
    pub(crate) fn navigation_scroll_event(&self, event: &UiEventEnvelope) -> Option<UiEventEnvelope> {
        if !matches!(&event.event, UiEvent::Key { pressed: true, key_code: 111 | 113 | 114 | 116, modifiers, .. } if modifiers & (1 | 4 | 8 | 64) == 0) {
            return None;
        }
        let window = self.windows.get(&(event.app, event.window))?;
        let tree = &window.tree;
        if !tree.get(event.target)?.state.contains(WidgetState::KEY_NAVIGATION) {
            return None;
        }
        let collection = navigation_collection(tree, event.target)?;
        // File panes already consume scroll ValueChanged events. Leave other
        // collections' existing keyboard/scroll contracts unchanged.
        if !super::layout_tokens::is_file_grid(tree.get(collection)?.text.as_deref()) {
            return None;
        }
        let scroll = enclosing_scroll_view(tree, collection)?;
        let theme = effective_window_theme(tree, self.theme);
        let viewport = super::layout::scroll_view_geometry(tree, scroll, &window.geometry, &theme)?;
        let mut tile = event.target;
        while let Some(parent) = tree.get(tile)?.parent {
            if parent == collection { break; }
            tile = parent;
        }
        let rect = window.geometry.get(&tile)?;
        let top = i64::from(rect.origin.y) - i64::from(viewport.track.origin.y);
        let bottom = top + i64::from(rect.size.height);
        let delta = if top < 0 { top } else { (bottom - i64::from(viewport.viewport_extent)).max(0) };
        let maximum = viewport.content_extent.saturating_sub(viewport.viewport_extent);
        let position = (i64::from(viewport.position) + delta).clamp(0, i64::from(maximum)) as u32;
        (position != viewport.position).then(|| UiEventEnvelope {
            app: event.app, window: event.window, target: scroll,
            event: UiEvent::ValueChanged(format!("{position}/{maximum}")),
        })
    }

    /// Maps keyboard input at the focused semantic widget boundary.
    ///
    /// Enter and Space activate activatable controls on key-down. Other input
    /// is forwarded as a semantic key event.
    ///
    /// # Errors
    ///
    /// Returns [`NativeUiError::UnknownWindow`] when the target window is absent.
    pub fn key_event(
        &mut self,
        app: AppId,
        window: WindowId,
        pressed: bool,
        key_code: u32,
        modifiers: u16,
        text: Option<String>,
    ) -> Result<Option<UiEventEnvelope>, NativeUiError> {
        let native_window = self
            .windows
            .get_mut(&(app, window))
            .ok_or(NativeUiError::UnknownWindow { app, window })?;
        if native_window.grid_resync_shadow.is_some() { return Ok(None); }
        if pressed && native_window.navigation_open.is_some() && matches!(key_code, 9 | 27) {
            native_window.focused = native_window.navigation_open.take();
            return Ok(None);
        }
        if pressed && matches!(key_code, 36 | 65 | 104)
            && let Some(focused) = native_window.focused.filter(|id| native_window.navigation_headers.contains_key(id))
        {
            native_window.navigation_open = if native_window.navigation_open == Some(focused) {None} else {Some(focused)};
            return Ok(None);
        }

        // Native key codes: Menu and Shift+F10. Anchor in window coordinates,
        // avoiding document zoom/pan and application-provided pointer offsets.
        if pressed && ((key_code == 135 && modifiers == 0) || (key_code == 76 && modifiers == 1))
            && let Some(focused) = native_window.focused
            && let Some(menu) = contextual_menu(&native_window.tree, Some(focused))
            && let Some(rect) = native_window.geometry.get(&focused)
        {
            native_window.popup_anchor = Some(Point {
                x: rect.origin.x.saturating_add_unsigned(rect.size.width / 2),
                y: rect.origin.y.saturating_add_unsigned(rect.size.height / 2),
            });
            native_window.popup_context_target = Some(focused);
            native_window.opened_menu = Some(menu);
            native_window.opened_submenus.clear();
            native_window.focused = native_window.tree.get(menu).and_then(|widget| {
                widget.children().iter().copied().find(|id| native_window.tree.get(*id).is_some_and(|child| {
                    child.state.contains(WidgetState::ENABLED) && matches!(child.kind, WidgetKind::MenuItem | WidgetKind::Menu)
                }))
            });
            return Ok(None);
        }
        if pressed && matches!(key_code, 36 | 65 | 104)
            && let (Some(sidebar), Some(focused)) = (native_window.navigation_open, native_window.focused)
            && super::layout_overlay::widget_belongs_to(&native_window.tree, focused, sidebar)
        { native_window.navigation_open = None; }
        let focused_is_editor = native_window.focused.is_some_and(|focused| {
            native_window.tree.get(focused).is_some_and(|widget| {
                matches!(
                    widget.kind,
                    WidgetKind::TerminalGrid | WidgetKind::VirtualGrid
                ) || (widget.kind == WidgetKind::TextEditor && !widget.state.contains(WidgetState::KEY_NAVIGATION))
                    || (widget.kind == WidgetKind::TextField && widget.state.contains(WidgetState::KEY_NAVIGATION))
            })
        });
        if pressed && key_code == 23 && !focused_is_editor {
            native_window.text_selection_all = None;
            let focusable = native_window
                .hit_targets
                .iter()
                .filter(|target| target.focusable)
                .map(|target| target.widget)
                .collect::<Vec<_>>();
            let Some(next) = next_focus(&focusable, native_window.focused, modifiers & 1 != 0)
            else {
                return Ok(None);
            };
            native_window.focused = Some(next);
            return Ok(Some(UiEventEnvelope {
                app,
                window,
                target: next,
                event: UiEvent::Focus { focused: true },
            }));
        }
        let printable_text = text
            .as_deref()
            .is_some_and(|input| input.chars().any(|character| !character.is_control()));
        if pressed
            && !printable_text
            && matches!(key_code, 27 | 9)
            && native_window.opened_menu.is_some()
        {
            // Escape unwinds one popup level. Keep the trusted root anchor
            // and owner until the final dismissal so Menu/Shift+F10 can reopen.
            if let Some(target) = close_deepest_submenu(native_window) {
                return Ok(Some(UiEventEnvelope { app, window, target,
                    event: UiEvent::Focus { focused: true } }));
            }
            let target = native_window.opened_menu.take().expect("checked open menu");
            native_window.opened_submenus.clear();
            let target = native_window.popup_anchor.and(native_window.popup_context_target.take()).unwrap_or(target);
            native_window.popup_anchor = None;
            native_window.focused = Some(target);
            return Ok(Some(UiEventEnvelope {
                app,
                window,
                target,
                event: UiEvent::Focus { focused: true },
            }));
        }
        // Explicit popup grids opt into two-dimensional navigation. Ordinary
        // menus retain their linear ordering and embedded text controls keep
        // their own caret keys.
        if pressed && !printable_text && modifiers & (1 | 4 | 8 | 64) == 0
            && matches!(key_code, 37..=40 | 110 | 111 | 113..=116)
            && let Some(menu) = native_window.opened_submenus.last().copied().or(native_window.opened_menu)
            && let Some(target) = popup_grid_destination(&native_window.tree, menu, native_window.focused, key_code)
        {
            native_window.focused = Some(target);
            native_window.hovered = None;
            return Ok(Some(UiEventEnvelope { app, window, target, event: UiEvent::Focus { focused: true } }));
        }
        // Popup navigation is based on menu order, not spatial neighbours in
        // the worksheet beneath it. Text fields embedded in popups retain their
        // own caret keys; disabled commands and separators are never focused.
        if pressed && !printable_text && modifiers & (1 | 4 | 8 | 64) == 0
            && let Some(root_menu) = native_window.opened_menu
            && native_window.focused.is_none_or(|id| !menu_contains(&native_window.tree, root_menu, id)
                || native_window.tree.get(id).is_some_and(|item| matches!(item.kind, WidgetKind::Menu | WidgetKind::MenuItem)))
        {
            // A newly registered context menu can coexist briefly with the
            // owner's semantic focus patch. The open popup still owns arrows;
            // only an editor inside that popup may keep its own caret keys.
            let focused = native_window.focused.filter(|id| menu_contains(&native_window.tree, root_menu, *id));
            let focused_owner = focused.and_then(|id| native_window.tree.get(id)).and_then(|item| item.parent)
                .filter(|owner| menu_chain_depth(Some(root_menu), &native_window.opened_submenus, *owner).is_some());
            let menu = focused_owner.unwrap_or_else(|| native_window.opened_submenus.last().copied().unwrap_or(root_menu));
            let mut next = None;
            if matches!(key_code, 38 | 40 | 111 | 116 | 110 | 115) {
                let items = menu_keyboard_items(&native_window.tree, menu);
                if !items.is_empty() {
                    let current = focused.and_then(|id| items.iter().position(|item| *item == id));
                    let index = match key_code {
                        110 => 0,
                        115 => items.len() - 1,
                        38 | 111 => current.map_or(items.len() - 1, |index| (index + items.len() - 1) % items.len()),
                        _ => current.map_or(0, |index| (index + 1) % items.len()),
                    };
                    if let Some(depth) = menu_chain_depth(Some(root_menu), &native_window.opened_submenus, menu) {
                        native_window.opened_submenus.truncate(depth);
                    }
                    next = Some(items[index]);
                }
            } else if matches!(key_code, 39 | 114 | 36 | 104)
                && let Some(item) = focused.and_then(|id| native_window.tree.get(id))
                && item.state.contains(WidgetState::ENABLED)
                && let Some(submenu) = item.children().iter().copied().find(|id| native_window.tree.get(*id)
                    .is_some_and(|child| child.kind == WidgetKind::Menu && child.state.contains(WidgetState::ENABLED)))
                && let Some(depth) = menu_chain_depth(Some(root_menu), &native_window.opened_submenus, menu)
            {
                native_window.opened_submenus.truncate(depth);
                if native_window.opened_submenus.len() < MAX_OPEN_SUBMENUS {
                    native_window.opened_submenus.push(submenu);
                    next = menu_keyboard_items(&native_window.tree, submenu).first().copied().or(focused);
                }
            } else if matches!(key_code, 37 | 113) {
                next = close_deepest_submenu(native_window);
            }
            if let Some(target) = next {
                native_window.focused = Some(target);
                native_window.hovered = None;
                return Ok(Some(UiEventEnvelope { app, window, target,
                    event: UiEvent::Focus { focused: true } }));
            }
            if matches!(key_code, 37..=40 | 110 | 111 | 113..=116) {
                return Ok(None);
            }
        }
        let target_id = native_window
            .focused
            .or_else(|| {
                native_window.hit_targets.iter().find_map(|target| {
                    native_window
                        .tree
                        .get(target.widget)
                        .is_some_and(|widget| widget.state.contains(WidgetState::FOCUSED))
                        .then_some(target.widget)
                })
            })
            .or_else(|| {
                native_window
                    .hit_targets
                    .iter()
                    .find(|target| target.focusable)
                    .map(|target| target.widget)
            });
        let Some(target_id) = target_id else {
            return Ok(None);
        };
        if pressed && matches!(key_code, 40 | 116)
            && native_window.tree.get(target_id).is_some_and(|widget|
                widget.kind == WidgetKind::Menu && widget.state.contains(WidgetState::KEY_NAVIGATION)
                    && widget.state.contains(WidgetState::ENABLED))
        {
            native_window.opened_menu = Some(target_id);
            native_window.opened_submenus.clear();
            native_window.popup_anchor = None;
            native_window.focused = menu_keyboard_items(&native_window.tree, target_id).first().copied();
            return Ok(None);
        }
        // Arrow/Home/End navigation belongs to an active text editor before
        // it belongs to the surrounding spatial-navigation graph.  Resolving
        // a neighbouring button first makes the same Right Arrow either move
        // the caret or leave a select-all range intact depending on responsive
        // layout geometry.
        let target_owns_text_navigation = native_window.tree.get(target_id).is_some_and(|widget| {
            matches!(
                widget.kind,
                WidgetKind::TextField
                    | WidgetKind::PasswordField
                    | WidgetKind::TextEditor
                    | WidgetKind::TerminalGrid
            )
        });
        let tree_navigation = if pressed
            && !target_owns_text_navigation
            && modifiers & (1 | 4 | 8 | 64) == 0
            && !printable_text
            && matches!(key_code, 37..=40 | 111 | 113 | 114 | 116)
        {
            tree_navigation_action(
                &native_window.tree,
                &native_window.geometry,
                target_id,
                key_code,
            )
        } else {
            None
        };
        if tree_navigation == Some(TreeNavigationAction::Toggle) {
            native_window.focused = Some(target_id);
            toggle_tree_item(native_window, target_id)?;
            native_window.geometry.clear();
            native_window.hit_targets.clear();
            return Ok(Some(UiEventEnvelope {
                app,
                window,
                target: target_id,
                event: UiEvent::Key {
                    pressed,
                    key_code,
                    modifiers,
                    text,
                },
            }));
        }
        let tree_focus = match tree_navigation {
            Some(TreeNavigationAction::Focus(target)) => Some(target),
            _ => None,
        };
        let target_id = if let Some(target) = tree_focus {
            target
        } else if pressed
            && !target_owns_text_navigation
            && modifiers & (1 | 4 | 8 | 64) == 0
            && !printable_text
            && matches!(key_code, 37..=40 | 111 | 113 | 114 | 116)
        {
            navigation_neighbor(
                &native_window.tree,
                &native_window.geometry,
                target_id,
                key_code,
            )
            .unwrap_or(target_id)
        } else {
            target_id
        };
        if pressed
            && native_window
                .tree
                .get(target_id)
                .is_some_and(|widget| widget.state.contains(WidgetState::KEY_NAVIGATION))
        {
            native_window.focused = Some(target_id);
        }
        let target = native_window
            .hit_targets
            .iter()
            .find(|target| target.widget == target_id)
            .copied()
            .or_else(|| {
                // Patch application invalidates geometry until the next
                // render, but keyboard focus is semantic state and must not
                // disappear during that short interval. Browsers likewise
                // route caret/editing keys independently of a fresh paint.
                let widget = native_window.tree.get(target_id)?;
                let (focusable, activatable) = super::render::widget_interaction(
                    widget.kind,
                    widget.state.contains(WidgetState::ENABLED),
                );
                (focusable || activatable).then_some(HitTarget {
                    widget: target_id,
                    rect: Rect {
                        origin: Point { x: 0, y: 0 },
                        size: Size {
                            width: 0,
                            height: 0,
                        },
                    },
                    focusable,
                    activatable,
                })
            });
        let Some(target) = target else {
            return Ok(None);
        };
        let target_widget = native_window.tree.get(target.widget);
        if target_widget.is_some_and(|widget| widget.kind == WidgetKind::TextField && widget.state.contains(WidgetState::KEY_NAVIGATION))
            && !printable_text && matches!(key_code, 23 | 110 | 111 | 113 | 114 | 115 | 116) {
            return Ok(Some(UiEventEnvelope { app, window, target: target.widget,
                event: UiEvent::Key { pressed, key_code, modifiers, text } }));
        }
        let chat_composer=target_widget.is_some_and(|widget|widget.kind==WidgetKind::TextField
            && widget.parent.and_then(|parent|native_window.tree.get(parent)).and_then(|parent|parent.text.as_deref())
                .is_some_and(|text|text.split(':').any(|token|token=="chat-composer")));
        let newline=chat_composer&&pressed&&matches!(key_code,36|104)&&modifiers==1;
        let text=if newline {Some("\n".into())}else{text};
        let printable_text=printable_text||newline;
        let text_control = target_widget.is_some_and(|widget| {
            matches!(
                widget.kind,
                WidgetKind::TextField | WidgetKind::PasswordField
            )
        });
        if pressed
            && text_control
            && modifiers & 4 != 0
            && modifiers & 8 == 0
            && matches!(key_code, 38 | 65 | 97)
        {
            native_window.text_selection_all = Some(target.widget);
            let length = target_widget
                .and_then(|widget| widget.text.as_deref())
                .map_or(0, str::len);
            native_window.text_edit = Some(TextEditState {
                widget: target.widget,
                anchor: 0,
                caret: length,
            });
            return Ok(Some(UiEventEnvelope {
                app,
                window,
                target: target.widget,
                event: UiEvent::Key {
                    pressed,
                    key_code,
                    modifiers,
                    text,
                },
            }));
        }
        if chat_composer&&pressed&&!printable_text&&matches!(key_code,110|111|115|116)&&modifiers&!(1|4)==0 {
            let value=target_widget.and_then(|widget|widget.text.as_deref()).unwrap_or_default();
            let mut edit=current_text_edit(native_window.text_edit,target.widget,value);
            let start=value[..edit.caret].rfind('\n').map_or(0,|n|n+1);
            let end=value[edit.caret..].find('\n').map_or(value.len(),|n|edit.caret+n);
            let column=value[start..edit.caret].chars().count();
            let next=match key_code {
                110=>if modifiers&4!=0{0}else{start},115=>if modifiers&4!=0{value.len()}else{end},
                111 if start>0=>{let previous_end=start-1;let previous_start=value[..previous_end].rfind('\n').map_or(0,|n|n+1);
                    previous_start+value[previous_start..previous_end].char_indices().nth(column).map_or(previous_end-previous_start,|(n,_)|n)},
                116 if end<value.len()=>{let next_start=end+1;let next_end=value[next_start..].find('\n').map_or(value.len(),|n|next_start+n);
                    next_start+value[next_start..next_end].char_indices().nth(column).map_or(next_end-next_start,|(n,_)|n)},
                _=>edit.caret,
            };
            edit.caret=next;if modifiers&1==0{edit.anchor=next;}
            native_window.text_edit=Some(edit);
            return Ok(Some(UiEventEnvelope {app,window,target:target.widget,event:UiEvent::Key {pressed,key_code,modifiers,text}}));
        }
        if pressed && text_control && !printable_text && matches!(key_code, 110 | 113 | 114 | 115) {
            let value = target_widget
                .and_then(|widget| widget.text.as_deref())
                .unwrap_or_default();
            let mut edit = current_text_edit(native_window.text_edit, target.widget, value);
            let extend = modifiers & 1 != 0;
            let control = modifiers & 4 != 0;
            let selection_start = edit.anchor.min(edit.caret);
            let selection_end = edit.anchor.max(edit.caret);
            let next = match key_code {
                110 => 0,
                115 => value.len(),
                113 if !extend && selection_start != selection_end => selection_start,
                114 if !extend && selection_start != selection_end => selection_end,
                113 if control => previous_word_boundary(value, edit.caret),
                114 if control => next_word_boundary(value, edit.caret),
                113 => previous_text_boundary(value, edit.caret),
                114 => next_text_boundary(value, edit.caret),
                _ => edit.caret,
            };
            edit.caret = next;
            if !extend {
                edit.anchor = next;
            }
            native_window.text_edit = Some(edit);
            native_window.text_selection_all = (edit.anchor == 0 && edit.caret == value.len()
                || edit.caret == 0 && edit.anchor == value.len())
            .then_some(target.widget);
            return Ok(Some(UiEventEnvelope {
                app,
                window,
                target: target.widget,
                event: UiEvent::Key {
                    pressed,
                    key_code,
                    modifiers,
                    text,
                },
            }));
        }
        let event = if tree_focus.is_some() {
            UiEvent::Focus { focused: true }
        } else if pressed
            && text_control
            && modifiers & 8 == 0
            // Only actual text insertion or deletion changes the value.
            // Function keys (notably spreadsheet F2) must reach the app as
            // keys instead of producing a spurious unchanged ValueChanged.
            && (printable_text || matches!(key_code, 8 | 22 | 119))
        {
            let mut value = target_widget
                .and_then(|widget| widget.text.clone())
                .unwrap_or_default();
            let mut edit = current_text_edit(native_window.text_edit, target.widget, &value);
            let (mut start, mut end) = (edit.anchor.min(edit.caret), edit.anchor.max(edit.caret));
            if !printable_text && matches!(key_code, 8 | 22 | 119) {
                if start == end {
                    if matches!(key_code, 8 | 22) {
                        start = if modifiers & 4 != 0 {
                            previous_word_boundary(&value, edit.caret)
                        } else {
                            previous_text_boundary(&value, edit.caret)
                        };
                    } else {
                        end = if modifiers & 4 != 0 {
                            next_word_boundary(&value, edit.caret)
                        } else {
                            next_text_boundary(&value, edit.caret)
                        };
                    }
                }
                value.replace_range(start..end, "");
                edit.anchor = start;
                edit.caret = start;
            } else if let Some(input) = text.as_deref() {
                let inserted = input
                    .chars()
                    .filter(|character| !character.is_control() || chat_composer && *character=='\n')
                    .collect::<String>();
                if value
                    .len()
                    .saturating_sub(end - start)
                    .saturating_add(inserted.len())
                    <= MAX_TEXT_BYTES
                {
                    value.replace_range(start..end, &inserted);
                    let caret = start + inserted.len();
                    edit.anchor = caret;
                    edit.caret = caret;
                } else {
                    for character in inserted.chars() {
                        if value
                            .len()
                            .saturating_sub(end - start)
                            .saturating_add(character.len_utf8())
                            > MAX_TEXT_BYTES
                        {
                            break;
                        }
                        value.replace_range(start..end, &character.to_string());
                        let caret = start + character.len_utf8();
                        edit.anchor = caret;
                        edit.caret = caret;
                        start = caret;
                        end = caret;
                    }
                }
            }
            native_window.text_edit = Some(edit);
            native_window.text_selection_all = None;
            UiEvent::ValueChanged(value)
        } else if pressed
            && target.activatable
            && matches!(key_code, 36 | 65 | 104)
            && !target_widget
                .is_some_and(|widget| widget.state.contains(WidgetState::KEY_NAVIGATION))
        {
            UiEvent::Activate
        } else {
            UiEvent::Key {
                pressed,
                key_code,
                modifiers,
                text,
            }
        };
        if event == UiEvent::Activate
            && target_widget.is_some_and(|widget| is_popup_action_kind(widget.kind)
                && !widget.children().iter().any(|child| native_window.tree.get(*child)
                    .is_some_and(|child| child.kind == WidgetKind::Menu)))
            && native_window.opened_menu.is_some_and(|menu| menu_contains(&native_window.tree, menu, target.widget))
        {
            if let Some(menu) = native_window.opened_menu.take() {
                native_window.focused = Some(menu);
            }
            native_window.opened_submenus.clear();
            native_window.popup_anchor = None;
        }
        text_menu::restore_focus(native_window, target.widget, &event);
        Ok(Some(UiEventEnvelope {
            app,
            window,
            target: target.widget,
            event,
        }))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TreeNavigationAction {
    Toggle,
    Focus(WidgetId),
}

fn tree_navigation_action(
    tree: &RetainedTree,
    geometry: &std::collections::BTreeMap<WidgetId, Rect>,
    current: WidgetId,
    key_code: u32,
) -> Option<TreeNavigationAction> {
    let widget = tree.get(current)?;
    if widget.kind != WidgetKind::TreeItem {
        return None;
    }
    let children = widget.children().iter().copied().filter(|child| {
        tree.get(*child)
            .is_some_and(|candidate| candidate.kind == WidgetKind::TreeItem)
    });
    let has_children = children.clone().next().is_some();
    match key_code {
        37 | 113 if has_children && widget.state.contains(WidgetState::CHECKED) => {
            Some(TreeNavigationAction::Toggle)
        }
        37 | 113 => widget.parent.and_then(|parent| {
            tree.get(parent)
                .is_some_and(|candidate| candidate.kind == WidgetKind::TreeItem)
                .then_some(TreeNavigationAction::Focus(parent))
        }),
        39 | 114 if has_children && !widget.state.contains(WidgetState::CHECKED) => {
            Some(TreeNavigationAction::Toggle)
        }
        39 | 114 => children
            .filter(|child| geometry.contains_key(child))
            .next()
            .map(TreeNavigationAction::Focus),
        38 | 40 | 111 | 116 => {
            navigation_neighbor(tree, geometry, current, key_code).map(TreeNavigationAction::Focus)
        }
        _ => None,
    }
}

fn toggle_tree_item(window: &mut NativeWindow, id: WidgetId) -> Result<bool, NativeUiError> {
    let Some(widget) = window.tree.get(id) else {
        return Ok(false);
    };
    if !tree_item_expandable(&window.tree, id) {
        return Ok(false);
    }
    let currently_expanded = widget.state.contains(WidgetState::CHECKED);
    let application_expanded = window
        .tree_expansion
        .get(&id)
        .map_or(currently_expanded, |expansion| {
            expansion.application_expanded
        });
    let user_expanded = !currently_expanded;
    let state = WidgetState::from_bits_retain(widget.state.bits() ^ WidgetState::CHECKED.bits());
    window.tree.apply(&[Patch::Update {
        id,
        state: Some(state),
        text: ValueChange::Keep,
        icon: ValueChange::Keep,
    }])?;
    if user_expanded == application_expanded {
        window.tree_expansion.remove(&id);
    } else {
        window.tree_expansion.insert(
            id,
            TreeExpansionOverride {
                application_expanded,
                user_expanded,
            },
        );
    }
    Ok(true)
}

pub(super) fn tree_item_expandable(tree: &RetainedTree, id: WidgetId) -> bool {
    tree.get(id).is_some_and(|widget| {
        widget.kind == WidgetKind::TreeItem
            && widget.children().iter().any(|child| {
                tree.get(*child)
                    .is_some_and(|candidate| candidate.kind == WidgetKind::TreeItem)
            })
    })
}

fn tree_disclosure_hit(
    tree: &RetainedTree,
    id: WidgetId,
    row: Rect,
    point: Point,
    theme: &Theme,
) -> bool {
    if !tree_item_expandable(tree, id) || !row.contains(point) {
        return false;
    }
    let left = row
        .origin
        .x
        .saturating_add_unsigned(u32::from(theme.spacing.control_x));
    let right = left.saturating_add_unsigned(u32::from(theme.metrics.icon_small));
    point.x >= left && point.x < right
}

fn current_text_edit(
    current: Option<TextEditState>,
    widget: WidgetId,
    value: &str,
) -> TextEditState {
    current.filter(|edit| edit.widget == widget).map_or(
        TextEditState {
            widget,
            anchor: value.len(),
            caret: value.len(),
        },
        |mut edit| {
            edit.anchor = clamp_text_boundary(value, edit.anchor);
            edit.caret = clamp_text_boundary(value, edit.caret);
            edit
        },
    )
}

fn previous_text_boundary(value: &str, position: usize) -> usize {
    value[..clamp_text_boundary(value, position)]
        .char_indices()
        .next_back()
        .map_or(0, |(index, _)| index)
}

fn next_text_boundary(value: &str, position: usize) -> usize {
    let position = clamp_text_boundary(value, position);
    value[position..]
        .char_indices()
        .nth(1)
        .map_or(value.len(), |(offset, _)| position + offset)
}

fn previous_word_boundary(value: &str, position: usize) -> usize {
    let mut cursor = clamp_text_boundary(value, position);
    while cursor > 0 {
        let previous = previous_text_boundary(value, cursor);
        if !value[previous..cursor].chars().all(char::is_whitespace) {
            break;
        }
        cursor = previous;
    }
    while cursor > 0 {
        let previous = previous_text_boundary(value, cursor);
        if value[previous..cursor].chars().all(char::is_whitespace) {
            break;
        }
        cursor = previous;
    }
    cursor
}

fn next_word_boundary(value: &str, position: usize) -> usize {
    let mut cursor = clamp_text_boundary(value, position);
    while cursor < value.len() {
        let next = next_text_boundary(value, cursor);
        if value[cursor..next].chars().all(char::is_whitespace) {
            break;
        }
        cursor = next;
    }
    while cursor < value.len() {
        let next = next_text_boundary(value, cursor);
        if !value[cursor..next].chars().all(char::is_whitespace) {
            break;
        }
        cursor = next;
    }
    cursor
}

fn text_caret_from_pointer(
    value: &str,
    rect: Rect,
    pointer_x: i32,
    has_icon: bool,
    theme: &Theme,
    measured: Option<u32>,
) -> usize {
    if let Some(caret) = measured { return value.char_indices().nth(caret as usize).map_or(value.len(), |(byte,_)|byte); }
    let inset = u32::from(theme.spacing.control_x).saturating_add(if has_icon {
        u32::from(theme.metrics.icon_small).saturating_add(u32::from(theme.spacing.unit))
    } else {
        0
    });
    let x = pointer_x
        .saturating_sub(rect.origin.x)
        .saturating_sub_unsigned(inset)
        .max(0) as u32;
    let mut best = 0;
    for (index, _) in value.char_indices().skip(1) {
        if super::layout::estimated_text_width(&value[..index], theme) > x {
            break;
        }
        best = index;
    }
    if super::layout::estimated_text_width(value, theme) <= x {
        value.len()
    } else {
        best
    }
}

#[derive(Clone, Copy)]
pub(super) struct EditorVisualLine {
    pub(super) start: usize,
    pub(super) length: usize,
}

fn text_editor_selection_from_pointer_measured(
    value: &str, rect: Rect, point: Point, modifiers: u16, theme: &Theme,
    measured: Option<u32>,
) -> Option<(u32, u32)> {
    const RENDERER_INSET: u32 = 4;
    const GUTTER_PADDING: u32 = 12;
    let presentation = EditorText::parse(value)?;
    if let Some(caret) = measured {
        let characters = presentation.document.chars().collect::<Vec<_>>();
        let caret = (caret as usize).min(characters.len());
        if modifiers & DOUBLE_CLICK_MODIFIER != 0 && !characters.is_empty() {
            let (start, end) = editor_word_range(&characters, caret.min(characters.len() - 1), 0, characters.len());
            return Some((start as u32, end as u32));
        }
        let anchor = if modifiers & 1 != 0 { presentation.anchor.unwrap_or(caret) } else { caret };
        return Some((anchor as u32, caret as u32));
    }
    let rich_rect=rect.inset(rice_ui::Insets {left:u32::from(theme.spacing.control_x),right:u32::from(theme.spacing.control_x),top:u32::from(theme.spacing.control_y),bottom:u32::from(theme.spacing.control_y)});
    if let Some(caret)=office_editor_hit(&presentation, rich_rect, point).or_else(||rich_attribute_hit(&presentation,rich_rect,point)) {
        let characters=presentation.document.chars().collect::<Vec<_>>();
        let caret=caret.min(characters.len());
        if modifiers & DOUBLE_CLICK_MODIFIER != 0 && !characters.is_empty() {
            let (a,b)=editor_word_range(&characters,caret.min(characters.len()-1),0,characters.len());
            return Some((u32::try_from(a).ok()?,u32::try_from(b).ok()?));
        }
        let anchor=if modifiers & 1 != 0 {presentation.anchor.unwrap_or(caret)} else {caret};
        return Some((u32::try_from(anchor).ok()?,u32::try_from(caret).ok()?));
    }
    let characters = presentation.document.chars().collect::<Vec<_>>();
    let logical_lines = editor_logical_lines(&characters);
    let size = u32::from(presentation.font_size.unwrap_or(theme.typography.body_px)).max(8);
    // Paper's fixed viewport uses the bundled face's snapped 0.6 em
    // advance. Other editor clients retain their established fallback.
    let glyph_advance_milli = if presentation.viewport.is_some() {
        size.saturating_mul(600).saturating_add(500) / 1_000 * 1_000
    } else { size.saturating_mul(615).max(1_000) };
    let line_height = size
        .saturating_mul(135)
        .div_ceil(100)
        .max(size.saturating_add(2));
    // The canvas editor owns its four-pixel content inset. Unlike native text
    // fields, it does not apply the theme's control padding around `rect`.
    // Subtracting both here made wrapping diverge from the painted lines and
    // could map a click on a later word to a word on the preceding wrap.
    let text_rect = rect;
    let gutter_width = if presentation.line_numbers {
        let last_line = presentation
            .line_number_start
            .saturating_add(logical_lines.len().saturating_sub(1))
            .max(1);
        let digits = presentation.viewport.map_or(last_line.ilog10().saturating_add(1), |v| v.digits);
        glyph_advance_milli
            .saturating_mul(digits.saturating_add(1))
            .div_ceil(1_000)
            .saturating_add(GUTTER_PADDING)
    } else {
        0
    };
    let available = text_rect
        .size
        .width
        .saturating_sub(RENDERER_INSET.saturating_mul(2))
        .saturating_sub(gutter_width)
        .max(1);
    let capacity = usize::try_from(available.saturating_mul(1_000) / glyph_advance_milli)
        .unwrap_or(usize::MAX)
        .max(1);
    let mut visual_lines = Vec::new();
    for (start, end) in logical_lines {
        wrap_editor_line(
            &characters,
            start,
            end,
            presentation.wrap,
            capacity,
            &mut visual_lines,
        );
    }
    let maximum_lines = usize::try_from(
        text_rect
            .size
            .height
            .saturating_sub(RENDERER_INSET.saturating_mul(2))
            / line_height,
    )
    .unwrap_or(usize::MAX)
    .max(1);
    let displayed_caret = presentation.caret.unwrap_or_default().min(characters.len());
    let caret_line = visual_lines
        .iter()
        .enumerate()
        .find_map(|(index, line)| {
            let end = line.start.saturating_add(line.length);
            (displayed_caret >= line.start
                && (displayed_caret < end
                    || (displayed_caret == end
                        && (presentation.viewport.is_some_and(|v| v.caret_upstream) || index == visual_lines.len().saturating_sub(1)
                            || visual_lines[index.saturating_add(1)].start > displayed_caret))))
                .then_some(index)
        })
        .unwrap_or_default();
    let first_line = presentation.viewport.map_or_else(|| caret_line.saturating_sub(maximum_lines.saturating_sub(1)), |v| (v.offset / line_height) as usize);
    let visible_row = usize::try_from(
        point
            .y
            .saturating_sub(text_rect.origin.y)
            .saturating_sub_unsigned(RENDERER_INSET)
            .saturating_add_unsigned(presentation.viewport.map_or(0, |v| v.offset % line_height))
            .max(0)
            .cast_unsigned()
            / line_height,
    )
    .unwrap_or(usize::MAX);
    let row = first_line
        .saturating_add(visible_row)
        .min(visual_lines.len().saturating_sub(1));
    let line = *visual_lines.get(row)?;
    let text_x = text_rect
        .origin
        .x
        .saturating_add_unsigned(RENDERER_INSET.saturating_add(gutter_width));
    let pointer_advance_milli = point
        .x
        .saturating_sub(text_x)
        .max(0)
        .cast_unsigned()
        .saturating_mul(1_000);
    if modifiers & DOUBLE_CLICK_MODIFIER != 0 && line.length > 0 {
        let column = usize::try_from(pointer_advance_milli / glyph_advance_milli)
            .unwrap_or(usize::MAX)
            .min(line.length.saturating_sub(1));
        let (anchor, caret) = editor_word_range(
            &characters,
            line.start.saturating_add(column),
            line.start,
            line.start.saturating_add(line.length),
        );
        return Some((u32::try_from(anchor).ok()?, u32::try_from(caret).ok()?));
    }
    // Choose the closest caret boundary, matching native text editors rather
    // than always flooring to the character before the click.
    let column = usize::try_from(
        pointer_advance_milli.saturating_add(glyph_advance_milli / 2) / glyph_advance_milli,
    )
    .unwrap_or(usize::MAX)
    .min(line.length);
    let caret = u32::try_from(line.start.saturating_add(column)).ok()?;
    let anchor = if modifiers & 1 != 0 {
        u32::try_from(presentation.anchor.unwrap_or(usize::try_from(caret).ok()?)).ok()?
    } else {
        caret
    };
    Some((anchor, caret))
}

// Same bounded character advances as richAttributeLayout in the canvas renderer.
fn rich_attribute_hit(p:&EditorText<'_>,rect:Rect,point:Point)->Option<usize> {
    if !p.language.starts_with("rich"){return None;}
    let chars=p.document.chars().collect::<Vec<_>>();
    let ranges=p.language.split('~').filter_map(|t|{
        let kind=t.chars().next()?;let value=t.get(1..)?;let (span,value)=value.split_once(',').unwrap_or((value,""));let (a,b)=span.split_once('-')?;
        Some((kind,a.parse::<usize>().ok()?,b.parse::<usize>().ok()?,value))
    }).collect::<Vec<_>>();
    let zoom=f64::from(p.font_size.unwrap_or(12))/12.0;let gutter=if p.line_numbers {(p.line_number_start.saturating_add(chars.iter().filter(|c|**c=='\n').count()).to_string().len()+1) as f64*8.0+12.0}else{0.0};let available=(f64::from(rect.size.width.saturating_sub(8).max(1))/zoom-gutter).max(1.0);
    let mut rows=Vec::<(usize,usize,f64,f64,Vec<(usize,f64,f64)>,f64)>::new();let mut start=0;let mut y=0.0;
    loop {
        let end=(start..chars.len()).find(|i|chars[*i]=='\n').unwrap_or(chars.len());
        let active=|r:&&(char,usize,usize,&str)|r.1<=start&&r.2>=start;
        let indent=if ranges.iter().filter(active).any(|r|matches!(r.0,'B'|'N'|'Q')){28.0}else{0.0};
        let align=ranges.iter().filter(active).rev().find(|r|matches!(r.0,'A'|'C'|'R'|'J')).map(|r|r.0);
        let mut at=start;
        loop {
            let from=at;let mut x=indent;let mut height=18.0_f64;let mut glyphs=Vec::new();
            while at<end {
                let size=ranges.iter().rev().find(|r|r.0=='f'&&r.1<=at&&r.2>at).and_then(|r|r.3.parse::<u32>().ok()).unwrap_or(12).clamp(6,128);
                let advance=rice_ui::editor_glyph_advance(chars[at],f64::from(size));
                if !glyphs.is_empty() && p.wrap!=rice_ui::EditorWrap::None && x+advance>available {
                    if p.wrap==rice_ui::EditorWrap::Word {
                        if let Some(space)=glyphs.iter().rposition(|g: &(usize,f64,f64)|chars[g.0].is_whitespace()) {
                            at=glyphs[space].0+1;glyphs.truncate(space+1);
                            x=glyphs.last().map_or(indent,|g|g.1+g.2);
                            height=18.0;
                            for g in &glyphs {
                                let size=ranges.iter().rev().find(|r|r.0=='f'&&r.1<=g.0&&r.2>g.0).and_then(|r|r.3.parse::<u32>().ok()).unwrap_or(12).clamp(6,128);
                                height=height.max((f64::from(size)*1.5).ceil());
                            }
                        }
                    }
                    break;
                }
                glyphs.push((at,x,advance));x+=advance;height=height.max((f64::from(size)*1.5).ceil());at+=1;
            }
            if align==Some('J')&&at<end {
                if let Some(last)=glyphs.iter().rposition(|g|!chars[g.0].is_whitespace()) {
                    let spaces=glyphs.iter().take(last).filter(|g|chars[g.0]==' ').count();
                    if spaces>0 {let extra=(available-glyphs[last].1-glyphs[last].2).max(0.0)/spaces as f64;let mut shift=0.0;
                        for (i,g) in glyphs.iter_mut().enumerate(){g.1+=shift;if i<last&&chars[g.0]==' ' {g.2+=extra;shift+=extra;}}
                    }
                }
            }
            let offset=match align {Some('C')=>((available-x)/2.0).max(0.0),Some('R')=>(available-x).max(0.0),_=>0.0};
            for g in &mut glyphs {g.1+=offset;}
            rows.push((from,at,y,height,glyphs,indent+offset));y+=height;if at>=end {break;}
        }
        if end==chars.len(){break;}start=end+1;
    }
    let caret=p.caret.unwrap_or(0);
    let row=rows.iter().enumerate().find(|(i,r)|caret>=r.0&&(caret<r.1||(caret==r.1&&(*i==rows.len()-1||rows[*i+1].0>caret)))).map(|(_,r)|r).or_else(||rows.last())?;
    let scroll=(row.2+row.3-f64::from(rect.size.height.saturating_sub(8))/zoom).max(0.0);
    let x=f64::from(point.x-rect.origin.x-4)/zoom-gutter;let y=f64::from(point.y-rect.origin.y-4)/zoom+scroll;
    let row=rows.iter().find(|r|y<r.2+r.3).or_else(||rows.last())?;
    Some(row.4.iter().find(|g|x<g.1+g.2/2.0).map_or(row.1,|g|g.0))
}

fn office_editor_hit(presentation: &EditorText<'_>, rect: Rect, point: Point) -> Option<usize> {
    let header=presentation.language.split('~').find_map(|t|t.strip_prefix('o'))?;
    let fields=header.split(',').map(str::parse::<u32>).collect::<Result<Vec<_>,_>>().ok()?;
    if fields.len()!=4 || !(200..=4000).contains(&fields[0]) {return None;}
    let scale=(f64::from(rect.size.width.saturating_sub(16).max(1))/f64::from(fields[0])).min(1.0)*f64::from(presentation.font_size.unwrap_or(12))/12.0;
    let page_left = ((f64::from(rect.size.width) - f64::from(fields[0]) * scale) / 2.0).max(8.0);
    let x=(f64::from(point.x-rect.origin.x)-page_left)/scale;
    let y=f64::from(point.y-rect.origin.y-8)/scale+f64::from(fields[2]);
    // Office shaping exports only valid grapheme caret stops. Prefer those exact
    // positions to character-cell interpolation; offsets are viewport scalars.
    if let Some(caret) = office_shaped_caret_hit(presentation, x, y) { return Some(caret); }
    let mut closest=None;let mut distance=f64::MAX;
    for token in presentation.language.split('~').filter_map(|t|t.strip_prefix('l')) {
        let values=token.split(',').take(5).map(str::parse::<u32>).collect::<Result<Vec<_>,_>>().ok()?;
        if values.len()!=5 || values[0]>values[1] || values[4]==0 {continue;}
        let advance=f64::from((values[4]*3/5).max(1));
        let left=f64::from(values[2]);let top=f64::from(values[3]);
        let right=left+f64::from(values[1]-values[0])*advance;
        let bottom=top+f64::from(values[4])*1.5;
        let dx=if x<left {left-x} else {(x-right).max(0.0)};
        let dy=if y<top {top-y} else {(y-bottom).max(0.0)};
        let score=dy*10000.0+dx;
        if score<distance {
            distance=score;let column=((x-left)/advance).round().max(0.0) as usize;
            closest=Some(values[0] as usize+column.min((values[1]-values[0]) as usize));
        }
    }
    closest
}

fn office_shaped_caret_hit(presentation: &EditorText<'_>, x: f64, y: f64) -> Option<usize> {
    const MAX_STOPS: usize = 16_384;
    if presentation.language.len() > 65_536 || !x.is_finite() || !y.is_finite() { return None; }
    let length = presentation.document.chars().count();
    let mut best: Option<(f64, f64, f64, u8, usize)> = None;
    let mut editing_line = None;
    let mut count = 0;
    for entry in presentation.language.split('~') {
        if let Some(line) = entry.strip_prefix('e') {
            let fields: Vec<_> = line.split(',').map(str::parse::<f64>).collect::<Result<Vec<_>,_>>().unwrap_or_default();
            editing_line = (fields.len() == 2 && fields.iter().all(|v| v.is_finite() && v.abs() <= 1_000_000_000.0)
                && fields[1] > fields[0] && fields[1] - fields[0] <= 65_536.0).then(|| (fields[0], fields[1]));
            continue;
        }
        let compact = entry.starts_with('d');
        let Some(token) = entry.strip_prefix('t').or_else(|| entry.strip_prefix('d')) else { continue; };
        count += 1;
        if count > MAX_STOPS { return None; }
        if token.len() > 160 { continue; }
        let mut fields = token.split(',');
        let Some(offset) = fields.next().and_then(|v| v.parse::<usize>().ok()).filter(|v| *v <= length) else { continue; };
        let Some(left) = fields.next().and_then(|v| v.parse::<f64>().ok()).filter(|v| v.is_finite() && v.abs() <= 10_000_000.0) else { continue; };
        let (top,bottom) = if compact {
            let Some(line) = editing_line else { continue; }; line
        } else {
            let Some(top) = fields.next().and_then(|v| v.parse::<f64>().ok()).filter(|v| v.is_finite() && v.abs() <= 1_000_000_000.0) else { continue; };
            let Some(bottom) = fields.next().and_then(|v| v.parse::<f64>().ok()).filter(|v| v.is_finite() && *v >= top && *v - top <= 65_536.0) else { continue; };
            (top,bottom)
        };
        let Some(affinity) = fields.next().and_then(|v| v.parse::<u8>().ok()).filter(|v| *v <= 1) else { continue; };
        if fields.next().is_some() { continue; }
        let dy = if y < top { top - y } else { (y - bottom).max(0.0) };
        let center = (y - (top + bottom) / 2.0).abs();
        let dx = (x - left).abs();
        let score = (dy, center, dx, 1 - affinity, offset);
        if best.as_ref().is_none_or(|old| score < *old) { best = Some(score); }
    }
    best.map(|(_, _, _, _, offset)| offset)
}

fn editor_word_range(
    characters: &[char],
    at: usize,
    line_start: usize,
    line_end: usize,
) -> (usize, usize) {
    let Some(character) = characters.get(at).copied() else {
        return (at, at);
    };
    let class = editor_word_class(character);
    let mut start = at;
    while start > line_start && editor_word_class(characters[start - 1]) == class {
        start -= 1;
    }
    let mut end = at.saturating_add(1);
    while end < line_end.min(characters.len()) && editor_word_class(characters[end]) == class {
        end += 1;
    }
    (start, end)
}

fn editor_word_class(character: char) -> u8 {
    if character.is_whitespace() {
        0
    } else if character.is_alphanumeric() || character == '_' {
        1
    } else {
        2
    }
}

fn editor_logical_lines(characters: &[char]) -> Vec<(usize, usize)> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut at = 0;
    while at < characters.len() {
        if matches!(characters[at], '\r' | '\n') {
            lines.push((start, at));
            if characters[at] == '\r' && characters.get(at + 1) == Some(&'\n') {
                at = at.saturating_add(1);
            }
            at = at.saturating_add(1);
            start = at;
        } else {
            at = at.saturating_add(1);
        }
    }
    lines.push((start, characters.len()));
    lines
}

pub(super) fn wrap_editor_line(
    characters: &[char],
    start: usize,
    end: usize,
    wrap: EditorWrap,
    capacity: usize,
    output: &mut Vec<EditorVisualLine>,
) {
    if wrap == EditorWrap::None || end.saturating_sub(start) <= capacity {
        output.push(EditorVisualLine {
            start,
            length: end.saturating_sub(start),
        });
        return;
    }
    if wrap == EditorWrap::Character {
        for segment in (start..end).step_by(capacity) {
            output.push(EditorVisualLine {
                start: segment,
                length: end.saturating_sub(segment).min(capacity),
            });
        }
        return;
    }
    let mut current_start = start;
    let mut current_length = 0_usize;
    let mut piece_start = start;
    while piece_start < end {
        let whitespace = characters[piece_start].is_whitespace();
        let mut piece_end = piece_start.saturating_add(1);
        while piece_end < end && characters[piece_end].is_whitespace() == whitespace {
            piece_end = piece_end.saturating_add(1);
        }
        let piece_length = piece_end.saturating_sub(piece_start);
        if current_length > 0 && current_length.saturating_add(piece_length) > capacity {
            output.push(EditorVisualLine {
                start: current_start,
                length: current_length,
            });
            current_start = piece_start;
            current_length = piece_length;
        } else {
            current_length = current_length.saturating_add(piece_length);
        }
        piece_start = piece_end;
    }
    output.push(EditorVisualLine {
        start: current_start,
        length: current_length,
    });
}

fn navigation_neighbor(
    tree: &RetainedTree,
    geometry: &std::collections::BTreeMap<WidgetId, Rect>,
    current: WidgetId,
    key_code: u32,
) -> Option<WidgetId> {
    let widget = tree.get(current)?;
    if !widget.state.contains(WidgetState::KEY_NAVIGATION) {
        return None;
    }
    let collection = navigation_collection(tree, current)?;
    let collection_widget = tree.get(collection)?;
    let icon_grid = collection_widget.kind == WidgetKind::IconGrid
        && super::layout_tokens::is_file_grid(collection_widget.text.as_deref());
    let cell_rect = |mut id| {
        // File thumbnails focus their caption button, ordinary icons focus
        // the whole tile. Directional movement belongs to the grid cell in
        // both cases, regardless of caption or thumbnail geometry.
        if icon_grid {
            while let Some(parent) = tree.get(id)?.parent {
                if parent == collection { break; }
                id = parent;
            }
        }
        geometry.get(&id).copied()
    };
    let current_rect = cell_rect(current)?;
    let current_x = i64::from(current_rect.origin.x) + i64::from(current_rect.size.width) / 2;
    let current_y = i64::from(current_rect.origin.y) + i64::from(current_rect.size.height) / 2;
    let direction = match key_code {
        37 | 113 => (-1_i64, 0_i64),
        38 | 111 => (0, -1),
        39 | 114 => (1, 0),
        40 | 116 => (0, 1),
        _ => return None,
    };
    geometry
        .iter()
        .filter_map(|(candidate, _)| {
            if *candidate == current
                || navigation_collection(tree, *candidate) != Some(collection)
                || !tree.get(*candidate).is_some_and(|widget| {
                    widget.state.contains(WidgetState::KEY_NAVIGATION)
                        && widget.state.contains(WidgetState::ENABLED)
                })
            {
                return None;
            }
            let rect = cell_rect(*candidate)?;
            let x = i64::from(rect.origin.x) + i64::from(rect.size.width) / 2;
            let y = i64::from(rect.origin.y) + i64::from(rect.size.height) / 2;
            let dx = x - current_x;
            let dy = y - current_y;
            if icon_grid && direction.0 != 0 && dy != 0 {
                return None;
            }
            let (forward, cross) = if direction.0 != 0 {
                (dx * direction.0, dy.unsigned_abs())
            } else {
                (dy * direction.1, dx.unsigned_abs())
            };
            (forward > 0).then_some((*candidate, (forward as u64, cross)))
        })
        .min_by_key(|(_, score)| *score)
        .map(|(candidate, _)| candidate)
}

fn navigation_collection(tree: &RetainedTree, mut current: WidgetId) -> Option<WidgetId> {
    loop {
        let widget = tree.get(current)?;
        if matches!(
            widget.kind,
            WidgetKind::IconGrid | WidgetKind::List | WidgetKind::Table | WidgetKind::Tree
        ) {
            return Some(current);
        }
        current = widget.parent?;
    }
}

fn enclosing_scroll_view(tree: &RetainedTree, mut widget: WidgetId) -> Option<WidgetId> {
    loop {
        let retained = tree.get(widget)?;
        if retained.kind == WidgetKind::ScrollView && retained.state.contains(WidgetState::ENABLED)
        {
            return Some(widget);
        }
        widget = retained.parent?;
    }
}

fn scroll_view_at_point(
    tree: &RetainedTree,
    geometry: &BTreeMap<WidgetId, Rect>,
    point: Point,
) -> Option<WidgetId> {
    geometry
        .iter()
        .filter_map(|(id, rect)| {
            tree.get(*id)
                .is_some_and(|widget| {
                    widget.kind == WidgetKind::ScrollView
                        && widget.state.contains(WidgetState::ENABLED)
                        && rect.contains(point)
                })
                .then_some((*id, rect.size.width.saturating_mul(rect.size.height)))
        })
        .min_by_key(|(_, area)| *area)
        .map(|(id, _)| id)
}

fn next_focus(
    focusable: &[WidgetId],
    current: Option<WidgetId>,
    reverse: bool,
) -> Option<WidgetId> {
    if focusable.is_empty() {
        return None;
    }
    let current =
        current.and_then(|candidate| focusable.iter().position(|widget| *widget == candidate));
    let index = match (current, reverse) {
        (Some(0), true) | (None, true) => focusable.len() - 1,
        (Some(index), true) => index - 1,
        (Some(index), false) => (index + 1) % focusable.len(),
        (None, false) => 0,
    };
    focusable.get(index).copied()
}

pub(super) fn contextual_menu(tree: &RetainedTree, target: Option<WidgetId>) -> Option<WidgetId> {
    let mut ancestor = target;
    while let Some(id) = ancestor {
        let widget = tree.get(id)?;
        if let Some(menu) = widget.children().iter().copied().find_map(|child| {
            let bar = tree.get(child)?;
            (bar.kind == WidgetKind::MenuBar && bar.text.as_deref() == Some("context"))
                .then(|| {
                    bar.children().iter().copied().find(|menu| {
                        tree.get(*menu)
                            .is_some_and(|menu| menu.kind == WidgetKind::Menu)
                    })
                })
                .flatten()
        }) {
            return Some(menu);
        }
        ancestor = widget.parent;
    }
    // A bound context menu belongs only to its owner subtree. The root scan
    // serves requests on empty window space, never unrelated target controls.
    if target.is_some() { return None; }
    let mut pending = tree.roots().to_vec();
    while let Some(id) = pending.pop() {
        let widget = tree.get(id)?;
        if widget.kind == WidgetKind::MenuBar && widget.id.get() != text_menu::BAR
            && widget.text.as_deref() == Some("context") {
            return widget.children().iter().copied().find(|child| {
                tree.get(*child)
                    .is_some_and(|child| child.kind == WidgetKind::Menu)
            });
        }
        pending.extend(widget.children().iter().copied());
    }
    None
}

const fn is_popup_action_kind(kind: WidgetKind) -> bool {
    matches!(
        kind,
        WidgetKind::MenuItem | WidgetKind::CheckBox | WidgetKind::RadioButton
    )
}

// Reusable SDK compound-gallery contract. Only an explicit bounded metadata
// token enables this behavior; labels and application names never do.
fn popup_grid_columns(widget: &Widget) -> Option<usize> {
    if widget.kind != WidgetKind::Column || !widget.state.contains(WidgetState::ENABLED) { return None; }
    let mut tokens = widget.text.as_deref()?.split(':');
    if tokens.next()? != "panel" { return None; }
    let tokens = tokens.collect::<Vec<_>>();
    if !tokens.contains(&"menu-grid") { return None; }
    tokens.iter().find_map(|token| token.strip_prefix("columns=")?.parse::<usize>().ok()).filter(|columns| (1..=32).contains(columns))
}

fn popup_grid_owner(tree: &RetainedTree, target: WidgetId) -> Option<(WidgetId, usize)> {
    let mut current = Some(target);
    for _ in 0..16 {
        let item = tree.get(current?)?;
        if let Some(columns) = popup_grid_columns(item) { return Some((item.id, columns)); }
        if item.kind == WidgetKind::Menu { return None; }
        current = item.parent;
    }
    None
}

fn popup_grid_destination(tree: &RetainedTree, menu: WidgetId, focused: Option<WidgetId>, key: u32) -> Option<WidgetId> {
    let grid = if let Some(owner) = focused.and_then(|id| popup_grid_owner(tree, id)) {
        menu_contains(tree, menu, owner.0).then_some(owner)?
    } else {
        if focused.is_some_and(|id| id != menu && menu_contains(tree, menu, id)) { return None; }
        let mut pending = vec![menu];
        let mut found = None;
        for _ in 0..4096 {
            let Some(id) = pending.pop() else { break; };
            let Some(item) = tree.get(id) else { continue; };
            if let Some(columns) = popup_grid_columns(item) { found = Some((id, columns)); break; }
            if item.state.contains(WidgetState::ENABLED) && (id == menu || item.kind != WidgetKind::Menu) {
                pending.extend(item.children().iter().rev().copied());
            }
        }
        found?
    };
    let mut pending = vec![grid.0];
    let mut cells = Vec::new();
    for _ in 0..4096 {
        let Some(id) = pending.pop() else { break; };
        let Some(item) = tree.get(id) else { continue; };
        if item.kind == WidgetKind::MenuItem { cells.push(id); }
        else if item.kind != WidgetKind::Menu { pending.extend(item.children().iter().rev().copied()); }
        if cells.len() > 256 { return None; }
    }
    let available = |id| tree.get(id).is_some_and(|item| item.state.contains(WidgetState::ENABLED));
    let current = focused.and_then(|id| cells.iter().position(|cell| *cell == id));
    let Some(index) = current else { return cells.iter().copied().find(|id| available(*id)); };
    let columns = grid.1;
    let last = cells.len().checked_sub(1)?;
    let destination = match key {
        110 => 0,
        115 => last,
        37 | 113 => if index % columns == 0 { index } else { index - 1 },
        39 | 114 => if index % columns == columns - 1 { index } else { (index + 1).min(last) },
        38 | 111 => index.checked_sub(columns).unwrap_or(index),
        40 | 116 => if index + columns > last { index } else { index + columns },
        _ => return None,
    };
    cells.get(destination).copied().filter(|id| available(*id)).or(focused.filter(|id| available(*id)))
}

fn menu_contains(tree: &RetainedTree, menu: WidgetId, target: WidgetId) -> bool {
    let mut current = Some(target);
    while let Some(id) = current {
        if id == menu {
            return true;
        }
        current = tree.get(id).and_then(|widget| widget.parent);
    }
    false
}

fn menu_chain_depth(
    opened_menu: Option<WidgetId>,
    opened_submenus: &[WidgetId],
    owner: WidgetId,
) -> Option<usize> {
    if opened_menu == Some(owner) {
        return Some(0);
    }
    opened_submenus
        .iter()
        .position(|menu| *menu == owner)
        .map(|index| index + 1)
}


pub(super) fn menu_keyboard_items(tree: &RetainedTree, menu: WidgetId) -> Vec<WidgetId> {
    tree.get(menu).map_or_else(Vec::new, |menu| menu.children().iter().copied().filter(|id| {
        tree.get(*id).is_some_and(|item| item.state.contains(WidgetState::ENABLED)
            && matches!(item.kind, WidgetKind::MenuItem | WidgetKind::Menu)
            && item.text.as_deref() != Some("menu:separator"))
    }).collect())
}

fn close_deepest_submenu(window: &mut NativeWindow) -> Option<WidgetId> {
    while let Some(submenu) = window.opened_submenus.pop() {
        if let Some(owner) = window.tree.get(submenu).and_then(|menu| menu.parent)
            && window.tree.get(owner).is_some_and(|item| item.kind == WidgetKind::MenuItem)
        {
            window.focused = Some(owner);
            window.hovered = None;
            return Some(owner);
        }
    }
    None
}

#[cfg(test)]
#[path = "popup_grid_tests.rs"]
mod popup_grid_tests;

#[cfg(test)]
#[path = "office_hit_tests.rs"]
mod office_hit_tests;
