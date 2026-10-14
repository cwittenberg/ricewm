//! Shared scrollbar value parsing and pointer geometry.

use rice_ui::{Point, Rect};

pub(super) const COLOR_COLUMNS: u32 = 24;
pub(super) const COLOR_ROWS: u32 = 6;

pub(super) fn fraction(text: Option<&str>) -> (u32, u32) {
    let mut parts = super::layout_tokens::scroll_value(text)
        .unwrap_or_default()
        .split('/');
    let position = parts
        .next()
        .and_then(|part| part.trim().parse::<u32>().ok())
        .unwrap_or(0);
    let total = parts
        .next()
        .and_then(|part| part.split(';').next())
        .and_then(|part| part.trim().parse::<u32>().ok())
        .unwrap_or(100)
        .max(1);
    (position.min(total), total)
}

pub(super) fn viewport(text: Option<&str>) -> Option<u32> {
    text?.split(';').find_map(|part| part.strip_prefix("viewport=")?.parse::<u32>().ok()).filter(|value| *value > 0)
}

pub(super) fn thumb_extent(rect: Rect, text: Option<&str>) -> u32 {
    let vertical = rect.size.height > rect.size.width;
    let (major, minor) = if vertical { (rect.size.height, rect.size.width) } else { (rect.size.width, rect.size.height) };
    if let Some(viewport) = viewport(text) {
        let maximum = fraction(text).1;
        let content = u64::from(maximum) + u64::from(viewport);
        return ((u64::from(major)*u64::from(viewport)/content) as u32).max(minor.max(18)).min(major);
    }
    major.div_ceil(4).max(minor).min(major)
}

pub(super) fn compact_track(rect: Rect) -> Rect {
    if rect.size.height > rect.size.width {
        let width = rect.size.width.min(8);
        Rect { origin: Point { x: rect.origin.x.saturating_add_unsigned(rect.size.width.saturating_sub(width)/2), y: rect.origin.y },
            size: rice_ui::Size { width, height: rect.size.height } }
    } else {
        let height = rect.size.height.min(8);
        Rect { origin: Point { x: rect.origin.x, y: rect.origin.y.saturating_add_unsigned(rect.size.height.saturating_sub(height)/2) },
            size: rice_ui::Size { width: rect.size.width, height } }
    }
}

pub(super) fn metadata_pointer_position(rect: Rect, point: Point, text: Option<&str>, flat: bool) -> u32 {
    let rect = if flat { compact_track(rect) } else { rect };
    pointer_position_with_extent(rect, point, fraction(text).1, thumb_extent(rect, text))
}

#[cfg(test)]
mod viewport_tests {
    use super::*;
    #[test]
    fn explicit_viewport_paints_and_drags_the_same_proportional_thumb() {
        let rect=Rect {origin:Point{x:400,y:100},size:rice_ui::Size{width:80,height:400}};
        let track=compact_track(rect);
        let value=Some("0/1200;viewport=400");
        assert_eq!(thumb_extent(track,value),100);
        assert_eq!(metadata_pointer_position(rect,Point{x:440,y:150},value,true),0);
        assert_eq!(metadata_pointer_position(rect,Point{x:440,y:300},value,true),600);
        assert_eq!(metadata_pointer_position(rect,Point{x:440,y:450},value,true),1200);
        assert_eq!(thumb_extent(track,Some("0/40000;viewport=400")),18);
        assert_eq!(thumb_extent(track,Some("0/1200")),100,"legacy controls retain their geometry");
    }
}

pub(super) fn ticks(text: Option<&str>) -> u32 {
    text.unwrap_or_default()
        .split(';')
        .find_map(|part| part.strip_prefix("ticks="))
        .and_then(|part| part.trim().parse::<u32>().ok())
        .unwrap_or(0)
        .min(21)
}

pub(super) fn pointer_position(rect: Rect, point: Point, total: u32) -> u32 {
    let vertical = rect.size.height > rect.size.width;
    let major = if vertical {
        rect.size.height
    } else {
        rect.size.width
    };
    let minor = if vertical {
        rect.size.width
    } else {
        rect.size.height
    };
    let extent = major.div_ceil(4).max(minor).min(major);
    pointer_position_with_extent(rect, point, total, extent)
}

pub(super) fn scroll_view_pointer_position(
    rect: Rect,
    point: Point,
    viewport_extent: u32,
    content_extent: u32,
) -> u32 {
    let maximum = content_extent.saturating_sub(viewport_extent);
    let extent = u32::try_from(
        u64::from(rect.size.height).saturating_mul(u64::from(viewport_extent))
            / u64::from(content_extent.max(1)),
    )
    .unwrap_or(rect.size.height)
    .max(rect.size.width)
    .min(rect.size.height);
    pointer_position_with_extent(rect, point, maximum, extent)
}

fn pointer_position_with_extent(rect: Rect, point: Point, total: u32, extent: u32) -> u32 {
    let vertical = rect.size.height > rect.size.width;
    let major = if vertical {
        rect.size.height
    } else {
        rect.size.width
    };
    let travel = major.saturating_sub(extent);
    if travel == 0 {
        return 0;
    }
    let coordinate = if vertical {
        point.y.saturating_sub(rect.origin.y)
    } else {
        point.x.saturating_sub(rect.origin.x)
    };
    let centered = coordinate.saturating_sub_unsigned(extent / 2).max(0);
    let offset = centered.cast_unsigned().min(travel);
    u32::try_from(u64::from(offset).saturating_mul(u64::from(total)) / u64::from(travel))
        .unwrap_or(total)
        .min(total)
}

pub(super) fn progress_position(rect: Rect, point: Point, total: u32) -> u32 {
    let offset = point
        .x
        .saturating_sub(rect.origin.x)
        .max(0)
        .cast_unsigned()
        .min(rect.size.width);
    u32::try_from(
        u64::from(offset).saturating_mul(u64::from(total)) / u64::from(rect.size.width.max(1)),
    )
    .unwrap_or(total)
    .min(total)
}

pub(super) fn color_picker_value(rect: Rect, point: Point) -> String {
    let x = point
        .x
        .saturating_sub(rect.origin.x)
        .max(0)
        .cast_unsigned()
        .min(rect.size.width.saturating_sub(1));
    let y = point
        .y
        .saturating_sub(rect.origin.y)
        .max(0)
        .cast_unsigned()
        .min(rect.size.height.saturating_sub(1));
    let column = u32::try_from(
        u64::from(x).saturating_mul(u64::from(COLOR_COLUMNS)) / u64::from(rect.size.width.max(1)),
    )
    .unwrap_or(COLOR_COLUMNS - 1)
    .min(COLOR_COLUMNS - 1);
    let row = u32::try_from(
        u64::from(y).saturating_mul(u64::from(COLOR_ROWS)) / u64::from(rect.size.height.max(1)),
    )
    .unwrap_or(COLOR_ROWS - 1)
    .min(COLOR_ROWS - 1);
    let [red, green, blue] = color_picker_rgb(column, row);
    format!("#{red:02x}{green:02x}{blue:02x}")
}

pub(super) fn color_picker_rgb(column: u32, row: u32) -> [u8; 3] {
    let column = column.min(COLOR_COLUMNS - 1);
    if row.min(COLOR_ROWS - 1) == 0 {
        let level = column.saturating_mul(255) / (COLOR_COLUMNS - 1);
        let level = u8::try_from(level).unwrap_or(u8::MAX);
        return [level, level, level];
    }
    let hue = column.min(COLOR_COLUMNS - 1).saturating_mul(1_536) / COLOR_COLUMNS;
    let (saturation, value) = match row.min(COLOR_ROWS - 1) {
        1 => (56, 255),
        2 => (136, 255),
        3 => (255, 255),
        4 => (255, 190),
        _ => (255, 124),
    };
    hsv_rgb(hue, saturation, value)
}

fn hsv_rgb(hue: u32, saturation: u32, value: u32) -> [u8; 3] {
    let sector = (hue / 256).min(5);
    let fraction = hue % 256;
    let low = value.saturating_mul(255 - saturation) / 255;
    let falling = value.saturating_mul(255 - saturation.saturating_mul(fraction) / 255) / 255;
    let rising = value.saturating_mul(255 - saturation.saturating_mul(255 - fraction) / 255) / 255;
    let (red, green, blue) = match sector {
        0 => (value, rising, low),
        1 => (falling, value, low),
        2 => (low, value, rising),
        3 => (low, falling, value),
        4 => (rising, low, value),
        _ => (value, low, falling),
    };
    [red as u8, green as u8, blue as u8]
}

#[cfg(test)]
mod color_tests {
    use super::*;

    #[test]
    fn color_picker_spans_black_white_bright_muted_and_dark_colors() {
        assert_eq!(color_picker_rgb(0, 0), [0, 0, 0]);
        assert_eq!(color_picker_rgb(COLOR_COLUMNS - 1, 0), [255, 255, 255]);
        assert_eq!(color_picker_rgb(0, 3), [255, 0, 0]);
        assert_eq!(color_picker_rgb(8, 3), [0, 255, 0]);
        assert_eq!(color_picker_rgb(16, 3), [0, 0, 255]);
        assert!(color_picker_rgb(0, 1)[1] > 150);
        assert!(color_picker_rgb(0, 5)[0] < 160);
    }

    #[test]
    fn pointer_selection_is_a_valid_hex_color() {
        let rect = Rect {
            origin: Point { x: 10, y: 20 },
            size: rice_ui::Size {
                width: 480,
                height: 120,
            },
        };
        assert_eq!(color_picker_value(rect, Point { x: 10, y: 20 }), "#000000");
        assert_eq!(color_picker_value(rect, Point { x: 489, y: 20 }), "#ffffff",);
        let value = color_picker_value(rect, Point { x: 250, y: 90 });
        assert!(value.starts_with('#'));
        assert_eq!(value.len(), 7);
        assert!(value[1..].bytes().all(|byte| byte.is_ascii_hexdigit()));
    }
}

// Continuous HSV controls: a saturation/value plane and a separate hue strip.
pub(super) fn continuous_color_value(text: &str, rect: Rect, point: Point) -> String {
    let rgb = text.split('#').nth(1).and_then(|s| u32::from_str_radix(s.split('@').next()?,16).ok()).unwrap_or(0xffffff);
    let r = ((rgb >> 16) & 255) as f32 / 255.;
    let g = ((rgb >> 8) & 255) as f32 / 255.;
    let b = (rgb & 255) as f32 / 255.;
    let maximum = r.max(g).max(b); let minimum = r.min(g).min(b); let delta = maximum - minimum;
    let mut h = if delta < 0.00001 {0.} else if maximum == r { ((g-b)/delta).rem_euclid(6.) / 6. }
        else if maximum == g { ((b-r)/delta + 2.) / 6. } else { ((r-g)/delta + 4.) / 6. };
    if let Some(degrees) = text.split('@').nth(1).and_then(|s|s.parse::<u16>().ok()) { h = f32::from(degrees.min(360))/360.; }
    let mut s = if maximum == 0. {0.} else {delta/maximum}; let mut v = maximum;
    let x = ((point.x - rect.origin.x) as f32 / rect.size.width.saturating_sub(1).max(1) as f32).clamp(0.,1.);
    let y = ((point.y - rect.origin.y) as f32 / rect.size.height.saturating_sub(1).max(1) as f32).clamp(0.,1.);
    if text.starts_with("color-hue:") { h = x; if s < 0.01 {s = 1.;} if v < 0.01 {v = 1.;} }
    else { s = x; v = 1. - y; }
    let c = v*s; let z = c * (1. - ((h*6.).rem_euclid(2.) - 1.).abs()); let m = v-c;
    let (r,g,b) = match ((h*6.).floor() as u8) % 6 {0=>(c,z,0.),1=>(z,c,0.),2=>(0.,c,z),3=>(0.,z,c),4=>(z,0.,c),_=>(c,0.,z)};
    format!("#{:02x}{:02x}{:02x}@{}",((r+m)*255.).round() as u8,((g+m)*255.).round() as u8,((b+m)*255.).round() as u8,(h*360.).round() as u16)
}

#[cfg(test)]
#[path = "../../../../tests/color-picker.rs"]
mod color_picker_tests;
