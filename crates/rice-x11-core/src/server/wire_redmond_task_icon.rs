//! Protocol encoding and raster helpers.
use super::*;



pub(super) fn redmond_task_icon(kind: u8) -> Option<&'static [u32]> {
    match kind {
        1 | 12 => Some(&redmond_task_icons::DESKTOP),
        11 => Some(&redmond_task_icons::FOLDER_OPEN),
        2 => Some(&redmond_task_icons::MSDOS),
        3 => Some(&redmond_task_icons::PAINT),
        7 => Some(&redmond_task_icons::NOTEPAD),
        9 => Some(&redmond_task_icons::MEDIA_PLAYER),
        10 | 114 => Some(&redmond_task_icons::TASK),
        113 => Some(&redmond_task_icons::WINDOWS),
        115 | 122 | 132 => Some(&redmond_task_icons::MAIL),
        116 => Some(&redmond_task_icons::CALCULATOR),
        117 => Some(&redmond_task_icons::SETTINGS),
        121 => Some(&redmond_task_icons::INTERNET_EXPLORER),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_bitmap_text(
    pixels: &mut [u32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    text: &[u8],
    color: u32,
    scale: usize,
    max_characters: usize,
) {
    let scale = scale.max(1);
    let advance = 6 * scale;
    for (index, byte) in text.iter().copied().take(max_characters).enumerate() {
        let glyph = wm_glyph(byte);
        let glyph_x = x.saturating_add(index.saturating_mul(advance));
        for (row, bits) in glyph.into_iter().enumerate() {
            for column in 0..5 {
                if bits & (1 << (4 - column)) == 0 {
                    continue;
                }
                fill_rect(
                    pixels,
                    width,
                    height,
                    glyph_x + column * scale,
                    y + row * scale,
                    scale,
                    scale,
                    color,
                );
            }
        }
    }
}

pub(super) fn draw_connectivity_icon(
    pixels: &mut [u32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    mode: u8,
    foreground: u32,
) {
    let strategy_color = match mode {
        0 => 0x00ef_4444,
        1 => 0x0038_bdf8,
        2 => 0x0022_c55e,
        3 => 0x00a7_78e8,
        _ => foreground,
    };
    // Shared globe outline. The inner mark distinguishes offline, hosted,
    // local-gateway, and remote-proxy strategies at taskbar scale.
    fill_rect(pixels, width, height, x + 4, y, 12, 2, strategy_color);
    fill_rect(pixels, width, height, x + 1, y + 4, 18, 2, strategy_color);
    fill_rect(pixels, width, height, x, y + 8, 20, 2, strategy_color);
    fill_rect(pixels, width, height, x + 1, y + 13, 18, 2, strategy_color);
    fill_rect(pixels, width, height, x + 4, y + 17, 12, 2, strategy_color);
    fill_rect(pixels, width, height, x + 4, y + 2, 2, 15, strategy_color);
    fill_rect(pixels, width, height, x + 14, y + 2, 2, 15, strategy_color);
    match mode {
        0 => {
            for offset in 0..16 {
                fill_rect(
                    pixels,
                    width,
                    height,
                    x + 2 + offset,
                    y + 1 + offset,
                    2,
                    2,
                    0x00ef_4444,
                );
            }
        }
        1 => {
            fill_rect(pixels, width, height, x + 8, y + 3, 4, 13, foreground);
        }
        2 => {
            fill_rect(pixels, width, height, x + 7, y + 7, 6, 6, foreground);
            fill_rect(pixels, width, height, x + 9, y + 4, 2, 11, foreground);
        }
        3 => {
            fill_rect(pixels, width, height, x + 7, y + 5, 6, 3, foreground);
            fill_rect(pixels, width, height, x + 7, y + 11, 6, 3, foreground);
            fill_rect(pixels, width, height, x + 9, y + 8, 2, 3, foreground);
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_rice_box_indicator(
    pixels: &mut [u32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    unread: u16,
    foreground: u32,
    panel_background: u32,
) {
    let background = if unread > 0 { 0x00e8_4716 } else { 0x0044_4444 };
    fill_rect(
        pixels,
        width,
        height,
        x,
        y,
        WM_RICE_BOX_INDICATOR_WIDTH,
        25,
        panel_background,
    );
    fill_rounded_rect(
        pixels,
        width,
        height,
        x,
        y,
        WM_RICE_BOX_INDICATOR_WIDTH,
        25,
        12,
        background,
    );

    // A compact envelope stays recognizable at panel scale without depending
    // on a decoded icon asset. The count is retained with the panel pixels.
    fill_rect(pixels, width, height, x + 7, y + 7, 14, 2, foreground);
    fill_rect(pixels, width, height, x + 7, y + 9, 2, 9, foreground);
    fill_rect(pixels, width, height, x + 19, y + 9, 2, 9, foreground);
    fill_rect(pixels, width, height, x + 7, y + 17, 14, 2, foreground);
    for offset in 0..5 {
        set_surface_pixel(pixels, width, height, x + 9 + offset, y + 10 + offset, foreground,
        );
        set_surface_pixel(pixels, width, height, x + 18 - offset, y + 10 + offset, foreground,
        );
    }
    let label = if unread > 99 {
        b"99+".to_vec()
    } else {
        unread.to_string().into_bytes()
    };
    let label_width = label.len().saturating_mul(6).saturating_sub(1);
    draw_bitmap_text(
        pixels,
        width,
        height,
        x + 34usize.saturating_sub(label_width / 2),
        y + 9,
        &label,
        foreground,
        1,
        3,
    );
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_system_controls_cluster(
    pixels: &mut [u32], width: usize, height: usize, x: usize, y: usize,
    _state: u8, battery_percentage: Option<u8>, foreground: u32, _muted: u32,
    surface_background: u32, background: u32,
) {
    let cluster_width = if battery_percentage.is_some() {
        WM_SYSTEM_CONTROLS_BATTERY_WIDTH
    } else {
        WM_SYSTEM_CONTROLS_WIDTH
    };
    // Clear the complete reserved rectangle first. An extension indicator
    // from an earlier panel snapshot must never leak through the transparent
    // corner pixels of this rounded, desktop-owned control surface.
    fill_rect(pixels, width, height, x, y, cluster_width, 24, surface_background,
    );
    fill_rounded_rect(
        pixels,
        width,
        height,
        x,
        y,
        cluster_width,
        24,
        12,
        background,
    );
    // Semantic system icons are deliberately overlaid by the canvas icon
    // renderer. That keeps the top bar on the same active-theme catalog as
    // the native quick-controls menu instead of baking a second icon set into
    // the compositor surface.
    if let Some(percentage) = battery_percentage {
        let battery_x = x + 70;
        fill_rect(pixels, width, height, battery_x, y + 7, 14, 10, foreground);
        fill_rect(pixels, width, height, battery_x + 14, y + 10, 2, 4, foreground,
        );
        fill_rect(pixels, width, height, battery_x + 2, y + 9, 10, 6, background,
        );
        let text = format!("{percentage}%");
        draw_bitmap_text(
            pixels,
            width,
            height,
            x + 88,
            y + 9,
            text.as_bytes(),
            foreground,
            1,
            4,
        );
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn draw_app_icon_sized(
    pixels: &mut [u32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    size: usize,
    kind: u8,
    foreground: u32,
) {
    const SOURCE_WIDTH: usize = 20;
    const SOURCE_HEIGHT: usize = 18;
    let mut source = [0u32; SOURCE_WIDTH * SOURCE_HEIGHT];
    draw_app_icon(
        &mut source,
        SOURCE_WIDTH,
        SOURCE_HEIGHT,
        0,
        0,
        kind,
        foreground,
    );
    for target_y in 0..size {
        let source_y = target_y.saturating_mul(SOURCE_HEIGHT) / size.max(1);
        for target_x in 0..size {
            let source_x = target_x.saturating_mul(SOURCE_WIDTH) / size.max(1);
            let color = source[source_y * SOURCE_WIDTH + source_x];
            if color != 0 {
                fill_rect(pixels, width, height, x + target_x, y + target_y, 1, 1, color,
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn draw_app_icon(
    pixels: &mut [u32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    kind: u8,
    foreground: u32,
) {
    match kind {
        255 => {}// Browser-owned named icon; leave the control background intact.
        1 => {
            fill_rect(pixels, width, height, x, y + 3, 19, 14, 0x00f5_c451);
            fill_rect(pixels, width, height, x + 2, y, 8, 5, 0x00fd_d66d);
            fill_rect(pixels, width, height, x + 2, y + 6, 15, 2, 0x00ff_e29a);
        }
        2 => {
            fill_rect(pixels, width, height, x, y, 20, 18, 0x0008_1018);
            fill_rect(pixels, width, height, x, y, 20, 2, 0x0064_748b);
            draw_bitmap_text(
                pixels,
                width,
                height,
                x + 3,
                y + 6,
                b">_",
                0x004a_de80,
                1,
                2,
            );
        }
        3 => {
            fill_rect(pixels, width, height, x, y, 20, 18, 0x00c4_b5fd);
            for offset in 0..7 {
                fill_rect(
                    pixels,
                    width,
                    height,
                    x + 2 + offset,
                    y + 14 - offset,
                    2,
                    offset + 1,
                    0x0022_c55e,
                );
                fill_rect(
                    pixels,
                    width,
                    height,
                    x + 10 + offset,
                    y + 9 + offset,
                    2,
                    8 - offset,
                    0x0016_a34a,
                );
            }
            fill_rect(pixels, width, height, x + 14, y + 3, 3, 3, 0x00fd_e047);
        }
        4 => {
            fill_rect(pixels, width, height, x + 2, y + 1, 14, 11, 0x0038_bdf8);
            fill_rect(pixels, width, height, x + 6, y + 6, 14, 11, 0x000e_7490);
            fill_rect(pixels, width, height, x + 8, y + 8, 10, 7, 0x0018_2333);
        }
        5 | 6 => {
            let reverse = kind == 6;
            for row in 0usize..13 {
                let spread = row.abs_diff(6);
                let length = 7usize.saturating_sub(spread);
                let start = if reverse {
                    x + 2
                } else {
                    x + 16usize.saturating_sub(length)
                };
                fill_rect(
                    pixels,
                    width,
                    height,
                    start,
                    y + 2 + row,
                    length,
                    1,
                    0x0060_a5fa,
                );
            }
        }
        7 => {
            fill_rect(pixels, width, height, x + 2, y, 15, 18, 0x00f8_fafc);
            fill_rect(pixels, width, height, x + 5, y + 4, 9, 1, 0x0094_a3b8);
            fill_rect(pixels, width, height, x + 5, y + 8, 9, 1, 0x0094_a3b8);
            fill_rect(pixels, width, height, x + 5, y + 12, 6, 1, 0x0094_a3b8);
            for offset in 0..9 {
                fill_rect(
                    pixels,
                    width,
                    height,
                    x + 9 + offset,
                    y + 16usize.saturating_sub(offset),
                    3,
                    2,
                    0x00f5_c451,
                );
            }
        }
        8 => {
            for row in 0usize..18 {
                let distance = row.abs_diff(8);
                let half_width = 8usize.saturating_sub(distance / 2);
                fill_rect(
                    pixels,
                    width,
                    height,
                    x + 9usize.saturating_sub(half_width),
                    y + row,
                    half_width.saturating_mul(2),
                    1,
                    0x0038_bdf8,
                );
            }
            fill_rect(pixels, width, height, x + 2, y + 8, 15, 1, foreground);
            fill_rect(pixels, width, height, x + 9, y + 2, 1, 14, foreground);
        }
        9 => {
            // Compact film frame with a high-contrast play glyph.
            fill_rect(pixels, width, height, x, y + 1, 20, 16, 0x001e_293b);
            for hole_x in [2usize, 7, 12, 17] {
                fill_rect(pixels, width, height, x + hole_x, y + 3, 2, 2, 0x0094_a3b8);
                fill_rect(pixels, width, height, x + hole_x, y + 13, 2, 2, 0x0094_a3b8);
            }
            for row in 0usize..9 {
                let length = row.min(8usize.saturating_sub(row)) + 1;
                fill_rect(
                    pixels,
                    width,
                    height,
                    x + 8,
                    y + 5 + row / 2,
                    length,
                    1,
                    0x0038_bdf8,
                );
            }
        }
        10 => {
            // A compact power glyph for the explicit session-exit action.
            let color = 0x00ef_4444;
            fill_rect(pixels, width, height, x + 8, y, 4, 10, color);
            for row in 0usize..14 {
                let inset = row.abs_diff(7) / 3;
                if row < 3 || row > 11 {
                    fill_rect(
                        pixels,
                        width,
                        height,
                        x + 3 + inset,
                        y + 4 + row,
                        2,
                        1,
                        color,
                    );
                    fill_rect(
                        pixels,
                        width,
                        height,
                        x + 15usize.saturating_sub(inset),
                        y + 4 + row,
                        2,
                        1,
                        color,
                    );
                } else {
                    fill_rect(pixels, width, height, x + 2, y + 4 + row, 2, 1, color);
                    fill_rect(pixels, width, height, x + 16, y + 4 + row, 2, 1, color);
                }
            }
            fill_rect(pixels, width, height, x + 5, y + 17, 10, 2, color);
        }
        11 => {
            // File manager: a window with a sidebar and file rows, distinct
            // from the yellow folder used for ordinary directories.
            fill_rect(pixels, width, height, x, y + 1, 20, 17, 0x0038_bdf8);
            fill_rect(pixels, width, height, x + 2, y + 4, 5, 12, 0x000e_7490);
            fill_rect(pixels, width, height, x + 9, y + 5, 8, 2, 0x00e0_f2fe);
            fill_rect(pixels, width, height, x + 9, y + 9, 8, 2, 0x00e0_f2fe);
            fill_rect(pixels, width, height, x + 9, y + 13, 6, 2, 0x00e0_f2fe);
        }
        12 => {
            fill_rect(pixels, width, height, x + 1, y + 1, 18, 13, 0x0038_bdf8);
            fill_rect(pixels, width, height, x + 3, y + 3, 14, 9, 0x000f_172a);
            fill_rect(pixels, width, height, x + 8, y + 14, 4, 2, foreground);
            fill_rect(pixels, width, height, x + 5, y + 16, 10, 2, foreground);
        }
        13 => {
            for offset in [0usize, 6, 12] {
                fill_rect(
                    pixels,
                    width,
                    height,
                    x + 1,
                    y + 2 + offset,
                    18,
                    2,
                    foreground,
                );
                fill_rect(
                    pixels,
                    width,
                    height,
                    x + 5 + (offset / 6) * 4,
                    y + offset,
                    4,
                    6,
                    0x0038_bdf8,
                );
            }
        }
        14 => {
            fill_rect(pixels, width, height, x, y + 1, 20, 17, 0x001e_293b);
            for (offset, bar) in [4usize, 10, 7, 14, 9].into_iter().enumerate() {
                fill_rect(
                    pixels,
                    width,
                    height,
                    x + 2 + offset * 3,
                    y + 16usize.saturating_sub(bar),
                    2,
                    bar,
                    if offset % 2 == 0 {
                        0x0022_c55e
                    } else {
                        0x0038_bdf8
                    },
                );
            }
        }
        _ => {
            fill_rect(pixels, width, height, x, y, 18, 18, 0x0006_473d);
            fill_rect(pixels, width, height, x + 3, y + 3, 12, 12, foreground);
            fill_rect(pixels, width, height, x + 6, y + 6, 6, 6, 0x0006_473d);
        }
    }
}
