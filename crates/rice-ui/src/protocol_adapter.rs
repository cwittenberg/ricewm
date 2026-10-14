use core::fmt;
use std::error::Error;

use crate::{IconId, Patch, ValueChange, WidgetKind, WidgetSpec, WidgetState};

impl Patch {
    /// Converts this toolkit patch into its versioned transport form.
    ///
    /// # Errors
    ///
    /// Returns [`PatchConversionError`] if an index does not fit the wire
    /// representation.
    pub fn to_protocol(&self) -> Result<rice_native_app_protocol::UiPatch, PatchConversionError> {
        use rice_native_app_protocol::{PatchValue, UiPatch};

        Ok(match self {
            Self::Create {
                widget,
                parent,
                index,
            } => UiPatch::Create {
                id: widget.id,
                parent: *parent,
                index: u32::try_from(*index).map_err(|_| {
                    PatchConversionError::IndexTooLarge(u64::try_from(*index).unwrap_or(u64::MAX))
                })?,
                kind: widget.kind as u16,
                state: widget.state.bits(),
                text: widget.text.clone(),
                icon: widget.icon.map(IconId::wire_value),
            },
            Self::Update {
                id,
                state,
                text,
                icon,
            } => UiPatch::Update {
                id: *id,
                state: state.map_or(PatchValue::Keep, |state| PatchValue::Set(state.bits())),
                text: text.clone().into(),
                icon: match icon {
                    ValueChange::Keep => PatchValue::Keep,
                    ValueChange::Set(icon) => PatchValue::Set(icon.wire_value()),
                    ValueChange::Clear => PatchValue::Clear,
                },
            },
            Self::Move { id, parent, index } => UiPatch::Move {
                id: *id,
                parent: *parent,
                index: u32::try_from(*index).map_err(|_| {
                    PatchConversionError::IndexTooLarge(u64::try_from(*index).unwrap_or(u64::MAX))
                })?,
            },
            Self::Remove { id } => UiPatch::Remove { id: *id },
            Self::Clear => UiPatch::Clear,
            Self::IconSelector {id,selector} => UiPatch::IconSelector {id:*id,selector:selector.clone()},
            Self::DocumentFont { id, font } => UiPatch::DocumentFont { id: *id, font: font.clone() },
            Self::GridDamage { id, damage } => UiPatch::GridDamage { id: *id, damage: damage.clone() },
            Self::ObserveSize { id, enabled } => UiPatch::ObserveSize { id: *id, enabled: *enabled },
            Self::EditSelection { id, anchor, caret } => UiPatch::EditSelection { id: *id, anchor: *anchor, caret: *caret },
        })
    }
}

impl<T> From<ValueChange<T>> for rice_native_app_protocol::PatchValue<T> {
    fn from(value: ValueChange<T>) -> Self {
        match value {
            ValueChange::Keep => Self::Keep,
            ValueChange::Set(value) => Self::Set(value),
            ValueChange::Clear => Self::Clear,
        }
    }
}

impl TryFrom<&rice_native_app_protocol::UiPatch> for Patch {
    type Error = PatchConversionError;

    fn try_from(patch: &rice_native_app_protocol::UiPatch) -> Result<Self, Self::Error> {
        use rice_native_app_protocol::{PatchValue, UiPatch};

        Ok(match patch {
            UiPatch::Create {
                id,
                parent,
                index,
                kind,
                state,
                text,
                icon,
            } => Self::Create {
                widget: WidgetSpec {
                    id: *id,
                    kind: WidgetKind::from_wire(*kind)
                        .ok_or(PatchConversionError::UnknownWidgetKind(*kind))?,
                    state: WidgetState::from_bits_retain(*state),
                    text: text.clone(),
                    icon: icon
                        .map(|value| {
                            IconId::from_wire(value).ok_or(PatchConversionError::UnknownIcon(value))
                        })
                        .transpose()?,
                },
                parent: *parent,
                index: usize::try_from(*index)
                    .map_err(|_| PatchConversionError::IndexTooLarge(u64::from(*index)))?,
            },
            UiPatch::Update {
                id,
                state,
                text,
                icon,
            } => Self::Update {
                id: *id,
                state: match state {
                    PatchValue::Keep => None,
                    PatchValue::Set(state) => Some(WidgetState::from_bits_retain(*state)),
                    PatchValue::Clear => Some(WidgetState::empty()),
                },
                text: text.clone().into(),
                icon: match icon {
                    PatchValue::Keep => ValueChange::Keep,
                    PatchValue::Set(value) => ValueChange::Set(
                        IconId::from_wire(*value)
                            .ok_or(PatchConversionError::UnknownIcon(*value))?,
                    ),
                    PatchValue::Clear => ValueChange::Clear,
                },
            },
            UiPatch::Move { id, parent, index } => Self::Move {
                id: *id,
                parent: *parent,
                index: usize::try_from(*index)
                    .map_err(|_| PatchConversionError::IndexTooLarge(u64::from(*index)))?,
            },
            UiPatch::Remove { id } => Self::Remove { id: *id },
            UiPatch::Clear => Self::Clear,
            UiPatch::IconSelector {id,selector} => Self::IconSelector {id:*id,selector:selector.clone()},
            UiPatch::DocumentFont { id, font } => Self::DocumentFont { id: *id, font: font.clone() },
            UiPatch::ObserveSize { id, enabled } => Self::ObserveSize { id: *id, enabled: *enabled },
            UiPatch::EditSelection { id, anchor, caret } => Self::EditSelection { id: *id, anchor: *anchor, caret: *caret },
            UiPatch::GridDamage { id, damage } => Self::GridDamage { id: *id, damage: damage.clone() },
        })
    }
}

impl<T> From<rice_native_app_protocol::PatchValue<T>> for ValueChange<T> {
    fn from(value: rice_native_app_protocol::PatchValue<T>) -> Self {
        match value {
            rice_native_app_protocol::PatchValue::Keep => Self::Keep,
            rice_native_app_protocol::PatchValue::Set(value) => Self::Set(value),
            rice_native_app_protocol::PatchValue::Clear => Self::Clear,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PatchConversionError {
    UnknownWidgetKind(u16),
    UnknownIcon(u16),
    IndexTooLarge(u64),
}

impl fmt::Display for PatchConversionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownWidgetKind(value) => write!(formatter, "unknown widget kind {value}"),
            Self::UnknownIcon(value) => write!(formatter, "unknown semantic icon {value}"),
            Self::IndexTooLarge(value) => write!(formatter, "widget index {value} is too large"),
        }
    }
}

impl Error for PatchConversionError {}
