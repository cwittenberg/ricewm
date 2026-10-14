use rice_ui::{Rect, Theme, Widget, WidgetKind};

use super::{helpers::push_command, icon::media_rect};
use crate::native_ui::{DrawCommand, MediaScaleMode, NativeUiError};

pub(super) fn render(
    widget: &Widget,
    rect: Rect,
    theme: &Theme,
    commands: &mut Vec<DrawCommand>,
) -> Result<bool, NativeUiError> {
    if widget.kind != WidgetKind::Image {
        return Ok(false);
    }
    let rect = media_rect(widget, rect, theme);
    if let Some(data_url) = widget.text.as_deref().and_then(parse_inline) {
        push_command(
            commands,
            DrawCommand::InlineImage {
                rect,
                data_url: data_url.to_owned(),
            },
        )?;
        return Ok(true);
    }
    let Some(media) = widget.text.as_deref().and_then(parse) else {
        return Ok(false);
    };
    push_command(
        commands,
        DrawCommand::Media {
            rect,
            request: media.request,
            quarter_turns: media.quarter_turns,
            scale_mode: media.scale_mode,
            scale_permille: media.scale_permille,
            pan_x: media.pan_x,
            pan_y: media.pan_y,
            image_edit: media.image_edit,
        },
    )?;
    Ok(true)
}

fn parse_inline(text: &str) -> Option<&str> {
    let value = text.strip_prefix("inline-image:")?;
    (value.starts_with("data:image/") && value.len() <= rice_ui::MAX_INLINE_IMAGE_TEXT_BYTES - 13)
        .then_some(value)
}

struct Presentation {
    request: u64,
    quarter_turns: u8,
    scale_mode: MediaScaleMode,
    scale_permille: u16,
    pan_x: i16,
    pan_y: i16,
    image_edit: Option<[u16; 6]>,
}

fn parse(text: &str) -> Option<Presentation> {
    let value = text.strip_prefix("media-request:")?;
    let mut parts = value.split(':');
    let request = parts.next()?.parse::<u64>().ok()?;
    if request == 0 {
        return None;
    }
    let Some(turns) = parts.next() else {
        return Some(Presentation {
            request,
            quarter_turns: 0,
            scale_mode: MediaScaleMode::Fit,
            scale_permille: 1_000,
            pan_x: 0,
            pan_y: 0,
            image_edit: None,
        });
    };
    let quarter_turns = turns.parse::<u8>().ok()?;
    let scale_mode = match parts.next()? {
        "fit" => MediaScaleMode::Fit,
        "actual" => MediaScaleMode::Actual,
        _ => return None,
    };
    let scale_permille = parts.next()?.parse::<u16>().ok()?;
    let pan_x = parts
        .next()
        .map_or(Some(0), |value| value.parse::<i16>().ok())?;
    let pan_y = parts
        .next()
        .map_or(Some(0), |value| value.parse::<i16>().ok())?;
    let image_edit = if let Some(x) = parts.next() {
        let values = [x.parse().ok()?, parts.next()?.parse().ok()?, parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?, parts.next()?.parse().ok()?, parts.next()?.parse().ok()?];
        let [x, y, w, h, ow, oh]: [u16; 6] = values;
        if w == 0 || h == 0 || ow == 0 || oh == 0 || ow > 4096 || oh > 4096
            || u32::from(x) + u32::from(w) > 4096 || u32::from(y) + u32::from(h) > 4096 { return None; }
        Some(values)
    } else { None };
    if quarter_turns > 3 || !(10..=64_000).contains(&scale_permille) || parts.next().is_some() {
        return None;
    }
    Some(Presentation {
        request,
        quarter_turns,
        scale_mode,
        scale_permille,
        pan_x,
        pan_y,
        image_edit,
    })
}
