//! Shared contain-fit geometry for native vector scene input and hover.
use super::{Point, PointerCursor, Rect, WebHoverKind, WebScene};

/// Opt-in intrinsic page framing. Dimensions are emitted by the SceneSurface
/// SDK from the submitted scene; ordinary surfaces retain their old geometry.
pub(super) fn page_size(text: Option<&str>) -> Option<(u32, u32)> {
    let mut parts = text?.strip_prefix("scene:page:")?.split(':');
    let width = parts.next()?.parse::<u32>().ok()?;
    let height = parts.next()?.parse::<u32>().ok()?;
    if !(1..=16_384).contains(&width) || !(1..=16_384).contains(&height) {
        return None;
    }
    Some((width, height))
}

pub(super) fn page_rect(rect: Rect, text: Option<&str>) -> Rect {
    let Some((width, height)) = page_size(text) else { return rect };
    let margin = 20_u32.min(rect.size.width / 8).min(rect.size.height / 8);
    let available = rect.inset(rice_ui::Insets::all(margin));
    if let Some((zoom, pan_x, pan_y)) = page_zoom(text) {
        let scaled = rice_ui::Size { width: width.saturating_mul(zoom).checked_div(100).unwrap_or(1).max(1),
            height: height.saturating_mul(zoom).checked_div(100).unwrap_or(1).max(1) };
        let offset = |available: u32, page: u32, pan: u32| -> i32 {
            if page <= available { i32::try_from((available-page)/2).unwrap_or_default() }
            else { -i32::try_from(u64::from(page-available)*u64::from(pan)/1_000_000).unwrap_or(i32::MAX) }
        };
        return Rect { origin: Point { x: available.origin.x.saturating_add(offset(available.size.width,scaled.width,pan_x)),
            y: available.origin.y.saturating_add(offset(available.size.height,scaled.height,pan_y)) }, size: scaled };
    }
    let (fitted_width, fitted_height) = if u64::from(available.size.width) * u64::from(height)
        <= u64::from(available.size.height) * u64::from(width)
    {
        (available.size.width, u32::try_from(u64::from(available.size.width) * u64::from(height) / u64::from(width)).unwrap_or(available.size.height))
    } else {
        (u32::try_from(u64::from(available.size.height) * u64::from(width) / u64::from(height)).unwrap_or(available.size.width), available.size.height)
    };
    Rect { origin: Point {
        x: available.origin.x.saturating_add_unsigned((available.size.width - fitted_width) / 2),
        y: available.origin.y.saturating_add_unsigned((available.size.height - fitted_height) / 2),
    }, size: rice_ui::Size { width: fitted_width, height: fitted_height } }
}

fn page_zoom(text: Option<&str>) -> Option<(u32,u32,u32)> {
    let (_, value) = text?.split(":cursor:").next()?.split_once(":zoom:")?;
    let mut parts = value.split(':');
    let zoom = parts.next()?.parse::<u32>().ok().filter(|value| (10..=400).contains(value))?;
    if parts.next()? != "pan" { return None; }
    let x = parts.next()?.parse::<u32>().ok().filter(|value| *value <= 1_000_000)?;
    let y = parts.next()?.parse::<u32>().ok().filter(|value| *value <= 1_000_000)?;
    if parts.next().is_some() { return None; }
    Some((zoom,x,y))
}

pub(super) fn logical_point(scene: &WebScene, surface: Rect, point: Point) -> (f64, f64) {
    let width = f64::from(scene.viewport_width.max(1));
    let height = f64::from(scene.viewport_height.max(1));
    let scale = (f64::from(surface.size.width.max(1)) / width)
        .min(f64::from(surface.size.height.max(1)) / height);
    let left = (f64::from(surface.size.width) - width * scale) / 2.0;
    let top = (f64::from(surface.size.height) - height * scale) / 2.0;
    (
        (f64::from(point.x) - f64::from(surface.origin.x) - left) / scale,
        (f64::from(point.y) - f64::from(surface.origin.y) - top) / scale,
    )
}

#[allow(clippy::cast_possible_truncation)]
pub(super) fn pointer_coordinate(value: f64) -> i32 {
    // floor keeps points immediately outside the letterbox negative instead
    // of accidentally reporting coordinate zero. Rust casts saturate overflow.
    value.floor() as i32
}

pub(super) fn drawing_cursor(text: Option<&str>) -> Option<PointerCursor> {
    let text = text?.strip_prefix("scene:")?;
    match text.rsplit_once(":cursor:")?.1 {
        "pointer" => Some(PointerCursor::Pointer),
        "crosshair" => Some(PointerCursor::Crosshair),
        "col-resize" => Some(PointerCursor::ColumnResize),
        "row-resize" => Some(PointerCursor::RowResize),
        _ => None,
    }
}

pub(super) fn cursor(scene: &WebScene, surface: Rect, point: Point) -> PointerCursor {
    if !surface.contains(point) {
        return PointerCursor::Default;
    }
    let (x, y) = logical_point(scene, surface, point);
    if x < 0.0
        || y < 0.0
        || x >= f64::from(scene.viewport_width)
        || y >= f64::from(scene.viewport_height)
    {
        return PointerCursor::Default;
    }
    if scene.hover_regions.iter().any(|region| {
        let [left, top, width, height] = region.rect().map(f64::from);
        region.kind == WebHoverKind::Pointer
            && x >= left
            && y >= top
            && x < left + width
            && y < top + height
    }) {
        PointerCursor::Pointer
    } else {
        PointerCursor::Default
    }
}

/// Surface background is independent of the contain-fit scene viewport, so it
/// also paints letterboxing. Invalid or absent metadata preserves legacy white.
pub(super) fn background(text: Option<&str>) -> rice_ui::Color {
    let text = text.map(|value| value.split(":cursor:").next().unwrap_or(value));
    if text.is_some_and(|value| value == "scene:bg:transparent"
        || page_size(Some(value)).is_some()
            && value.split_once(":bg:").is_some_and(|(_, color)| color.split(':').next() == Some("transparent"))) {
        return rice_ui::Color::opaque(0, 0, 0).with_alpha(0);
    }
    let Some(hex) = text.and_then(|value| value.strip_prefix("scene:bg:#")
        .or_else(|| page_size(Some(value)).and_then(|_| value.split_once(":bg:#").and_then(|(_, hex)| hex.split(':').next())))) else {
        return rice_ui::Color::opaque(255, 255, 255);
    };
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return rice_ui::Color::opaque(255, 255, 255);
    }
    let value = u32::from_str_radix(hex, 16).unwrap_or(0xffffff);
    let [_, red, green, blue] = value.to_be_bytes();
    rice_ui::Color::opaque(red, green, blue)
}

#[cfg(test)]
mod tests {
    #[test]
    fn framed_pages_fit_portrait_landscape_and_small_workspaces_without_changing_defaults() {
        use super::{page_rect, page_size, background};
        use rice_ui::{Color, Point, Rect, Size};
        for (width, height) in [(1600_u32, 900_u32), (900, 1600), (4000, 1000)] {
            let metadata = format!("scene:page:{width}:{height}:bg:#123456");
            assert_eq!(page_size(Some(&metadata)), Some((width, height)));
            assert_eq!(background(Some(&metadata)), Color::opaque(18, 52, 86));
            for (workspace_width, workspace_height) in [(800, 600), (300, 800), (20, 10)] {
                let workspace = Rect { origin: Point { x: 17, y: 23 }, size: Size { width: workspace_width, height: workspace_height } };
                let page = page_rect(workspace, Some(&metadata));
                assert!(page.origin.x >= workspace.origin.x && page.origin.y >= workspace.origin.y);
                assert!(page.origin.x as i64 + i64::from(page.size.width) <= workspace.origin.x as i64 + i64::from(workspace_width));
                assert!(page.origin.y as i64 + i64::from(page.size.height) <= workspace.origin.y as i64 + i64::from(workspace_height));
                assert!((i64::from(page.size.width)*i64::from(height)-i64::from(page.size.height)*i64::from(width)).abs() < i64::from(width.max(height)));
                assert_eq!(page_rect(workspace, None), workspace);
                assert_eq!(page_rect(workspace, Some("scene:bg:#123456")), workspace);
            }
        }
        for metadata in ["scene:page:0:20", "scene:page:20:0", "scene:page:999999:20", "scene:page:wide:20"] {
            assert_eq!(page_size(Some(metadata)), None);
        }
    }

    #[test]
    fn bounded_background_metadata_is_exact_and_opaque() {
        use super::background;
        use rice_ui::Color;
        assert_eq!(background(Some("scene:bg:transparent")), Color::opaque(0, 0, 0).with_alpha(0));
        assert_eq!(background(Some("scene:bg:transparent:cursor:pointer")), Color::opaque(0, 0, 0).with_alpha(0));
        assert_eq!(background(Some("scene:page:24:24:bg:transparent")), Color::opaque(0, 0, 0).with_alpha(0));
        assert_eq!(background(Some("scene:bg:#000000")), Color::opaque(0, 0, 0));
        assert_eq!(
            background(Some("scene:bg:#01aBfF")),
            Color::opaque(1, 171, 255)
        );
        for value in [
            None,
            Some("#000000"),
            Some("scene:bg:#12345"),
            Some("scene:bg:#1234567"),
            Some("scene:bg:#12é45"),
            Some("scene:bg:#ffffff\n"),
        ] {
            assert_eq!(background(value), Color::opaque(255, 255, 255));
        }
    }
}

/// Scene-relative editor/handle geometry. Micro-units avoid permille rounding
/// at high zoom, and source lookup uses the surface's already-fitted rectangle.
pub(super) fn overlay(text: Option<&str>) -> Option<(rice_native_app_protocol::WidgetId, [i32; 4])> {
    let mut parts = text?.strip_prefix("panel:scene-overlay:")?.split(':');
    let surface = parts.next()?.parse::<u64>().ok().and_then(rice_native_app_protocol::WidgetId::new)?;
    let mut values = [0; 4];
    for value in &mut values { *value = parts.next()?.parse::<i32>().ok()?; }
    if parts.next()? != "flush" || parts.next().is_some()
        || values.iter().any(|value| value.unsigned_abs() > 16_000_000)
        || values[2] <= 0 || values[3] <= 0 { return None; }
    Some((surface, values))
}

pub(super) fn overlay_rect(source: Rect, values: [i32; 4]) -> Rect {
    let scaled = |value: i32, extent: u32| -> i32 {
        i32::try_from(i64::from(value) * i64::from(extent) / 1_000_000).unwrap_or_default()
    };
    Rect { origin: Point { x: source.origin.x.saturating_add(scaled(values[0], source.size.width)),
        y: source.origin.y.saturating_add(scaled(values[1], source.size.height)) },
        size: rice_ui::Size { width: scaled(values[2], source.size.width).unsigned_abs().max(1),
            height: scaled(values[3], source.size.height).unsigned_abs().max(1) } }
}
