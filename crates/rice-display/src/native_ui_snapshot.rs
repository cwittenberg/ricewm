//! Renderer-private encoding for deterministic native-UI draw snapshots.
//!
//! RAPP remains the only application/host message protocol. This format is a
//! compact display-backend detail and is intentionally not exposed by the
//! native application protocol crate.

use crate::native_ui::{DrawCommand, RenderOutput};

pub const SNAPSHOT_VERSION: u16 = 14;
pub const SNAPSHOT_HEADER_BYTES: u16 = 104;
pub const MAX_SNAPSHOT_BYTES: usize = 32 * 1024 * 1024;

const MAGIC: &[u8; 4] = b"RUID";

/// Encode a complete semantic draw snapshot.
///
/// All integers are little-endian. The 104-byte header is:
/// `magic[4], version:u16, header_bytes:u16, total_bytes:u32, width:u32,
/// height:u32, command_count:u32, app:u64, window:u64, focused_widget:u64,
/// dialog_widget:u64, dialog_rect:[i32,i32,u32,u32], dialog_title_bytes:u32,
/// pointer_cursor:u32, popup_rect:[i32,i32,u32,u32],
/// popup_command_start:u32, direction:u32`. Direction at offset 100 is 0 LTR
/// or 1 RTL, resolved from the current display locale. It applies to static
/// labels only; editor, caret, brand and selection text remain LTR until
/// a shared shaped-editing owner handles visual text positions. Pointer cursors are 0 default, 1 column resize,
/// and 2 row resize. `popup_command_start` is `u32::MAX` when no popup is
/// open. The optional UTF-8 dialog title follows the header and the command
/// stream follows that title.
///
/// Each command begins with `tag:u8, flags:u8, record_bytes:u16`. Tags are:
/// 1 rounded fill, 2 rounded border, 3 UTF-8 text, 4 semantic icon, 5 push
/// clip, 6 pop clip, 7/8 web scene header/chunks, 9 decoded media surface,
/// 10 animated spinner, 11 animated indeterminate activity bar, 16 a
/// glyph-measured single-line text selection, and 17 a capability-isolated
/// host surface identity, and 18 a browser-measured group-box frame.
/// Rectangle-bearing records then contain `x:i32, y:i32, width:u32,
/// height:u32`. Colors are four RGBA bytes. Text adds `size_px:u16,
/// text_bytes:u32, utf8[]`; text flags are 0 start, 1 center, 2 end. Icon adds
/// its stable `u16` semantic ID. Media adds `request:u64`, clockwise quarter
/// turns, scale mode (fit/actual), and a bounded `scale_permille:u16`.
/// Scene header tag 7 flag bit 0 marks a native vector `SceneSurface`: fit its
/// viewport with a centered aspect-preserving scale. Flag 0 retains the browser
/// `WebSurface` width-based scale. All other scene header flag bits are reserved.
/// Fractional document fonts use tags 31 (styled text) and 32 (styled edit).
/// They have the same layout as tags 27/28 except for an eight-byte LE f64
/// point size at offset 24 in place of the legacy u16; subsequent fields move
/// forward six bytes. Integer fonts keep the original tags and exact bytes.
pub fn encode_snapshot(
    app: u64,
    window: u64,
    output: &RenderOutput,
) -> Result<Vec<u8>, SnapshotError> {
    let mut bytes = Vec::with_capacity(usize::from(SNAPSHOT_HEADER_BYTES));
    bytes.extend_from_slice(MAGIC);
    push_u16(&mut bytes, SNAPSHOT_VERSION);
    push_u16(&mut bytes, SNAPSHOT_HEADER_BYTES);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, output.size.width);
    push_u32(&mut bytes, output.size.height);
    push_u32(&mut bytes, 0);
    push_u64(&mut bytes, app);
    push_u64(&mut bytes, window);
    push_u64(
        &mut bytes,
        output
            .focused
            .map_or(0, rice_native_app_protocol::WidgetId::get),
    );
    let dialog_title = if let Some(dialog) = &output.dialog {
        push_u64(&mut bytes, dialog.widget.get());
        push_rect(&mut bytes, dialog.rect);
        push_u32(
            &mut bytes,
            u32::try_from(dialog.title.len()).map_err(|_| SnapshotError::SizeLimit)?,
        );
        push_u32(&mut bytes, output.cursor as u32);
        Some(dialog.title.as_bytes())
    } else {
        push_u64(&mut bytes, 0);
        push_rect(
            &mut bytes,
            rice_ui::Rect {
                origin: rice_ui::Point { x: 0, y: 0 },
                size: rice_ui::Size {
                    width: 0,
                    height: 0,
                },
            },
        );
        push_u32(&mut bytes, 0);
        push_u32(&mut bytes, output.cursor as u32);
        None
    };
    if let Some(popup) = output.popup {
        push_rect(&mut bytes, popup.rect);
        push_u32(
            &mut bytes,
            u32::try_from(popup.command_start).map_err(|_| SnapshotError::SizeLimit)?,
        );
    } else {
        push_rect(
            &mut bytes,
            rice_ui::Rect {
                origin: rice_ui::Point { x: 0, y: 0 },
                size: rice_ui::Size {
                    width: 0,
                    height: 0,
                },
            },
        );
        push_u32(&mut bytes, u32::MAX);
    }
    push_u32(&mut bytes, rice_i18n::direction() as u32);
    if let Some(title) = dialog_title {
        bytes.extend_from_slice(title);
    }

    let mut command_count = 0_u32;
    for command in &output.commands {
        command_count = command_count
            .checked_add(encode_command(&mut bytes, command)?)
            .ok_or(SnapshotError::SizeLimit)?;
        if bytes.len() > MAX_SNAPSHOT_BYTES {
            return Err(SnapshotError::SizeLimit);
        }
    }
    let total = u32::try_from(bytes.len()).map_err(|_| SnapshotError::SizeLimit)?;
    bytes[8..12].copy_from_slice(&total.to_le_bytes());
    bytes[20..24].copy_from_slice(&command_count.to_le_bytes());
    Ok(bytes)
}

fn encode_command(bytes: &mut Vec<u8>, command: &DrawCommand) -> Result<u32, SnapshotError> {
    match command {
        DrawCommand::IconOverride {rect,selector} => {if selector.len()>320{return Err(SnapshotError::RecordLimit);}record_header(bytes,34,(22+selector.len()) as u16)?;push_rect(bytes,*rect);push_u16(bytes,selector.len() as u16);bytes.extend_from_slice(selector.as_bytes());}
        DrawCommand::ThemeSurface {rect,role,state} => {record_header(bytes,33,22)?;push_rect(bytes,*rect);bytes.push(*role);bytes.push(*state);}
        DrawCommand::Fill {
            rect,
            color,
            radius,
        } => {
            record_header(bytes, 1, 26)?;
            push_rect(bytes, *rect);
            bytes.extend_from_slice(&color.channels());
            push_u16(bytes, *radius);
        }
        DrawCommand::Border {
            rect,
            color,
            width,
            radius,
        } => {
            record_header(bytes, 2, 28)?;
            push_rect(bytes, *rect);
            bytes.extend_from_slice(&color.channels());
            push_u16(bytes, *width);
            push_u16(bytes, *radius);
        }
        DrawCommand::StrokeLine {rect,color,width,pattern} => { if !(1..=12).contains(width)||*pattern>6{return Err(SnapshotError::RecordLimit);} let offset=bytes.len();record_header(bytes,30,26)?;bytes[offset+1]=*pattern;push_rect(bytes,*rect);bytes.extend_from_slice(&color.channels());push_u16(bytes,*width); }
        DrawCommand::DottedLine { rect, color } => {
            record_header(bytes, 23, 24)?;
            push_rect(bytes, *rect);
            bytes.extend_from_slice(&color.channels());
        }
        DrawCommand::ShapedFill { rect, color, shape } => {
            record_header(bytes, 21, 25)?;
            push_rect(bytes, *rect);
            bytes.extend_from_slice(&color.channels());
            bytes.push(*shape as u8);
        }
        DrawCommand::ShapedBorder {
            rect,
            color,
            width,
            shape,
        } => {
            record_header(bytes, 22, 27)?;
            push_rect(bytes, *rect);
            bytes.extend_from_slice(&color.channels());
            push_u16(bytes, *width);
            bytes.push(*shape as u8);
        }
        DrawCommand::GroupBoxFrame {
            rect,
            legend,
            color,
            width,
            size_px,
            legend_x,
            legend_y,
        } => {
            let text_len = u32::try_from(legend.len()).map_err(|_| SnapshotError::SizeLimit)?;
            let record_len = 40_usize
                .checked_add(legend.len())
                .and_then(|length| u16::try_from(length).ok())
                .ok_or(SnapshotError::RecordLimit)?;
            record_header(bytes, 18, record_len)?;
            push_rect(bytes, *rect);
            bytes.extend_from_slice(&color.channels());
            push_u16(bytes, *width);
            push_u16(bytes, *size_px);
            bytes.extend_from_slice(&legend_x.to_le_bytes());
            bytes.extend_from_slice(&legend_y.to_le_bytes());
            push_u32(bytes, text_len);
            bytes.extend_from_slice(legend.as_bytes());
        }
        DrawCommand::BalloonFrame {
            rect,
            fill,
            border,
            radius,
            caret_x,
            caret_at_top,
        } => {
            let record_offset = bytes.len();
            record_header(bytes, 19, 34)?;
            bytes[record_offset + 1] = u8::from(*caret_at_top);
            push_rect(bytes, *rect);
            bytes.extend_from_slice(&fill.channels());
            bytes.extend_from_slice(&border.channels());
            push_u16(bytes, *radius);
            bytes.extend_from_slice(&caret_x.to_le_bytes());
        }
        DrawCommand::Text {
            rect,
            text,
            color,
            size_px,
            align,
            role,
            caret,
            caret_offset,
            brand,
            gradient,
        } => {
            let text_len = u32::try_from(text.len()).map_err(|_| SnapshotError::SizeLimit)?;
            let record_len = 30_usize
                .checked_add(caret_offset.map_or(0, |_| 4))
                .ok_or(SnapshotError::SizeLimit)?
                .checked_add(text.len())
                .ok_or(SnapshotError::SizeLimit)?;
            let record_len = u16::try_from(record_len).map_err(|_| SnapshotError::RecordLimit)?;
            let record_offset = bytes.len();
            record_header(bytes, 3, record_len)?;
            bytes[record_offset + 1] = *align as u8
                | ((*role as u8) << 2)
                | (u8::from(*caret) << 4)
                | (u8::from(caret_offset.is_some()) << 5)
                | (u8::from(*brand) << 6)
                | (u8::from(*gradient) << 7);
            push_rect(bytes, *rect);
            bytes.extend_from_slice(&color.channels());
            push_u16(bytes, *size_px);
            push_u32(bytes, text_len);
            if let Some(offset) = caret_offset {
                push_u32(bytes, *offset);
            }
            bytes.extend_from_slice(text.as_bytes());
        }
        DrawCommand::GridText {zoom_percent,rect,text,color,align,font,size_px,vertical,wrap,strike} => {
            if *size_px==0||*vertical>2||!(25..=400).contains(zoom_percent)||font.as_ref().is_some_and(|f|!f.is_valid()){return Err(SnapshotError::RecordLimit);}
            let font=match font {Some(font)=>font.encode().ok_or(SnapshotError::RecordLimit)?,None=>String::new()};
            let scaled=*zoom_percent!=100;
            let length=(32_usize+usize::from(scaled)*2).checked_add(font.len()).and_then(|n|n.checked_add(text.len())).and_then(|n|u16::try_from(n).ok()).ok_or(SnapshotError::RecordLimit)?;
            let offset=bytes.len();record_header(bytes,29,length)?;bytes[offset+1]=*align as u8|(*vertical<<2)|(u8::from(*wrap)<<4)|(u8::from(*strike)<<5)|(u8::from(scaled)<<6);
            push_rect(bytes,*rect);bytes.extend_from_slice(&color.channels());push_u16(bytes,*size_px);push_u16(bytes,font.len() as u16);push_u32(bytes,text.len() as u32);
            if scaled {push_u16(bytes,*zoom_percent);}
            bytes.extend_from_slice(font.as_bytes());bytes.extend_from_slice(text.as_bytes());
        }
        DrawCommand::StyledEdit { zoom_percent,rect,text,color,font,caret,selection,selection_color,vertical } => {
            if !font.is_valid()||!(25..=400).contains(zoom_percent)||*vertical>2 { return Err(SnapshotError::RecordLimit); }
            let scaled=*zoom_percent!=100;
            let integer_size=font.size_pt.as_integer();
            let length=(50_usize+usize::from(integer_size.is_none())*6+usize::from(scaled)*2).checked_add(font.family.len()).and_then(|n|n.checked_add(text.len())).and_then(|n|u16::try_from(n).ok()).ok_or(SnapshotError::RecordLimit)?;
            let offset=bytes.len();
            record_header(bytes,if integer_size.is_some(){28}else{32},length)?;
            bytes[offset+1]=*vertical<<2;
            push_rect(bytes,*rect); bytes.extend_from_slice(&color.channels());
            if let Some(size)=integer_size {push_u16(bytes,size);}else{bytes.extend_from_slice(&font.size_pt.points().to_le_bytes());}
            push_u16(bytes,font.weight);
            bytes.push(u8::from(font.italic)|(u8::from(font.underline)<<1)|(u8::from(scaled)<<2)); bytes.push(font.family.len() as u8);
            push_u32(bytes,text.len() as u32); push_u32(bytes,caret.unwrap_or(u32::MAX));
            push_u32(bytes,selection.map_or(u32::MAX,|s|s.0)); push_u32(bytes,selection.map_or(u32::MAX,|s|s.1));
            bytes.extend_from_slice(&selection_color.channels());
            if scaled {push_u16(bytes,*zoom_percent);}
            bytes.extend_from_slice(font.family.as_bytes()); bytes.extend_from_slice(text.as_bytes());
        }
        DrawCommand::StyledText { zoom_percent,rect,text,color,align,font } => {
            if !font.is_valid()||!(25..=400).contains(zoom_percent) { return Err(SnapshotError::RecordLimit); }
            let scaled=*zoom_percent!=100;
            let integer_size=font.size_pt.as_integer();
            let record_len=(34_usize+usize::from(integer_size.is_none())*6+usize::from(scaled)*2).checked_add(font.family.len()).and_then(|n|n.checked_add(text.len())).and_then(|n|u16::try_from(n).ok()).ok_or(SnapshotError::RecordLimit)?;
            let offset=bytes.len();record_header(bytes,if integer_size.is_some(){27}else{31},record_len)?;bytes[offset+1]=*align as u8;
            push_rect(bytes,*rect);bytes.extend_from_slice(&color.channels());
            if let Some(size)=integer_size {push_u16(bytes,size);}else{bytes.extend_from_slice(&font.size_pt.points().to_le_bytes());}
            push_u16(bytes,font.weight);
            bytes.push(u8::from(font.italic)|(u8::from(font.underline)<<1)|(u8::from(scaled)<<2));bytes.push(font.family.len() as u8);push_u32(bytes,text.len() as u32);
            if scaled {push_u16(bytes,*zoom_percent);}
            bytes.extend_from_slice(font.family.as_bytes());bytes.extend_from_slice(text.as_bytes());
        }
        DrawCommand::TextSelection {
            rect,
            text,
            color,
            size_px,
            align,
            start,
            end,
        } => {
            let text_len = u32::try_from(text.len()).map_err(|_| SnapshotError::SizeLimit)?;
            let record_len = 38_usize
                .checked_add(text.len())
                .ok_or(SnapshotError::SizeLimit)?;
            let record_len = u16::try_from(record_len).map_err(|_| SnapshotError::RecordLimit)?;
            let record_offset = bytes.len();
            record_header(bytes, 16, record_len)?;
            bytes[record_offset + 1] = *align as u8;
            push_rect(bytes, *rect);
            bytes.extend_from_slice(&color.channels());
            push_u16(bytes, *size_px);
            push_u32(bytes, *start);
            push_u32(bytes, *end);
            push_u32(bytes, text_len);
            bytes.extend_from_slice(text.as_bytes());
        }
        DrawCommand::Icon {
            rect,
            icon,
            color,
            hovered,
            animated,
            shortcut,
        } => {
            let record_offset = bytes.len();
            record_header(bytes, 4, 26)?;
            bytes[record_offset + 1] =
                u8::from(*hovered) | (u8::from(*animated) << 1) | (u8::from(*shortcut) << 2);
            push_rect(bytes, *rect);
            bytes.extend_from_slice(&color.channels());
            push_u16(bytes, icon.wire_value());
        }
        DrawCommand::ScopedIcon {rect,icon,color,hovered} => {
            let record_offset=bytes.len();record_header(bytes,4,26)?;
            bytes[record_offset+1]=8|u8::from(*hovered);
            push_rect(bytes,*rect);bytes.extend_from_slice(&color.channels());push_u16(bytes,icon.wire_value());
        }
        DrawCommand::Spinner { rect, color } => {
            record_header(bytes, 10, 24)?;
            push_rect(bytes, *rect);
            bytes.extend_from_slice(&color.channels());
        }
        DrawCommand::Activity { rect, color } => {
            record_header(bytes, 11, 24)?;
            push_rect(bytes, *rect);
            bytes.extend_from_slice(&color.channels());
        }
        DrawCommand::Glass {
            rect,
            tone,
            tint,
            radius,
            hovered,
            control,
            active,
            structural,
            popup,
        } => {
            let record_offset = bytes.len();
            record_header(bytes, 13, 26)?;
            bytes[record_offset + 1] = (*tone & 0x03)
                | (u8::from(*hovered) << 2)
                | (u8::from(*control) << 3)
                | (u8::from(*active) << 4)
                | (u8::from(*structural) << 5)
                | (u8::from(*popup) << 6);
            push_rect(bytes, *rect);
            bytes.extend_from_slice(&tint.channels());
            push_u16(bytes, *radius);
        }
        DrawCommand::ScenicBackdrop { rect } => {
            record_header(bytes, 14, 20)?;
            push_rect(bytes, *rect);
        }
        DrawCommand::WelcomeJourney { rect } => {
            record_header(bytes, 15, 20)?;
            push_rect(bytes, *rect);
        }
        DrawCommand::PushClip { rect } => {
            record_header(bytes, 5, 20)?;
            push_rect(bytes, *rect);
        }
        DrawCommand::PopClip => record_header(bytes, 6, 4)?,
        DrawCommand::Media {
            rect,
            request,
            quarter_turns,
            scale_mode,
            scale_permille,
            pan_x,
            pan_y,
            image_edit,
        } => {
            record_header(bytes, 9, if image_edit.is_some() { 48 } else { 36 })?;
            push_rect(bytes, *rect);
            push_u64(bytes, *request);
            bytes.push(*quarter_turns);
            bytes.push(*scale_mode as u8);
            push_u16(bytes, *scale_permille);
            bytes.extend_from_slice(&pan_x.to_le_bytes());
            bytes.extend_from_slice(&pan_y.to_le_bytes());
            if let Some(values) = image_edit { for value in values { push_u16(bytes, *value); } }
        }
        DrawCommand::InlineImage { rect, data_url } => {
            let data_len = u32::try_from(data_url.len()).map_err(|_| SnapshotError::SizeLimit)?;
            let record_len = 24_usize
                .checked_add(data_url.len())
                .and_then(|length| u16::try_from(length).ok())
                .ok_or(SnapshotError::RecordLimit)?;
            record_header(bytes, 20, record_len)?;
            push_rect(bytes, *rect);
            push_u32(bytes, data_len);
            bytes.extend_from_slice(data_url.as_bytes());
        }
        DrawCommand::WebScene {
            surface,
            rect,
            epoch,
            viewport_width,
            viewport_height,
            display_list,
            damage,
            vector_scene,
        } => {
            record_header(bytes, 7, 64)?;
            let flags = bytes.len() - 3;
            bytes[flags] = u8::from(*vector_scene);
            push_rect(bytes, *rect);
            push_u64(bytes, surface.get());
            push_u64(bytes, *epoch);
            push_u32(bytes, *viewport_width);
            push_u32(bytes, *viewport_height);
            push_u32(
                bytes,
                u32::try_from(display_list.len()).map_err(|_| SnapshotError::SizeLimit)?,
            );
            let damage = damage.unwrap_or([0, 0, u32::MAX, u32::MAX]);
            for value in damage {
                push_u32(bytes, value);
            }
            let mut count = 1_u32;
            for (index, chunk) in display_list.chunks(60 * 1024).enumerate() {
                let record_len = 16usize
                    .checked_add(chunk.len())
                    .and_then(|length| u16::try_from(length).ok())
                    .ok_or(SnapshotError::RecordLimit)?;
                record_header(bytes, 8, record_len)?;
                push_u64(bytes, surface.get());
                push_u32(
                    bytes,
                    u32::try_from(index.saturating_mul(60 * 1024))
                        .map_err(|_| SnapshotError::SizeLimit)?,
                );
                bytes.extend_from_slice(chunk);
                count = count.checked_add(1).ok_or(SnapshotError::SizeLimit)?;
            }
            return Ok(count);
        }
        DrawCommand::ColorPicker { rect, color, hue, hue_degrees } => {
            record_header(bytes, 26, 27)?;
            push_rect(bytes, *rect);
            bytes.extend_from_slice(&color.channels());
            bytes.push(u8::from(*hue));
            bytes.extend_from_slice(&hue_degrees.to_le_bytes());
        }
        DrawCommand::TerminalGrid { rect, session } => {
            record_header(bytes, 25, 28)?;
            push_rect(bytes, *rect);
            bytes.extend_from_slice(&session.to_le_bytes());
        }
        DrawCommand::TerminalRow { rect, text } => {
            let length = u16::try_from(24 + text.len()).map_err(|_| SnapshotError::RecordLimit)?;
            record_header(bytes, 24, length)?;
            push_rect(bytes, *rect);
            push_u32(bytes, text.len() as u32);
            bytes.extend_from_slice(text.as_bytes());
        }
        DrawCommand::TerminalPortal { rect, session } => {
            record_header(bytes, 12, 28)?;
            push_rect(bytes, *rect);
            push_u64(bytes, *session);
        }
        DrawCommand::HostPortal { rect, identity } => {
            if identity.len() != 64 || !identity.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(SnapshotError::RecordLimit);
            }
            record_header(bytes, 17, 84)?;
            push_rect(bytes, *rect);
            bytes.extend_from_slice(identity.as_bytes());
        }
    }
    Ok(1)
}

fn record_header(bytes: &mut Vec<u8>, tag: u8, record_bytes: u16) -> Result<(), SnapshotError> {
    let new_len = bytes
        .len()
        .checked_add(usize::from(record_bytes))
        .ok_or(SnapshotError::SizeLimit)?;
    if new_len > MAX_SNAPSHOT_BYTES {
        return Err(SnapshotError::SizeLimit);
    }
    bytes.push(tag);
    bytes.push(0);
    push_u16(bytes, record_bytes);
    Ok(())
}

fn push_rect(bytes: &mut Vec<u8>, rect: rice_ui::Rect) {
    bytes.extend_from_slice(&rect.origin.x.to_le_bytes());
    bytes.extend_from_slice(&rect.origin.y.to_le_bytes());
    push_u32(bytes, rect.size.width);
    push_u32(bytes, rect.size.height);
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotError {
    RecordLimit,
    SizeLimit,
}

#[cfg(test)]
mod tests {
    use rice_ui::{Color, IconId, Point, Rect, Size};

    use super::*;
    use crate::native_ui::TextAlign;

    #[test]
    fn snapshot_v14_direction_header_follows_scoped_locale() {
        let output = RenderOutput {
            size: Size { width: 320, height: 200 },
            commands: Vec::new(), hit_targets: Vec::new(), focused: None,
            cursor: crate::native_ui::PointerCursor::Default, dialog: None, popup: None,
        };
        for (locale, direction) in [("en", 0_u32), ("ar", 1), ("fa", 1), ("ur", 1), ("zh-Hans", 0), ("en", 0)] {
            rice_i18n::with_locale(rice_i18n::locale_code(locale), || {
                let bytes = encode_snapshot(7, 1, &output).unwrap();
                assert_eq!(bytes.len(), 104);
                assert_eq!(u16::from_le_bytes(bytes[4..6].try_into().unwrap()), 14);
                assert_eq!(u16::from_le_bytes(bytes[6..8].try_into().unwrap()), 104);
                assert_eq!(u32::from_le_bytes(bytes[100..104].try_into().unwrap()), direction);
                assert_eq!(u32::from_le_bytes(bytes[96..100].try_into().unwrap()), u32::MAX);
            });
        }
    }

    #[test]
    fn shortcut_icon_flag_is_encoded_separately_from_the_target_icon() {
        let output = RenderOutput {
            size: Size {
                width: 48,
                height: 48,
            },
            commands: vec![DrawCommand::Icon {
                rect: Rect {
                    origin: Point { x: 0, y: 0 },
                    size: Size {
                        width: 48,
                        height: 48,
                    },
                },
                icon: IconId::Text,
                color: Color::rgba(1, 2, 3, 255),
                hovered: false,
                animated: false,
                shortcut: true,
            }],
            hit_targets: Vec::new(),
            focused: None,
            cursor: crate::native_ui::PointerCursor::Default,
            dialog: None,
            popup: None,
        };
        let bytes = encode_snapshot(1, 1, &output).expect("snapshot");
        let command = &bytes[usize::from(SNAPSHOT_HEADER_BYTES)..];
        assert_eq!(command[0], 4);
        assert_eq!(command[1] & 0x04, 0x04);
        assert_eq!(u16::from_le_bytes([command[24], command[25]]), 107);
    }

    #[test]
    fn styled_text_has_a_separate_bounded_record_without_changing_plain_text_flags() {
        let output=RenderOutput{size:Size{width:140,height:50},commands:vec![DrawCommand::StyledText{zoom_percent:100,
            rect:Rect{origin:Point{x:0,y:0},size:Size{width:140,height:50}},text:"Bold".into(),
            color:Color::rgba(1,2,3,255),align:TextAlign::Center,
            font:rice_ui::grid_frame::GridFont{family:"RiceWM Mono".into(),size_pt:rice_ui::grid_frame::GridFontSize::new(18.0).unwrap(),weight:700,italic:true,underline:true}}],
            hit_targets:Vec::new(),focused:None,cursor:crate::native_ui::PointerCursor::Default,dialog:None,popup:None};
        let bytes=encode_snapshot(1,1,&output).unwrap();let record=&bytes[usize::from(SNAPSHOT_HEADER_BYTES)..];
        assert_eq!(record[0],27);assert_eq!(record[1],1);assert_eq!(u16::from_le_bytes([record[2],record[3]]) as usize,record.len());
        assert_eq!(u16::from_le_bytes([record[24],record[25]]),18);assert_eq!(u16::from_le_bytes([record[26],record[27]]),700);
        assert_eq!(record[28],3);assert_eq!(record[29],11);assert_eq!(&record[34..45],b"RiceWM Mono");assert_eq!(&record[45..],b"Bold");
    }

    #[test]
    fn fractional_styled_records_preserve_points_caret_selection_and_zoom() {
        use rice_ui::grid_frame::{GridFont,GridFontSize};
        for points in [11.5,14.5,11.25] {
            for zoom in [70,100,150] {
                let font=GridFont{family:"Arial".into(),size_pt:GridFontSize::new(points).unwrap(),weight:700,italic:true,underline:true};
                let rect=Rect{origin:Point{x:2,y:3},size:Size{width:90,height:20}};
                let mut output=RenderOutput{size:Size{width:100,height:30},commands:vec![DrawCommand::StyledText{zoom_percent:zoom,rect,text:"Wié😀".into(),color:Color::rgba(1,2,3,255),align:TextAlign::End,font:font.clone()},DrawCommand::StyledEdit{zoom_percent:zoom,rect,text:"Wié😀".into(),color:Color::rgba(1,2,3,255),font,caret:Some(4),selection:Some((1,4)),selection_color:Color::rgba(5,6,7,255),vertical:0}],hit_targets:Vec::new(),focused:None,cursor:crate::native_ui::PointerCursor::Default,dialog:None,popup:None};
                let bytes=encode_snapshot(1,1,&output).unwrap();let mut offset=usize::from(SNAPSHOT_HEADER_BYTES);
                for (tag,base) in [(31,40),(32,56)] {
                    let length=usize::from(u16::from_le_bytes(bytes[offset+2..offset+4].try_into().unwrap()));let record=&bytes[offset..offset+length];
                    assert_eq!(record[0],tag);assert_eq!(record[1],if tag==31 {2}else{0});
                    assert_eq!(f64::from_le_bytes(record[24..32].try_into().unwrap()).to_bits(),points.to_bits());
                    assert_eq!(u16::from_le_bytes(record[32..34].try_into().unwrap()),700);
                    assert_eq!(record[34],3|if zoom==100 {0}else{4});assert_eq!(record[35],5);
                    assert_eq!(u32::from_le_bytes(record[36..40].try_into().unwrap()),"Wié😀".len() as u32);
                    if tag==32 {assert_eq!(&record[40..44],&4_u32.to_le_bytes());assert_eq!(&record[44..48],&1_u32.to_le_bytes());assert_eq!(&record[48..52],&4_u32.to_le_bytes());assert_eq!(&record[52..56],&[5,6,7,255]);}
                    let text_start=base+if zoom==100 {0}else{assert_eq!(u16::from_le_bytes(record[base..base+2].try_into().unwrap()),zoom);2};
                    assert_eq!(&record[text_start..text_start+5],b"Arial");assert_eq!(&record[text_start+5..],"Wié😀".as_bytes());offset+=length;
                }
                assert_eq!(offset,bytes.len());
                if let DrawCommand::StyledText{text,..}=&mut output.commands[0] {*text="x".repeat(65_536);}
                assert!(matches!(encode_snapshot(1,1,&output),Err(SnapshotError::RecordLimit)),"oversized fractional record is rejected without truncation");
            }
        }
    }

    #[test]
    fn integral_float_fonts_keep_legacy_styled_edit_bytes() {
        let font=serde_json::from_str(r#"{"family":"Arial","size_pt":11.0,"weight":400,"italic":false,"underline":false}"#).unwrap();
        let output=RenderOutput{size:Size{width:100,height:30},commands:vec![DrawCommand::StyledEdit{zoom_percent:150,rect:Rect{origin:Point{x:2,y:3},size:Size{width:90,height:20}},text:"é".into(),color:Color::rgba(1,2,3,255),font,caret:Some(1),selection:None,selection_color:Color::rgba(5,6,7,255),vertical:0}],hit_targets:Vec::new(),focused:None,cursor:crate::native_ui::PointerCursor::Default,dialog:None,popup:None};
        let bytes=encode_snapshot(1,1,&output).unwrap();let record=&bytes[usize::from(SNAPSHOT_HEADER_BYTES)..];
        let mut expected=vec![28,0,59,0];
        expected.extend_from_slice(&2_i32.to_le_bytes());expected.extend_from_slice(&3_i32.to_le_bytes());expected.extend_from_slice(&90_u32.to_le_bytes());expected.extend_from_slice(&20_u32.to_le_bytes());
        expected.extend_from_slice(&[1,2,3,255,11,0,144,1,4,5,2,0,0,0]);
        expected.extend_from_slice(&1_u32.to_le_bytes());expected.extend_from_slice(&u32::MAX.to_le_bytes());expected.extend_from_slice(&u32::MAX.to_le_bytes());expected.extend_from_slice(&[5,6,7,255,150,0]);expected.extend_from_slice(b"Arial");expected.extend_from_slice("é".as_bytes());
        assert_eq!(record,expected,"integer font snapshot ABI remains byte-for-byte unchanged");
    }

    #[test]
    fn centered_text_alignment_has_a_stable_wire_flag() {
        let output = RenderOutput {
            size: Size {
                width: 20,
                height: 10,
            },
            commands: vec![DrawCommand::Text {
                rect: Rect {
                    origin: Point { x: 2, y: 3 },
                    size: Size {
                        width: 16,
                        height: 7,
                    },
                },
                text: String::from("A"),
                color: Color::rgba(1, 2, 3, 4),
                size_px: 13,
                align: TextAlign::Center,
                role: crate::native_ui::TextRole::Label,
                caret: false,
                caret_offset: None,
                brand: false,
                gradient: false,
            }],
            hit_targets: Vec::new(),
            focused: None,
            cursor: crate::native_ui::PointerCursor::Default,
            dialog: None,
            popup: None,
        };
        let bytes = encode_snapshot(5, 7, &output).expect("snapshot");
        assert_eq!(
            &bytes[usize::from(SNAPSHOT_HEADER_BYTES)..],
            &[
                3, 1, 31, 0, // tag, center flag, record bytes
                2, 0, 0, 0, // x
                3, 0, 0, 0, // y
                16, 0, 0, 0, // width
                7, 0, 0, 0, // height
                1, 2, 3, 4, // RGBA
                13, 0, // font size
                1, 0, 0, 0, // UTF-8 bytes
                b'A',
            ],
        );
    }

    #[test]
    fn text_selection_carries_character_offsets_without_pixel_estimates() {
        let output = RenderOutput {
            size: Size {
                width: 320,
                height: 40,
            },
            commands: vec![DrawCommand::TextSelection {
                rect: Rect {
                    origin: Point { x: 12, y: 8 },
                    size: Size {
                        width: 296,
                        height: 24,
                    },
                },
                text: String::from("https://tweakers.net/"),
                color: Color::rgba(233, 84, 32, 144),
                size_px: 12,
                align: TextAlign::Start,
                start: 0,
                end: 21,
            }],
            hit_targets: Vec::new(),
            focused: None,
            cursor: crate::native_ui::PointerCursor::Default,
            dialog: None,
            popup: None,
        };
        let bytes = encode_snapshot(5, 7, &output).expect("snapshot");
        let offset = usize::from(SNAPSHOT_HEADER_BYTES);
        let text_len = "https://tweakers.net/".len();
        assert_eq!(&bytes[offset..offset + 4], &[16, 0, 59, 0]);
        assert_eq!(
            u32::from_le_bytes(bytes[offset + 26..offset + 30].try_into().unwrap()),
            0,
        );
        assert_eq!(
            u32::from_le_bytes(bytes[offset + 30..offset + 34].try_into().unwrap()),
            21,
        );
        assert_eq!(
            u32::from_le_bytes(bytes[offset + 34..offset + 38].try_into().unwrap()),
            u32::try_from(text_len).unwrap(),
        );
        assert_eq!(&bytes[offset + 38..], b"https://tweakers.net/");
    }

    #[test]
    fn animation_primitives_have_bounded_stable_records() {
        let rect = Rect {
            origin: Point { x: 2, y: 3 },
            size: Size {
                width: 16,
                height: 8,
            },
        };
        let output = RenderOutput {
            size: Size {
                width: 40,
                height: 20,
            },
            commands: vec![
                DrawCommand::Spinner {
                    rect,
                    color: Color::rgba(1, 2, 3, 4),
                },
                DrawCommand::Activity {
                    rect,
                    color: Color::rgba(5, 6, 7, 8),
                },
            ],
            hit_targets: Vec::new(),
            focused: None,
            cursor: crate::native_ui::PointerCursor::Default,
            dialog: None,
            popup: None,
        };
        let bytes = encode_snapshot(5, 7, &output).expect("snapshot");
        let offset = usize::from(SNAPSHOT_HEADER_BYTES);
        assert_eq!(&bytes[offset..offset + 4], &[10, 0, 24, 0]);
        assert_eq!(&bytes[offset + 24..offset + 28], &[11, 0, 24, 0]);
    }

    #[test]
    fn terminal_portal_has_a_bounded_stable_record() {
        let output = RenderOutput {
            size: Size {
                width: 320,
                height: 200,
            },
            commands: vec![DrawCommand::TerminalPortal {
                rect: Rect {
                    origin: Point { x: 12, y: 34 },
                    size: Size {
                        width: 296,
                        height: 150,
                    },
                },
                session: 42,
            }],
            hit_targets: Vec::new(),
            focused: None,
            cursor: crate::native_ui::PointerCursor::Default,
            dialog: None,
            popup: None,
        };
        let bytes = encode_snapshot(5, 7, &output).expect("snapshot");
        let offset = usize::from(SNAPSHOT_HEADER_BYTES);
        assert_eq!(&bytes[offset..offset + 4], &[12, 0, 28, 0]);
        assert_eq!(
            i32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()),
            12
        );
        assert_eq!(
            i32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap()),
            34
        );
        assert_eq!(
            u32::from_le_bytes(bytes[offset + 12..offset + 16].try_into().unwrap()),
            296
        );
        assert_eq!(
            u32::from_le_bytes(bytes[offset + 16..offset + 20].try_into().unwrap()),
            150
        );
        assert_eq!(
            u64::from_le_bytes(bytes[offset + 20..offset + 28].try_into().unwrap()),
            42
        );
    }

    #[test]
    fn split_resize_cursor_uses_the_bounded_header_field() {
        let output = RenderOutput {
            size: Size {
                width: 20,
                height: 10,
            },
            commands: Vec::new(),
            hit_targets: Vec::new(),
            focused: None,
            cursor: crate::native_ui::PointerCursor::ColumnResize,
            dialog: None,
            popup: None,
        };
        let bytes = encode_snapshot(5, 7, &output).expect("snapshot");
        assert_eq!(u32::from_le_bytes(bytes[76..80].try_into().unwrap()), 1);
    }

    #[test]
    fn dialog_title_follows_popup_metadata_in_the_expanded_header() {
        let output = RenderOutput {
            size: Size {
                width: 800,
                height: 600,
            },
            commands: Vec::new(),
            hit_targets: Vec::new(),
            focused: None,
            cursor: crate::native_ui::PointerCursor::Default,
            dialog: Some(crate::native_ui::RenderedDialog {
                widget: rice_native_app_protocol::WidgetId::new(9).unwrap(),
                rect: Rect {
                    origin: Point { x: 140, y: 90 },
                    size: Size {
                        width: 520,
                        height: 320,
                    },
                },
                title: String::from("Save Document As"),
            }),
            popup: Some(crate::native_ui::RenderedPopup {
                rect: Rect {
                    origin: Point { x: 410, y: 120 },
                    size: Size {
                        width: 220,
                        height: 260,
                    },
                },
                command_start: 0,
            }),
        };
        let bytes = encode_snapshot(5, 7, &output).expect("snapshot");
        assert_eq!(i32::from_le_bytes(bytes[80..84].try_into().unwrap()), 410);
        assert_eq!(u32::from_le_bytes(bytes[96..100].try_into().unwrap()), 0);
        assert_eq!(
            &bytes[usize::from(SNAPSHOT_HEADER_BYTES)..],
            b"Save Document As",
        );
    }
    #[test]
    fn document_alignment_edit_flags_preserve_legacy_record_geometry_and_offsets() {
        use rice_ui::grid_frame::{GridFont,GridFontSize};
        for size in [12.0,11.25,409.0] {
            for zoom in [70,100,150] {
                for height in [20,180] {
                    let font=GridFont{family:"Arial".into(),size_pt:GridFontSize::new(size).unwrap(),weight:700,italic:true,underline:true};
                    let rect=Rect{origin:Point{x:4,y:8},size:Size{width:210,height}};
                    let mut output=RenderOutput{size:Size{width:240,height:200},commands:vec![DrawCommand::StyledEdit{zoom_percent:zoom,rect,text:"Wi😀i".into(),color:Color::rgba(1,2,3,255),font,caret:Some(3),selection:Some((1,3)),selection_color:Color::rgba(4,5,6,255),vertical:0}],hit_targets:vec![],focused:None,cursor:crate::native_ui::PointerCursor::Default,dialog:None,popup:None};
                    let legacy=encode_snapshot(1,1,&output).unwrap();
                    let offset=usize::from(SNAPSHOT_HEADER_BYTES);
                    assert_eq!(legacy[offset],if size==11.25 {32}else{28});
                    assert_eq!(legacy[offset+1],0);
                    for alignment in [0,1,2] {
                        if let DrawCommand::StyledEdit{vertical,..}=&mut output.commands[0] {*vertical=alignment;}
                        let actual=encode_snapshot(1,1,&output).unwrap();
                        let mut expected=legacy.clone();expected[offset+1]=alignment<<2;
                        assert_eq!(actual,expected,"alignment changes only the negotiated flag bits");
                    }
                    for invalid in [3,255] {
                        if let DrawCommand::StyledEdit{vertical,..}=&mut output.commands[0] {*vertical=invalid;}
                        assert!(matches!(encode_snapshot(1,1,&output),Err(SnapshotError::RecordLimit)));
                    }
                }
            }
        }
    }

}
