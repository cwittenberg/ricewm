use core::fmt;
use std::{collections::{BTreeMap, BTreeSet}, error::Error};

use crate::{
    IconId, MAX_CHILDREN_PER_WIDGET, MAX_PATCH_BATCH, MAX_ROOTS, MAX_WIDGET_TEXT_BYTES,
    MAX_WIDGETS, Patch, ValueChange, Widget, WidgetId, WidgetSpec, WidgetState,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RetainedTree {
    widgets: BTreeMap<WidgetId, Widget>,
    roots: Vec<WidgetId>,
    size_observers: BTreeSet<WidgetId>,
    text_selection_observers: BTreeSet<WidgetId>,
    icon_selectors: BTreeMap<WidgetId,String>,
    document_fonts: BTreeMap<WidgetId, (crate::grid_frame::GridFont,u16,Option<crate::grid_frame::DocumentPaint>)>,
}

impl RetainedTree {
    pub fn icon_selector(&self,id:WidgetId)->Option<&str>{self.icon_selectors.get(&id).map(String::as_str)}

    #[must_use]
    pub fn len(&self) -> usize {
        self.widgets.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.widgets.is_empty()
    }

    #[must_use]
    pub fn roots(&self) -> &[WidgetId] {
        &self.roots
    }

    #[must_use]
    pub fn get(&self, id: WidgetId) -> Option<&Widget> {
        self.widgets.get(&id)
    }

    /// IDs subscribed to resolved native layout sizes (at most 64).
    pub fn size_observers(&self) -> impl Iterator<Item = WidgetId> + '_ {
        self.size_observers.iter().copied()
    }

    pub fn document_font(&self, id: WidgetId) -> Option<&crate::grid_frame::GridFont> { self.document_fonts.get(&id).map(|(font,_,_)|font) }
    pub fn document_paint(&self,id:WidgetId)->Option<&crate::grid_frame::DocumentPaint>{self.document_fonts.get(&id).and_then(|(_,_,paint)|paint.as_ref())}
    pub fn document_zoom(&self,id:WidgetId)->u16{self.document_fonts.get(&id).map_or(100,|(_,zoom,_)|*zoom)}

    pub fn observes_text_selection(&self, id: WidgetId) -> bool {
        self.text_selection_observers.contains(&id)
    }

    pub fn has_text_selection_observers(&self) -> bool {
        !self.text_selection_observers.is_empty()
    }

    /// Applies a batch atomically; rejection leaves the prior tree unchanged.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError`] for invariant or resource-bound violations.
    pub fn apply(&mut self, patches: &[Patch]) -> Result<(), TreeError> {
        let mut candidate = self.clone();
        if candidate.apply_for_grid_resync(patches)? { return Err(TreeError::GridBaseMismatch); }
        *self = candidate;
        Ok(())
    }

    /// Validate every structural patch while retaining a private recovery shadow.
    /// Only a grid base mismatch may be skipped; malformed data still rejects.
    pub fn apply_for_grid_resync(&mut self, patches: &[Patch]) -> Result<bool, TreeError> {
        if patches.len() > MAX_PATCH_BATCH {
            return Err(TreeError::LimitExceeded {
                resource: "patch batch",
                maximum: MAX_PATCH_BATCH,
            });
        }
        let mut candidate = self.clone();
        let mut base_mismatch = false;
        for patch in patches {
            match candidate.apply_one(patch) {
                Err(TreeError::GridBaseMismatch) => base_mismatch = true,
                other => other?,
            }
        }
        *self = candidate;
        Ok(base_mismatch)
    }

    fn apply_one(&mut self, patch: &Patch) -> Result<(), TreeError> {
        match patch {
            Patch::Create {
                widget,
                parent,
                index,
            } => self.create(widget, *parent, *index),
            Patch::Update {
                id,
                state,
                text,
                icon,
            } => self.update(*id, *state, text, icon),
            Patch::IconSelector {id,selector} => {
                if !self.widgets.contains_key(id){return Err(TreeError::MissingWidget(*id));}
                if let Some(value)=selector {if value.is_empty()||value.len()>320||!value.bytes().all(|b|b.is_ascii_alphanumeric()||b"-_.:+/@*".contains(&b)){return Err(TreeError::InvalidIconSelector);}self.icon_selectors.insert(*id,value.clone());}else{self.icon_selectors.remove(id);}Ok(())
            },
            Patch::DocumentFont { id, font } => {
                let widget = self.widgets.get(id).ok_or(TreeError::MissingWidget(*id))?;
                if !matches!(widget.kind, crate::WidgetKind::TextField | crate::WidgetKind::TextEditor | crate::WidgetKind::Label | crate::WidgetKind::Button) { return Err(TreeError::InvalidDocumentFont); }
                if let Some(value) = font {
                    if value.len() > 1024 { return Err(TreeError::InvalidDocumentFont); }
                    let mut fields: serde_json::Map<String,serde_json::Value> = serde_json::from_str(value).map_err(|_|TreeError::InvalidDocumentFont)?;
                    let zoom = match fields.remove("zoom_percent") {
                        Some(value) => value.as_u64().filter(|zoom|(25..=400).contains(zoom)).ok_or(TreeError::InvalidDocumentFont)? as u16,
                        None => 100,
                    };
                    let paint = match fields.remove("paint") {
                        Some(value) => { if !value.is_object(){return Err(TreeError::InvalidDocumentFont);} let paint: crate::grid_frame::DocumentPaint = serde_json::from_value(value).map_err(|_|TreeError::InvalidDocumentFont)?;
                            if !paint.is_valid(){return Err(TreeError::InvalidDocumentFont);} Some(paint) },
                        None => None,
                    };
                    let parsed: crate::grid_frame::GridFont = serde_json::from_value(serde_json::Value::Object(fields)).map_err(|_| TreeError::InvalidDocumentFont)?;
                    if !parsed.is_valid() { return Err(TreeError::InvalidDocumentFont); }
                    self.document_fonts.insert(*id, (parsed,zoom,paint));
                } else { self.document_fonts.remove(id); }
                Ok(())
            },
            Patch::EditSelection { id, anchor, caret } => {
                let widget = self.widgets.get(id).ok_or(TreeError::MissingWidget(*id))?;
                let length = widget.text.as_deref().unwrap_or("").chars().count();
                if widget.kind != crate::WidgetKind::TextField || *anchor as usize > length || *caret as usize > length {
                    return Err(TreeError::InvalidTextSelection);
                }
                if self.text_selection_observers.len() >= 64 && !self.text_selection_observers.contains(id) {
                    return Err(TreeError::LimitExceeded { resource: "text selection observers", maximum: 64 });
                }
                self.text_selection_observers.insert(*id);
                Ok(())
            },
            Patch::ObserveSize { id, enabled } => {
                if !self.widgets.contains_key(id) { return Err(TreeError::MissingWidget(*id)); }
                if *enabled {
                    if self.size_observers.len() >= 64 && !self.size_observers.contains(id) {
                        return Err(TreeError::LimitExceeded { resource: "size observers", maximum: 64 });
                    }
                    self.size_observers.insert(*id);
                } else { self.size_observers.remove(id); }
                Ok(())
            },
            Patch::GridDamage { id, damage } => {
                let widget = self.widgets.get_mut(id).ok_or(TreeError::MissingWidget(*id))?;
                if widget.kind != crate::WidgetKind::VirtualGrid { return Err(TreeError::InvalidGridDamage); }
                let frame = crate::grid_frame::GridFrame::parse(widget.text.as_deref().unwrap_or(""))
                    .map_err(|_| TreeError::InvalidGridDamage)?;
                let damage = crate::grid_damage::GridDamage::parse(damage).map_err(|_| TreeError::InvalidGridDamage)?;
                let next = damage.apply(&frame).map_err(|error| match error {
                    crate::grid_damage::GridDamageError::BaseMismatch => TreeError::GridBaseMismatch,
                    crate::grid_damage::GridDamageError::Invalid => TreeError::InvalidGridDamage,
                })?;
                widget.text = Some(next.encode().map_err(|_| TreeError::InvalidGridDamage)?);
                Ok(())
            },
            Patch::Move { id, parent, index } => self.move_widget(*id, *parent, *index),
            Patch::Remove { id } => self.remove(*id),
            Patch::Clear => {
                self.widgets.clear();
                self.roots.clear();
                self.size_observers.clear();
                self.text_selection_observers.clear();
                self.document_fonts.clear();
                self.icon_selectors.clear();
                Ok(())
            }
        }
    }

    fn create(
        &mut self,
        spec: &WidgetSpec,
        parent: Option<WidgetId>,
        index: usize,
    ) -> Result<(), TreeError> {
        if self.widgets.contains_key(&spec.id) {
            return Err(TreeError::DuplicateWidget(spec.id));
        }
        if self.widgets.len() >= MAX_WIDGETS {
            return Err(TreeError::LimitExceeded {
                resource: "widgets",
                maximum: MAX_WIDGETS,
            });
        }
        validate_text(spec.text.as_deref(), spec.kind)?;
        self.validate_insertion(parent, index)?;
        self.insert_child(parent, index, spec.id);
        self.widgets.insert(
            spec.id,
            Widget {
                id: spec.id,
                kind: spec.kind,
                state: spec.state,
                text: spec.text.clone(),
                icon: spec.icon,
                parent,
                children: Vec::new(),
            },
        );
        Ok(())
    }

    fn update(
        &mut self,
        id: WidgetId,
        state: Option<WidgetState>,
        text: &ValueChange<String>,
        icon: &ValueChange<IconId>,
    ) -> Result<(), TreeError> {
        if let ValueChange::Set(text) = text {
            validate_text(Some(text), self.widgets.get(&id).ok_or(TreeError::MissingWidget(id))?.kind)?;
        }
        let widget = self
            .widgets
            .get_mut(&id)
            .ok_or(TreeError::MissingWidget(id))?;
        if let Some(state) = state {
            widget.state = state;
        }
        match text {
            ValueChange::Keep => {}
            ValueChange::Set(text) => widget.text = Some(text.clone()),
            ValueChange::Clear => widget.text = None,
        }
        match icon {
            ValueChange::Keep => {}
            ValueChange::Set(icon) => widget.icon = Some(*icon),
            ValueChange::Clear => widget.icon = None,
        }
        Ok(())
    }

    fn move_widget(
        &mut self,
        id: WidgetId,
        new_parent: Option<WidgetId>,
        mut new_index: usize,
    ) -> Result<(), TreeError> {
        let old_parent = self
            .widgets
            .get(&id)
            .ok_or(TreeError::MissingWidget(id))?
            .parent;
        if new_parent == Some(id) || self.is_descendant(new_parent, id) {
            return Err(TreeError::Cycle);
        }
        if old_parent == new_parent {
            let length = self.siblings(new_parent)?.len();
            if new_index > length {
                return Err(TreeError::InvalidIndex {
                    index: new_index,
                    length,
                });
            }
        } else {
            self.validate_insertion(new_parent, new_index)?;
        }

        let old_index = self
            .siblings(old_parent)?
            .iter()
            .position(|candidate| *candidate == id)
            .ok_or(TreeError::InconsistentTree)?;
        if old_parent == new_parent && old_index < new_index {
            new_index -= 1;
        }
        self.siblings_mut(old_parent)?.remove(old_index);
        self.insert_child(new_parent, new_index, id);
        self.widgets
            .get_mut(&id)
            .ok_or(TreeError::InconsistentTree)?
            .parent = new_parent;
        Ok(())
    }

    fn remove(&mut self, id: WidgetId) -> Result<(), TreeError> {
        let parent = self
            .widgets
            .get(&id)
            .ok_or(TreeError::MissingWidget(id))?
            .parent;
        let position = self
            .siblings(parent)?
            .iter()
            .position(|candidate| *candidate == id)
            .ok_or(TreeError::InconsistentTree)?;
        self.siblings_mut(parent)?.remove(position);

        let mut pending = vec![id];
        while let Some(current) = pending.pop() {
            let widget = self
                .widgets
                .remove(&current)
                .ok_or(TreeError::InconsistentTree)?;
            self.size_observers.remove(&current);
            self.document_fonts.remove(&current);
            self.icon_selectors.remove(&current);
            self.text_selection_observers.remove(&current);
            pending.extend(widget.children);
        }
        Ok(())
    }

    fn validate_insertion(&self, parent: Option<WidgetId>, index: usize) -> Result<(), TreeError> {
        let siblings = self.siblings(parent)?;
        let maximum = if parent.is_some() {
            MAX_CHILDREN_PER_WIDGET
        } else {
            MAX_ROOTS
        };
        if siblings.len() >= maximum {
            return Err(TreeError::LimitExceeded {
                resource: if parent.is_some() {
                    "children"
                } else {
                    "roots"
                },
                maximum,
            });
        }
        if index > siblings.len() {
            return Err(TreeError::InvalidIndex {
                index,
                length: siblings.len(),
            });
        }
        Ok(())
    }

    fn is_descendant(&self, candidate: Option<WidgetId>, ancestor: WidgetId) -> bool {
        let mut current = candidate;
        while let Some(id) = current {
            if id == ancestor {
                return true;
            }
            current = self.widgets.get(&id).and_then(|widget| widget.parent);
        }
        false
    }

    fn siblings(&self, parent: Option<WidgetId>) -> Result<&[WidgetId], TreeError> {
        match parent {
            Some(parent) => self
                .widgets
                .get(&parent)
                .map(|widget| widget.children.as_slice())
                .ok_or(TreeError::MissingParent(parent)),
            None => Ok(&self.roots),
        }
    }

    fn siblings_mut(&mut self, parent: Option<WidgetId>) -> Result<&mut Vec<WidgetId>, TreeError> {
        match parent {
            Some(parent) => self
                .widgets
                .get_mut(&parent)
                .map(|widget| &mut widget.children)
                .ok_or(TreeError::MissingParent(parent)),
            None => Ok(&mut self.roots),
        }
    }

    fn insert_child(&mut self, parent: Option<WidgetId>, index: usize, id: WidgetId) {
        match parent {
            Some(parent) => self
                .widgets
                .get_mut(&parent)
                .expect("parent was validated")
                .children
                .insert(index, id),
            None => self.roots.insert(index, id),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TreeError {
    InvalidTextSelection,
    InvalidDocumentFont,
    InvalidIconSelector,
    InvalidGridDamage,
    GridBaseMismatch,
    DuplicateWidget(WidgetId),
    MissingWidget(WidgetId),
    MissingParent(WidgetId),
    InvalidIndex {
        index: usize,
        length: usize,
    },
    TextTooLong {
        actual: usize,
        maximum: usize,
    },
    LimitExceeded {
        resource: &'static str,
        maximum: usize,
    },
    Cycle,
    InconsistentTree,
}

impl fmt::Display for TreeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidGridDamage => formatter.write_str("invalid grid damage"),
            Self::InvalidIconSelector => formatter.write_str("invalid icon selector"),
            Self::InvalidDocumentFont => formatter.write_str("invalid document font"),
            Self::InvalidTextSelection => formatter.write_str("invalid text field selection"),
            Self::GridBaseMismatch => formatter.write_str("grid damage base mismatch"),
            Self::DuplicateWidget(id) => write!(formatter, "widget {id} already exists"),
            Self::MissingWidget(id) => write!(formatter, "widget {id} does not exist"),
            Self::MissingParent(id) => write!(formatter, "parent widget {id} does not exist"),
            Self::InvalidIndex { index, length } => {
                write!(formatter, "index {index} exceeds child count {length}")
            }
            Self::TextTooLong { actual, maximum } => {
                write!(
                    formatter,
                    "widget text is {actual} bytes, maximum is {maximum}"
                )
            }
            Self::LimitExceeded { resource, maximum } => {
                write!(formatter, "{resource} limit of {maximum} exceeded")
            }
            Self::Cycle => formatter.write_str("widget move would create a cycle"),
            Self::InconsistentTree => formatter.write_str("retained tree is inconsistent"),
        }
    }
}

impl Error for TreeError {}

fn validate_text(text: Option<&str>, kind: crate::WidgetKind) -> Result<(), TreeError> {
    let maximum = if kind == crate::WidgetKind::VirtualGrid {
        crate::grid_frame::MAX_FRAME_BYTES
    } else if kind == crate::WidgetKind::TerminalRow
        || (kind == crate::WidgetKind::TextEditor && text.is_some_and(|v| v.starts_with("editor:rich~o"))) {
        rice_native_app_protocol::MAX_WIDGET_PATCH_TEXT_BYTES
    } else if kind == crate::WidgetKind::Image
        && text.is_some_and(|text| text.starts_with("inline-image:data:image/"))
    {
        crate::MAX_INLINE_IMAGE_TEXT_BYTES
    } else {
        MAX_WIDGET_TEXT_BYTES
    };
    if let Some(text) = text
        && text.len() > maximum
    {
        return Err(TreeError::TextTooLong {
            actual: text.len(),
            maximum,
        });
    }
    Ok(())
}

#[cfg(test)]
mod office_text_bounds {
    use super::*;
    #[test]
    fn only_office_editor_geometry_uses_the_larger_bounded_payload() {
        let text = format!("editor:rich~o816,1056,0,1~{}", "x".repeat(MAX_WIDGET_TEXT_BYTES));
        assert!(validate_text(Some(&text), crate::WidgetKind::TextEditor).is_ok());
        assert!(validate_text(Some(&text), crate::WidgetKind::Label).is_err());
        for prefix in ["editor:plain:", "editor:markdown:", "editor:rich:"] {
            assert!(validate_text(Some(&format!("{prefix}{}", "x".repeat(MAX_WIDGET_TEXT_BYTES))), crate::WidgetKind::TextEditor).is_err());
        }
        assert!(validate_text(Some(&format!("{text}{}", "x".repeat(rice_native_app_protocol::MAX_WIDGET_PATCH_TEXT_BYTES))), crate::WidgetKind::TextEditor).is_err());
    }
}
