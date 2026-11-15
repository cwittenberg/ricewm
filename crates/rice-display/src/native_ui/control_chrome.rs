//! One geometry model for themed controls, drawing and pointer input.
use rice_ui::{ChromeProfile, Insets, Point, Rect, Size, Theme};

pub(super) struct ScrollParts {
    pub vertical: bool,
    pub track: Rect,
    pub thumb: Rect,
    pub decrement: Rect,
    pub increment: Rect,
    pub secondary_decrement: Option<Rect>,
    pub secondary_increment: Option<Rect>,
    pub grip: Rect,
}

impl ScrollParts {
    pub fn decrement_contains(&self, point: Point) -> bool {
        self.decrement.contains(point)
            || self.secondary_decrement.is_some_and(|r| r.contains(point))
    }
    pub fn increment_contains(&self, point: Point) -> bool {
        self.increment.contains(point)
            || self.secondary_increment.is_some_and(|r| r.contains(point))
    }
}

pub(super) fn scrollbar(
    rect: Rect,
    position: u32,
    total: u32,
    extent: u32,
    theme: &Theme,
) -> ScrollParts {
    let c = theme.control_chrome;
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
    let paired = theme.chrome_profile == ChromeProfile::YellowJacket && !c.scrollbar();
    let arrow = c.metric(0, minor).min(major / if paired { 4 } else { 2 });
    let placement = c.metric(8, 0);
    let local = |offset: u32, length: u32| Rect {
        origin: Point {
            x: rect
                .origin
                .x
                .saturating_add_unsigned(if vertical { 0 } else { offset }),
            y: rect
                .origin
                .y
                .saturating_add_unsigned(if vertical { offset } else { 0 }),
        },
        size: Size {
            width: if vertical { minor } else { length },
            height: if vertical { length } else { minor },
        },
    };
    let track_length = major.saturating_sub(arrow * if paired { 4 } else { 2 });
    let track = local(
        if paired || placement == 1 {
            arrow * 2
        } else if placement == 2 {
            0
        } else {
            arrow
        },
        track_length,
    );
    let minimum = c.metric(1, minor.max(18));
    let thumb_length = ((u64::from(extent) * u64::from(track_length) / u64::from(major.max(1)))
        as u32)
        .max(minimum)
        .min(track_length);
    let travel = track_length.saturating_sub(thumb_length);
    let offset =
        (u64::from(travel) * u64::from(position.min(total)) / u64::from(total.max(1))) as u32;
    let thumb = Rect {
        origin: Point {
            x: track
                .origin
                .x
                .saturating_add_unsigned(if vertical { 0 } else { offset }),
            y: track
                .origin
                .y
                .saturating_add_unsigned(if vertical { offset } else { 0 }),
        },
        size: Size {
            width: if vertical { minor } else { thumb_length },
            height: if vertical { thumb_length } else { minor },
        },
    };
    let inset = c.metric(2, 0).min(minor.saturating_sub(1) / 2);
    let thumb = thumb.inset(if vertical {
        Insets {
            left: inset,
            right: inset,
            top: 0,
            bottom: 0,
        }
    } else {
        Insets {
            top: inset,
            bottom: inset,
            left: 0,
            right: 0,
        }
    });
    let grip = Rect {
        origin: Point {
            x: thumb
                .origin
                .x
                .saturating_add_unsigned(thumb.size.width.saturating_sub(7) / 2),
            y: thumb
                .origin
                .y
                .saturating_add_unsigned(thumb.size.height.saturating_sub(7) / 2),
        },
        size: Size {
            width: thumb.size.width.min(7),
            height: thumb.size.height.min(7),
        },
    };
    ScrollParts {
        vertical,
        track,
        thumb,
        grip,
        decrement: local(if placement == 2 { major - arrow * 2 } else { 0 }, arrow),
        increment: local(if placement == 1 { arrow } else { major - arrow }, arrow),
        secondary_decrement: paired.then(|| local(major - arrow * 2, arrow)),
        secondary_increment: paired.then(|| local(arrow, arrow)),
    }
}

pub(super) fn scroll_position(parts: &ScrollParts, point: Point, total: u32) -> u32 {
    let vertical = parts.vertical;
    let (coordinate, major, extent) = if vertical {
        (
            point.y - parts.track.origin.y,
            parts.track.size.height,
            parts.thumb.size.height,
        )
    } else {
        (
            point.x - parts.track.origin.x,
            parts.track.size.width,
            parts.thumb.size.width,
        )
    };
    let travel = major.saturating_sub(extent);
    let offset = coordinate
        .saturating_sub_unsigned(extent / 2)
        .max(0)
        .cast_unsigned()
        .min(travel);
    (u64::from(offset) * u64::from(total) / u64::from(travel.max(1))) as u32
}

pub(super) struct SliderParts {
    pub track: Rect,
    pub fill: Rect,
    pub thumb: Rect,
}
pub(super) fn slider(rect: Rect, position: u32, total: u32, theme: &Theme) -> SliderParts {
    if !theme.control_chrome.slider() {
        let redmond = theme.chrome_profile == ChromeProfile::Redmond98;
        let width = if redmond { 11 } else { u32::from(theme.metrics.control_height) * 2 / 3 };
        let height = if redmond { 18 } else { width };
        let margin = if redmond { 7 } else { (width / 2 + 1).max(6) };
        let track_height = if redmond { 4 } else if theme.chrome_profile == ChromeProfile::Orchard { 6 }
            else { (u32::from(theme.metrics.corner_radius) * 2).max(6) };
        let track = Rect { origin: Point { x: rect.origin.x.saturating_add_unsigned(margin),
            y: rect.origin.y.saturating_add_unsigned(rect.size.height / 2).saturating_sub_unsigned(track_height / 2) },
            size: Size { width: rect.size.width.saturating_sub(margin * 2), height: track_height } };
        let offset = (u64::from(track.size.width) * u64::from(position.min(total)) / u64::from(total.max(1))) as u32;
        return SliderParts { track, fill: Rect { size: Size { width: offset, ..track.size }, ..track },
            thumb: Rect { origin: Point { x: track.origin.x.saturating_add_unsigned(offset).saturating_sub_unsigned(width / 2),
                y: rect.origin.y.saturating_add_unsigned(rect.size.height.saturating_sub(height) / 2) },
                size: Size { width, height: height.min(rect.size.height) } } };
    }
    let c = theme.control_chrome;
    let width = c
        .metric(4, u32::from(theme.metrics.control_height) * 2 / 3)
        .min(rect.size.width);
    let height = c.metric(5, width).min(rect.size.height);
    let track_height = c.metric(3, 4).min(rect.size.height);
    let track = Rect {
        origin: Point {
            x: rect.origin.x.saturating_add_unsigned(width / 2),
            y: rect
                .origin
                .y
                .saturating_add_unsigned(rect.size.height.saturating_sub(track_height) / 2),
        },
        size: Size {
            width: rect.size.width.saturating_sub(width),
            height: track_height,
        },
    };
    let offset = (u64::from(track.size.width) * u64::from(position.min(total))
        / u64::from(total.max(1))) as u32;
    SliderParts {
        track,
        fill: Rect {
            size: Size {
                width: offset,
                ..track.size
            },
            ..track
        },
        thumb: Rect {
            origin: Point {
                x: rect.origin.x.saturating_add_unsigned(offset),
                y: rect
                    .origin
                    .y
                    .saturating_add_unsigned(rect.size.height.saturating_sub(height) / 2),
            },
            size: Size { width, height },
        },
    }
}
pub(super) fn slider_position(rect: Rect, point: Point, theme: &Theme) -> u32 {
    let parts = slider(rect, 0, 1000, theme);
    let offset = point
        .x
        .saturating_sub(parts.track.origin.x)
        .max(0)
        .cast_unsigned()
        .min(parts.track.size.width);
    (u64::from(offset) * 1000 / u64::from(parts.track.size.width.max(1))) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scrollbar_placement_and_zero_arrows_keep_endpoints_reachable() {
        for vertical in [false, true] {
            let rect = Rect {
                origin: Point { x: 10, y: 20 },
                size: if vertical {
                    Size {
                        width: 18,
                        height: 300,
                    }
                } else {
                    Size {
                        width: 300,
                        height: 18,
                    }
                },
            };
            for placement in 0..3 {
                let mut theme = Theme::ricewm_default();
                theme.control_chrome.metrics_mask = 0x103;
                theme.control_chrome.metrics[0] = 18;
                theme.control_chrome.metrics[1] = 24;
                theme.control_chrome.metrics[8] = placement;
                let first = scrollbar(rect, 0, 100, 100, &theme);
                let last = scrollbar(rect, 100, 100, 100, &theme);
                assert!(!first.decrement.intersection(first.track).is_some());
                assert!(!first.increment.intersection(first.track).is_some());
                let center = |r: Rect| Point {
                    x: r.origin.x + (r.size.width / 2) as i32,
                    y: r.origin.y + (r.size.height / 2) as i32,
                };
                assert_eq!(scroll_position(&first, center(first.thumb), 100), 0);
                assert_eq!(scroll_position(&last, center(last.thumb), 100), 100);
                theme.control_chrome.metrics[0] = 0;
                let no_arrows = scrollbar(rect, 0, 100, 100, &theme);
                assert_eq!(no_arrows.track, rect);
            }
        }
    }
}
