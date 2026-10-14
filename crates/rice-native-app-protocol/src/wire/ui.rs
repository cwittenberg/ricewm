use crate::{
    MAX_PATCHES, MAX_TEXT_BYTES, PointerEventKind, UiEvent, UiEventEnvelope, UiPatch,
    UiPatchEnvelope,
};

use super::{DecodeError, EncodeError, Reader, Writer};

pub(super) fn encode_patch(
    writer: &mut Writer,
    envelope: &UiPatchEnvelope,
) -> Result<(), EncodeError> {
    writer.id(envelope.app);
    writer.id(envelope.window);
    writer.u64(envelope.sequence);
    writer.count(envelope.patches.len(), MAX_PATCHES, "UI patches")?;
    for patch in &envelope.patches {
        match patch {
            UiPatch::Create {
                id,
                parent,
                index,
                kind,
                state,
                text,
                icon,
            } => {
                writer.u8(0);
                writer.id(*id);
                writer.option_id(*parent);
                writer.u32(*index);
                writer.u16(*kind);
                writer.u32(*state);
                writer.option_string(text.as_deref(), crate::MAX_GRID_PATCH_TEXT_BYTES, "widget text")?;
                writer.option_u16(*icon);
            }
            UiPatch::Update {
                id,
                state,
                text,
                icon,
            } => {
                writer.u8(1);
                writer.id(*id);
                writer.patch_u32(state);
                writer.patch_string(text, crate::MAX_GRID_PATCH_TEXT_BYTES, "widget text")?;
                writer.patch_u16(icon);
            }
            UiPatch::Move { id, parent, index } => {
                writer.u8(2);
                writer.id(*id);
                writer.option_id(*parent);
                writer.u32(*index);
            }
            UiPatch::Remove { id } => {
                writer.u8(3);
                writer.id(*id);
            }
            UiPatch::Clear => writer.u8(4),
            UiPatch::IconSelector {id,selector} => {writer.u8(9);writer.id(*id);writer.option_string(selector.as_deref(),320,"icon selector")?;},
            UiPatch::DocumentFont { id, font } => { writer.u8(8); writer.id(*id); writer.option_string(font.as_deref(), 1024, "document font")?; },
            UiPatch::ObserveSize { id, enabled } => { writer.u8(6); writer.id(*id); writer.bool(*enabled); },
            UiPatch::EditSelection { id, anchor, caret } => { writer.u8(7); writer.id(*id); writer.u32(*anchor); writer.u32(*caret); },
            UiPatch::GridDamage { id, damage } => {
                writer.u8(5); writer.id(*id);
                writer.string(damage, crate::MAX_GRID_PATCH_TEXT_BYTES, "grid damage")?;
            },
        }
    }
    Ok(())
}

pub(super) fn decode_patch(reader: &mut Reader<'_>) -> Result<UiPatchEnvelope, DecodeError> {
    let app = reader.id("app id")?;
    let window = reader.id("window id")?;
    let sequence = reader.u64()?;
    let count = reader.count(MAX_PATCHES, "UI patches")?;
    let mut patches = Vec::with_capacity(count);
    for _ in 0..count {
        patches.push(match reader.u8()? {
            0 => UiPatch::Create {
                id: reader.id("widget id")?,
                parent: reader.option_id("parent widget id")?,
                index: reader.u32()?,
                kind: reader.u16()?,
                state: reader.u32()?,
                text: reader.option_string(crate::MAX_GRID_PATCH_TEXT_BYTES, "widget text")?,
                icon: reader.option_u16()?,
            },
            1 => UiPatch::Update {
                id: reader.id("widget id")?,
                state: reader.patch_u32()?,
                text: reader.patch_string(crate::MAX_GRID_PATCH_TEXT_BYTES, "widget text")?,
                icon: reader.patch_u16()?,
            },
            2 => UiPatch::Move {
                id: reader.id("widget id")?,
                parent: reader.option_id("parent widget id")?,
                index: reader.u32()?,
            },
            3 => UiPatch::Remove {
                id: reader.id("widget id")?,
            },
            4 => UiPatch::Clear,
            9 => UiPatch::IconSelector {id:reader.id("widget id")?,selector:reader.option_string(320,"icon selector")?},
            8 => UiPatch::DocumentFont { id: reader.id("widget id")?, font: reader.option_string(1024, "document font")? },
            6 => UiPatch::ObserveSize { id: reader.id("widget id")?, enabled: reader.bool("observe size")? },
            7 => UiPatch::EditSelection { id: reader.id("widget id")?, anchor: reader.u32()?, caret: reader.u32()? },
            5 => UiPatch::GridDamage { id: reader.id("widget id")?,
                damage: reader.string(crate::MAX_GRID_PATCH_TEXT_BYTES, "grid damage")? },
            value => {
                return Err(DecodeError::UnknownTag {
                    field: "UI patch",
                    value,
                });
            }
        });
    }
    Ok(UiPatchEnvelope {
        app,
        window,
        sequence,
        patches,
    })
}

pub(super) fn encode_event(
    writer: &mut Writer,
    envelope: &UiEventEnvelope,
) -> Result<(), EncodeError> {
    writer.id(envelope.app);
    writer.id(envelope.window);
    writer.id(envelope.target);
    match &envelope.event {
        UiEvent::Activate => writer.u8(0),
        UiEvent::GridResync => writer.u8(6),
        UiEvent::SizeChanged { width, height } => { writer.u8(7); writer.u32(*width); writer.u32(*height); },
        UiEvent::ValueChanged(value) => {
            writer.u8(1);
            writer.string(value, MAX_TEXT_BYTES, "widget value")?;
        }
        UiEvent::Pointer {
            kind,
            x,
            y,
            button,
            modifiers,
        } => {
            writer.u8(2);
            writer.u8(*kind as u8);
            writer.i32(*x);
            writer.i32(*y);
            writer.u8(*button);
            writer.u16(*modifiers);
        }
        UiEvent::Key {
            pressed,
            key_code,
            modifiers,
            text,
        } => {
            writer.u8(3);
            writer.bool(*pressed);
            writer.u32(*key_code);
            writer.u16(*modifiers);
            writer.option_string(text.as_deref(), MAX_TEXT_BYTES, "key text")?;
        }
        UiEvent::Focus { focused } => {
            writer.u8(4);
            writer.bool(*focused);
        }
        UiEvent::TextSelection { anchor, caret } => {
            writer.u8(5);
            writer.u32(*anchor);
            writer.u32(*caret);
        }
    }
    Ok(())
}

pub(super) fn decode_event(reader: &mut Reader<'_>) -> Result<UiEventEnvelope, DecodeError> {
    let app = reader.id("app id")?;
    let window = reader.id("window id")?;
    let target = reader.id("widget id")?;
    let event = match reader.u8()? {
        0 => UiEvent::Activate,
        6 => UiEvent::GridResync,
        7 => UiEvent::SizeChanged { width: reader.u32()?, height: reader.u32()? },
        1 => UiEvent::ValueChanged(reader.string(MAX_TEXT_BYTES, "widget value")?),
        2 => UiEvent::Pointer {
            kind: pointer_kind(reader)?,
            x: reader.i32()?,
            y: reader.i32()?,
            button: reader.u8()?,
            modifiers: reader.u16()?,
        },
        3 => UiEvent::Key {
            pressed: reader.bool("key pressed")?,
            key_code: reader.u32()?,
            modifiers: reader.u16()?,
            text: reader.option_string(MAX_TEXT_BYTES, "key text")?,
        },
        4 => UiEvent::Focus {
            focused: reader.bool("focused")?,
        },
        5 => UiEvent::TextSelection {
            anchor: reader.u32()?,
            caret: reader.u32()?,
        },
        value => {
            return Err(DecodeError::UnknownTag {
                field: "UI event",
                value,
            });
        }
    };
    Ok(UiEventEnvelope {
        app,
        window,
        target,
        event,
    })
}

fn pointer_kind(reader: &mut Reader<'_>) -> Result<PointerEventKind, DecodeError> {
    match reader.u8()? {
        0 => Ok(PointerEventKind::Move),
        1 => Ok(PointerEventKind::Down),
        2 => Ok(PointerEventKind::Up),
        3 => Ok(PointerEventKind::Enter),
        4 => Ok(PointerEventKind::Leave),
        5 => Ok(PointerEventKind::Wheel),
        value => Err(DecodeError::UnknownTag {
            field: "pointer event kind",
            value,
        }),
    }
}
