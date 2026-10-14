//! Renderer-measured scalar hits. Validation precedes all focus/drag mutation.
use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EditorMeasurement {
    pub widget: WidgetId,
    pub caret: u32,
    pub text: String,
}

impl NativeUiHost {
    pub(crate) fn validate_editor_measurement(
        &self, app: AppId, window: WindowId, kind: PointerEventKind,
        point: Point, sample: &EditorMeasurement,
    ) -> bool {
        let Some(native) = self.windows.get(&(app, window)) else { return false; };
        if native.grid_resync_shadow.is_some() { return false; }
        let captured = matches!(kind, PointerEventKind::Move | PointerEventKind::Up)
            .then_some(native.text_drag).flatten().map(|drag| drag.widget);
        let target = captured.or_else(|| self.hit_test(app, window, point).map(|hit| hit.widget));
        if target != Some(sample.widget) { return false; }
        let Some(widget) = native.tree.get(sample.widget) else { return false; };
        if !widget.state.contains(rice_ui::WidgetState::ENABLED) { return false; }
        if super::layout::is_chat_composer(&native.tree, widget) {
            let Some(presentation) = rice_ui::EditorText::parse(&sample.text) else { return false; };
            return presentation.language == "plain"
                && presentation.wrap == rice_ui::EditorWrap::Word
                && !presentation.line_numbers
                && widget.text.as_deref() == Some(presentation.document)
                && sample.caret as usize <= presentation.document.chars().count();
        }
        if widget.text.as_deref() != Some(sample.text.as_str()) { return false; }
        if widget.kind == WidgetKind::TextField {
            return native.tree.document_font(sample.widget).is_some() && sample.caret as usize <= sample.text.chars().count();
        }
        if widget.kind != WidgetKind::TextEditor { return false; }
        let Some(presentation) = rice_ui::EditorText::parse(&sample.text) else { return false; };
        !presentation.language.starts_with("rich") && !presentation.language.starts_with("office")
            && sample.caret as usize <= presentation.document.chars().count()
    }
}
