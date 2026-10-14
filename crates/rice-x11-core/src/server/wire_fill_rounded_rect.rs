//! Protocol encoding and raster helpers.
use super::*;



#[allow(clippy::too_many_arguments)]
pub(super) fn fill_rounded_rect(
    pixels: &mut [u32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    rect_width: usize,
    rect_height: usize,
    radius: usize,
    color: u32,
) {
    let radius = radius.min(rect_width / 2).min(rect_height / 2);
    if radius == 0 {
        fill_rect(pixels, width, height, x, y, rect_width, rect_height, color);
        return;
    }
    // Test pixel centers against a circle centered on the half-pixel boundary
    // between each cap and the straight section. Integer pixel coordinates put
    // the opposite corners on centers one pixel apart for even-sized shapes,
    // which makes both rounded ends shorter than the middle section.
    let doubled_radius = radius.saturating_mul(2);
    let radius_squared = doubled_radius.saturating_mul(doubled_radius);
    for local_y in 0..rect_height {
        for local_x in 0..rect_width {
            let corner_x = rounded_rect_edge_distance(local_x, rect_width, radius);
            let corner_y = rounded_rect_edge_distance(local_y, rect_height, radius);
            if corner_x == 0
                || corner_y == 0
                || corner_x
                    .saturating_mul(corner_x)
                    .saturating_add(corner_y.saturating_mul(corner_y))
                    <= radius_squared
            {
                set_surface_pixel(pixels, width, height, x + local_x, y + local_y, color);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn fill_chamfered_rect(
    pixels: &mut [u32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    rect_width: usize,
    rect_height: usize,
    chamfer: usize,
    square_left: bool,
    square_right: bool,
    color: u32,
) {
    let chamfer = chamfer.min(rect_width / 2).min(rect_height / 2);
    for local_y in 0..rect_height {
        let edge = local_y.min(rect_height.saturating_sub(local_y + 1));
        let cut = chamfer.saturating_sub(edge);
        let left_cut = if square_left { 0 } else { cut };
        let right_cut = if square_right { 0 } else { cut };
        fill_rect(
            pixels,
            width,
            height,
            x.saturating_add(left_cut),
            y.saturating_add(local_y),
            rect_width.saturating_sub(left_cut.saturating_add(right_cut)),
            1,
            color,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn fill_wedged_rect(
    pixels: &mut [u32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    rect_width: usize,
    rect_height: usize,
    wedge: usize,
    color: u32,
) {
    let wedge = wedge.min(rect_width / 2).min(rect_height / 2);
    for local_y in 0..rect_height {
        // Match the Neon control-box silhouette: clipped upper-left and
        // lower-right corners with short vertical sides between them.
        let left_cut = wedge.saturating_sub(local_y);
        let right_cut = local_y
            .saturating_add(wedge + 1)
            .saturating_sub(rect_height);
        fill_rect(
            pixels,
            width,
            height,
            x.saturating_add(left_cut),
            y.saturating_add(local_y),
            rect_width.saturating_sub(left_cut.saturating_add(right_cut)),
            1,
            color,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn fill_taskbar_control(
    profile: WmChromeProfile,
    pixels: &mut [u32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    rect_width: usize,
    rect_height: usize,
    color: u32,
) {
    if profile == WmChromeProfile::NeonGrid {
        fill_wedged_rect(
            pixels,
            width,
            height,
            x,
            y,
            rect_width,
            rect_height,
            5,
            color,
        );
    } else {
        fill_rect(pixels, width, height, x, y, rect_width, rect_height, color);
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn fill_redmond_bevel(
    pixels: &mut [u32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    rect_width: usize,
    rect_height: usize,
    pressed: bool,
) {
    let colors = if pressed {
        [0x0000_0000, 0x0080_8080, 0x00df_dfdf, 0x00ff_ffff]
    } else {
        [0x00ff_ffff, 0x00df_dfdf, 0x0080_8080, 0x0000_0000]
    };
    for inset in 0..2 {
        let left = x.saturating_add(inset);
        let top = y.saturating_add(inset);
        let edge_width = rect_width.saturating_sub(inset.saturating_mul(2));
        let edge_height = rect_height.saturating_sub(inset.saturating_mul(2));
        fill_rect(
            pixels,
            width,
            height,
            left,
            top,
            edge_width,
            1,
            colors[inset],
        );
        fill_rect(
            pixels,
            width,
            height,
            left,
            top,
            1,
            edge_height,
            colors[inset],
        );
        fill_rect(
            pixels,
            width,
            height,
            left,
            top.saturating_add(edge_height.saturating_sub(1)),
            edge_width,
            1,
            colors[3 - inset],
        );
        fill_rect(
            pixels,
            width,
            height,
            left.saturating_add(edge_width.saturating_sub(1)),
            top,
            1,
            edge_height,
            colors[3 - inset],
        );
    }
}

pub(super) fn rounded_rect_edge_distance(coordinate: usize, extent: usize, radius: usize) -> usize {
    let center = if coordinate < radius {
        radius.saturating_mul(2)
    } else if coordinate >= extent.saturating_sub(radius) {
        extent.saturating_sub(radius).saturating_mul(2)
    } else {
        return 0;
    };
    coordinate
        .saturating_mul(2)
        .saturating_add(1)
        .abs_diff(center)
}

pub(super) fn set_surface_pixel(
    pixels: &mut [u32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    color: u32,
) {
    if x < width && y < height {
        pixels[y * width + x] = color;
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn paint_gnome_symbolic_icon(
    pixels: &mut [u32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    kind: u8,
    color: u32,
) {
    match kind {
        1 => {
            // Network/globe.
            for offset in 2..12 {
                set_surface_pixel(pixels, width, height, x + offset, y + 1, color);
                set_surface_pixel(pixels, width, height, x + offset, y + 12, color);
                set_surface_pixel(pixels, width, height, x + 1, y + offset, color);
                set_surface_pixel(pixels, width, height, x + 12, y + offset, color);
                set_surface_pixel(pixels, width, height, x + 7, y + offset, color);
                set_surface_pixel(pixels, width, height, x + offset, y + 7, color);
            }
        }
        2 => {
            // Share nodes.
            fill_rect(pixels, width, height, x + 1, y + 5, 4, 4, color);
            fill_rect(pixels, width, height, x + 10, y + 1, 4, 4, color);
            fill_rect(pixels, width, height, x + 10, y + 10, 4, 4, color);
            for offset in 0..7 {
                set_surface_pixel(
                    pixels,
                    width,
                    height,
                    x + 4 + offset,
                    y + 6 - offset / 2,
                    color,
                );
                set_surface_pixel(
                    pixels,
                    width,
                    height,
                    x + 4 + offset,
                    y + 8 + offset / 2,
                    color,
                );
            }
        }
        3 | 5 => {
            // Clock/history.
            for offset in 3..11 {
                set_surface_pixel(pixels, width, height, x + offset, y + 1, color);
                set_surface_pixel(pixels, width, height, x + offset, y + 13, color);
                set_surface_pixel(pixels, width, height, x + 1, y + offset, color);
                set_surface_pixel(pixels, width, height, x + 13, y + offset, color);
            }
            fill_rect(pixels, width, height, x + 7, y + 4, 2, 5, color);
            fill_rect(pixels, width, height, x + 7, y + 8, 5, 2, color);
            if kind == 5 {
                fill_rect(pixels, width, height, x, y + 2, 5, 2, color);
                fill_rect(pixels, width, height, x, y + 2, 2, 5, color);
            }
        }
        4 => {
            // Settings gear.
            fill_rect(pixels, width, height, x + 5, y, 4, 15, color);
            fill_rect(pixels, width, height, x, y + 5, 15, 4, color);
            fill_rect(pixels, width, height, x + 3, y + 3, 9, 9, color);
            fill_rect(pixels, width, height, x + 6, y + 6, 3, 3, 0x0032_3232);
        }
        6 => {
            // Link chain.
            fill_rect(pixels, width, height, x + 1, y + 3, 8, 3, color);
            fill_rect(pixels, width, height, x + 6, y + 9, 8, 3, color);
            for offset in 0..5 {
                set_surface_pixel(pixels, width, height, x + 5 + offset, y + 5 + offset, color);
            }
        }
        8 => {
            // Media playback start.
            for row in 1usize..14 {
                let half = row.abs_diff(7);
                fill_rect(
                    pixels,
                    width,
                    height,
                    x + 3,
                    y + row,
                    11usize.saturating_sub(half),
                    1,
                    color,
                );
            }
        }
        9 => {
            // Media playback pause.
            fill_rect(pixels, width, height, x + 3, y + 2, 3, 12, color);
            fill_rect(pixels, width, height, x + 10, y + 2, 3, 12, color);
        }
        10 => {
            // Media playback stop.
            fill_rect(pixels, width, height, x + 3, y + 3, 11, 11, color);
        }
        11 => {
            // Folder or filesystem location.
            fill_rect(pixels, width, height, x + 1, y + 5, 14, 9, color);
            fill_rect(pixels, width, height, x + 2, y + 3, 7, 4, color);
            fill_rect(pixels, width, height, x + 2, y + 7, 12, 5, 0x0032_3232);
        }
        12 => {
            // Map location pin.
            for row in 0..10 {
                let inset = row / 3;
                fill_rect(
                    pixels,
                    width,
                    height,
                    x + 2 + inset,
                    y + row,
                    12usize.saturating_sub(inset * 2),
                    1,
                    color,
                );
            }
            fill_rect(pixels, width, height, x + 7, y + 9, 2, 6, color);
            fill_rect(pixels, width, height, x + 6, y + 4, 4, 4, 0x0032_3232);
        }
        13 => {
            // Copy/clipboard.
            fill_rect(pixels, width, height, x + 5, y + 1, 10, 11, color);
            fill_rect(pixels, width, height, x + 1, y + 5, 10, 10, color);
            fill_rect(pixels, width, height, x + 3, y + 7, 6, 6, 0x0032_3232);
        }
        14 => {
            // Eject.
            for row in 0..8 {
                fill_rect(
                    pixels,
                    width,
                    height,
                    x + 7usize.saturating_sub(row),
                    y + row,
                    row * 2 + 1,
                    1,
                    color,
                );
            }
            fill_rect(pixels, width, height, x + 1, y + 11, 14, 3, color);
        }
        15 => {
            // Alarm/timer.
            for offset in 3..12 {
                set_surface_pixel(pixels, width, height, x + offset, y + 2, color);
                set_surface_pixel(pixels, width, height, x + offset, y + 13, color);
                set_surface_pixel(pixels, width, height, x + 2, y + offset, color);
                set_surface_pixel(pixels, width, height, x + 13, y + offset, color);
            }
            fill_rect(pixels, width, height, x + 7, y + 5, 2, 5, color);
            fill_rect(pixels, width, height, x + 7, y + 9, 5, 2, color);
            fill_rect(pixels, width, height, x, y, 5, 2, color);
            fill_rect(pixels, width, height, x + 11, y, 5, 2, color);
        }
        16 => {
            // Application grid.
            for row in 0..3 {
                for column in 0..3 {
                    fill_rect(
                        pixels,
                        width,
                        height,
                        x + 1 + column * 5,
                        y + 1 + row * 5,
                        3,
                        3,
                        color,
                    );
                }
            }
        }
        _ => {
            for offset in 0..7 {
                fill_rect(
                    pixels,
                    width,
                    height,
                    x + 7usize.abs_diff(offset),
                    y + offset,
                    offset.saturating_mul(2).saturating_add(1).min(14),
                    1,
                    color,
                );
                fill_rect(
                    pixels,
                    width,
                    height,
                    x + offset,
                    y + 7 + offset,
                    14usize.saturating_sub(offset.saturating_mul(2)),
                    1,
                    color,
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn make_region_opaque(
    pixels: &mut [u32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    region_width: usize,
    region_height: usize,
) {
    let end_y = y.saturating_add(region_height).min(height);
    let end_x = x.saturating_add(region_width).min(width);
    for row in y.min(height)..end_y {
        for pixel in &mut pixels[row * width + x.min(width)..row * width + end_x] {
            *pixel |= 0xff00_0000;
        }
    }
}

pub(super) fn liquid_glass_color(color: u32, palette: &WmThemePalette) -> u32 {
    let rgb = color & 0x00ff_ffff;
    let surface = palette.menu[..2].contains(&rgb)
        || [
            palette.taskbar[0],
            palette.taskbar[2],
            palette.taskbar[3],
            palette.taskbar[4],
        ]
        .contains(&rgb)
        || [palette.frame[1], palette.frame[3], palette.frame[5]].contains(&rgb);
    let raised = palette.menu[2..5].contains(&rgb)
        || [
            palette.taskbar[1],
            palette.taskbar[6],
            palette.frame[0],
            palette.frame[2],
        ]
        .contains(&rgb);
    let alpha = if surface {
        WM_FROSTED_ICE_SURFACE_ALPHA
    } else if raised {
        WM_FROSTED_ICE_RAISED_ALPHA
    } else {
        u8::MAX
    };
    u32::from(alpha) << 24 | rgb
}

pub(super) fn apply_liquid_glass_pixel(pixel: &mut u32, palette: &WmThemePalette) {
    if *pixel != 0 && *pixel & 0xff00_0000 == 0 {
        *pixel = liquid_glass_color(*pixel, palette);
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_menu_triangle(
    pixels: &mut [u32],
    width: usize,
    height: usize,
    center_x: usize,
    center_y: usize,
    upward: bool,
    rightward: bool,
    color: u32,
) {
    if rightward {
        for offset in 0..7usize {
            let half_height = (6usize.saturating_sub(offset)) / 2;
            fill_rect(
                pixels,
                width,
                height,
                center_x.saturating_sub(3).saturating_add(offset),
                center_y.saturating_sub(half_height),
                1,
                half_height.saturating_mul(2).saturating_add(1),
                color,
            );
        }
        return;
    }
    for offset in 0..7usize {
        let half_width = if upward {
            offset / 2
        } else {
            (6usize.saturating_sub(offset)) / 2
        };
        fill_rect(
            pixels,
            width,
            height,
            center_x.saturating_sub(half_width),
            center_y.saturating_sub(3).saturating_add(offset),
            half_width.saturating_mul(2).saturating_add(1),
            1,
            color,
        );
    }
}

pub(super) fn draw_argb_icon(
    pixels: &mut [u32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    icon: &[u32],
) {
    if icon.len() < 16 * 16 {
        return;
    }
    for icon_y in 0..16usize {
        for icon_x in 0..16usize {
            let source = icon[icon_y * 16 + icon_x];
            let alpha = (source >> 24) & 0xff;
            if alpha == 0 {
                continue;
            }
            let target_x = x + icon_x;
            let target_y = y + icon_y;
            if target_x >= width || target_y >= height {
                continue;
            }
            let source_rgb = source & 0x00ff_ffff;
            let target = &mut pixels[target_y * width + target_x];
            if alpha == 0xff {
                *target = source_rgb;
                continue;
            }
            let inverse = 0xff - alpha;
            let blend_channel = |shift: u32| {
                (((source_rgb >> shift) & 0xff) * alpha + ((*target >> shift) & 0xff) * inverse)
                    / 0xff
            };
            *target = (blend_channel(16) << 16) | (blend_channel(8) << 8) | blend_channel(0);
        }
    }
}
