//! Opt-in part skins replace their complete native face, including artwork.
use super::*;

fn state(widget: &Widget, rect: Rect, point: Option<Point>, pressed: bool, focused: bool) -> u8 {
    let hovered = point.is_some_and(|p| rect.contains(p));
    if !widget.state.contains(WidgetState::ENABLED) {
        4
    } else if hovered && (pressed || widget.state.contains(WidgetState::PRESSED)) {
        2
    } else if hovered {
        1
    } else if focused || widget.state.contains(WidgetState::FOCUSED) {
        3
    } else {
        0
    }
}
fn part(
    rect: Rect,
    role: u8,
    state: u8,
    color: rice_ui::Color,
    theme: &Theme,
    commands: &mut Vec<DrawCommand>,
) -> Result<(), NativeUiError> {
    if rect.size.width == 0 || rect.size.height == 0 {
        return Ok(());
    }
    if !theme.control_chrome.styled(role, state) {
        push_command(
            commands,
            DrawCommand::Fill {
                rect,
                color,
                radius: 0,
            },
        )?;
    }
    push_command(commands, DrawCommand::ThemeSurface { rect, role, state })
}
pub(super) fn scrollbar(
    rect: Rect,
    position: u32,
    total: u32,
    extent: u32,
    state: u8,
    theme: &Theme,
    commands: &mut Vec<DrawCommand>,
) -> Result<(), NativeUiError> {
    let parts = super::super::control_chrome::scrollbar(rect, position, total, extent, theme);
    let vertical = rect.size.height > rect.size.width;
    let axis = u8::from(vertical);
    part(
        parts.track,
        21 + axis,
        state,
        theme.palette.surface_raised,
        theme,
        commands,
    )?;
    for (button, role, leading) in [
        (parts.decrement, 25 + axis, true),
        (parts.increment, 27 + axis, false),
    ] {
        part(button, role, state, theme.palette.surface, theme, commands)?;
        if button.size.width > 0
            && button.size.height > 0
        {
            super::scrollbar::render_pixel_arrow(
                button,
                vertical,
                leading,
                theme.palette.text,
                commands,
            )?;
        }
    }
    part(
        parts.thumb,
        23 + axis,
        state,
        theme.palette.accent,
        theme,
        commands,
    )?;
    if theme.control_chrome.styled(29 + axis, state) {
        part(
            parts.grip,
            29 + axis,
            state,
            theme.palette.text,
            theme,
            commands,
        )?;
    }
    Ok(())
}
pub(super) fn widget_scrollbar(
    widget: &Widget,
    rect: Rect,
    point: Option<Point>,
    pressed: bool,
    focused: bool,
    theme: &Theme,
    commands: &mut Vec<DrawCommand>,
) -> Result<(), NativeUiError> {
    let (position, total) = super::super::scroll::fraction(widget.text.as_deref());
    let extent = super::super::scroll::thumb_extent(rect, widget.text.as_deref());
    let parts = super::super::control_chrome::scrollbar(rect, position, total, extent, theme);
    let axis = u8::from(rect.size.height > rect.size.width);
    for (part_rect, role, color) in [
        (parts.track, 21 + axis, theme.palette.surface_raised),
        (parts.decrement, 25 + axis, theme.palette.surface),
        (parts.increment, 27 + axis, theme.palette.surface),
        (parts.thumb, 23 + axis, theme.palette.accent),
        (parts.grip, 29 + axis, theme.palette.text),
    ] {
        let state = state(widget, part_rect, point, pressed, focused);
        if role == 29 + axis && !theme.control_chrome.styled(role, state) {
            continue;
        }
        part(part_rect, role, state, color, theme, commands)?;
        if (role == 25 + axis || role == 27 + axis)
            && part_rect.size.width > 0
            && part_rect.size.height > 0
        {
            super::scrollbar::render_pixel_arrow(
                part_rect,
                axis == 1,
                role == 25 + axis,
                theme.palette.text,
                commands,
            )?;
        }
    }
    Ok(())
}
pub(super) fn slider(
    widget: &Widget,
    rect: Rect,
    point: Option<Point>,
    pressed: bool,
    focused: bool,
    theme: &Theme,
    commands: &mut Vec<DrawCommand>,
) -> Result<(), NativeUiError> {
    let (position, total) = super::super::scroll::fraction(widget.text.as_deref());
    let parts = super::super::control_chrome::slider(rect, position, total, theme);
    part(
        parts.track,
        31,
        state(widget, parts.track, point, pressed, focused),
        theme.palette.surface_raised,
        theme,
        commands,
    )?;
    part(
        parts.fill,
        32,
        state(widget, parts.fill, point, pressed, focused),
        theme.palette.accent,
        theme,
        commands,
    )?;
    part(
        parts.thumb,
        33,
        state(widget, parts.thumb, point, pressed, focused),
        theme.palette.surface,
        theme,
        commands,
    )
}
