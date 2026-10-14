//! Bounded binary adapter for one `ricewm-theme-catalog-v1` record.

use rice_ui::{
    ButtonShape, CatalogTheme, ChromeProfile, Color, FRAME_COLOR_COUNT, MENU_COLOR_COUNT,
    TASKBAR_COLOR_COUNT, Theme,
};

pub const THEME_RECORD_BYTES: usize = 96;
pub const EXTENDED_THEME_RECORD_BYTES: usize = 126;
const MAGIC: &[u8; 4] = b"RTUI";
const VERSION: u16 = 1;
const SIGNAL_RADIUS_MARKER: u16 = 1 << 14;
const REDMOND98_RADIUS_MARKER: u16 = 1 << 15;
const FLAG_LIQUID_GLASS: u16 = 1;
const FLAG_FROSTED_BUTTONS: u16 = 1 << 1;
const FLAG_FROSTED_SWITCHES: u16 = 1 << 2;
const FLAG_FROSTED_SLIDERS: u16 = 1 << 3;
const FLAG_FROSTED_SCROLLBARS: u16 = 1 << 4;
const FLAG_FAMILIAR: u16 = 1 << 5;
const FLAG_YELLOW_JACKET: u16 = 1 << 6;
const FLAG_COMMAND_DECK: u16 = 1 << 7;
const FLAG_ORCHARD: u16 = 1 << 8;
const FLAG_NEON_GRID: u16 = 1 << 9;
const FLAG_BUTTON_WEDGED: u16 = 1 << 10;
const FLAG_BUTTON_PILL: u16 = 1 << 11;
const FLAG_BUTTON_CHAMFERED: u16 = 1 << 12;
const FLAG_BUTTON_DEFAULT: u16 = 1 << 13;
const FLAG_GLASS_SIDEBARS: u16 = 1 << 14;
const FLAG_GLASS_MENUS: u16 = 1 << 15;
const VALID_FLAGS: u16 = FLAG_LIQUID_GLASS
    | FLAG_FROSTED_BUTTONS
    | FLAG_FROSTED_SWITCHES
    | FLAG_FROSTED_SLIDERS
    | FLAG_FROSTED_SCROLLBARS
    | FLAG_FAMILIAR
    | FLAG_YELLOW_JACKET
    | FLAG_COMMAND_DECK
    | FLAG_ORCHARD
    | FLAG_NEON_GRID
    | FLAG_BUTTON_WEDGED
    | FLAG_BUTTON_PILL
    | FLAG_BUTTON_CHAMFERED
    | FLAG_BUTTON_DEFAULT
    | FLAG_GLASS_SIDEBARS
    | FLAG_GLASS_MENUS;
const FROSTED_ICE_SURFACE_ALPHA: u8 = 190;
const FROSTED_ICE_RAISED_ALPHA: u8 = 150;

pub fn decode_theme(bytes: &[u8]) -> Option<Theme> {
    if ![THEME_RECORD_BYTES, EXTENDED_THEME_RECORD_BYTES].contains(&bytes.len())
        || &bytes[..4] != MAGIC
        || u16::from_le_bytes(bytes[4..6].try_into().ok()?) != if bytes.len() == THEME_RECORD_BYTES { VERSION } else { 2 }
        || usize::from(u16::from_le_bytes(bytes[6..8].try_into().ok()?)) != bytes.len()
    {
        return None;
    }
    let encoded_radius = u16::from_le_bytes(bytes[8..10].try_into().ok()?);
    let redmond98 = encoded_radius & REDMOND98_RADIUS_MARKER != 0;
    let radius = encoded_radius & !(REDMOND98_RADIUS_MARKER | SIGNAL_RADIUS_MARKER);
    let flags = u16::from_le_bytes(bytes[10..12].try_into().ok()?);
    if flags & !VALID_FLAGS != 0 {
        return None;
    }
    let mut offset = 12;
    let taskbar = decode_group::<TASKBAR_COLOR_COUNT>(bytes, &mut offset)?;
    let menu = decode_group::<MENU_COLOR_COUNT>(bytes, &mut offset)?;
    let frame = decode_group::<FRAME_COLOR_COUNT>(bytes, &mut offset)?;
    if offset != THEME_RECORD_BYTES {
        return None;
    }
    let mut theme = CatalogTheme::new(taskbar, menu, frame, radius)
        .ok()
        .map(CatalogTheme::to_theme)?;
    theme.signal_controls = encoded_radius & SIGNAL_RADIUS_MARKER != 0;
    if flags & FLAG_FAMILIAR != 0 {
        let familiar = Theme::familiar_light();
        theme.spacing = familiar.spacing;
        theme.metrics = familiar.metrics;
        theme.typography = familiar.typography;
    }
    let profiles = [
        (FLAG_FAMILIAR, ChromeProfile::Familiar),
        (FLAG_YELLOW_JACKET, ChromeProfile::YellowJacket),
        (FLAG_COMMAND_DECK, ChromeProfile::CommandDeck),
        (FLAG_ORCHARD, ChromeProfile::Orchard),
        (FLAG_NEON_GRID, ChromeProfile::NeonGrid),
    ];
    if profiles
        .iter()
        .filter(|(flag, _)| flags & *flag != 0)
        .count()
        > 1
    {
        return None;
    }
    theme.chrome_profile = profiles
        .into_iter()
        .find_map(|(flag, profile)| (flags & flag != 0).then_some(profile))
        .unwrap_or_default();
    if redmond98 {
        if theme.chrome_profile != ChromeProfile::Standard {
            return None;
        }
        theme.chrome_profile = ChromeProfile::Redmond98;
    }
    let button_shapes = [
        (FLAG_BUTTON_DEFAULT, ButtonShape::Default),
        (FLAG_BUTTON_WEDGED, ButtonShape::Wedged),
        (FLAG_BUTTON_PILL, ButtonShape::Pill),
        (FLAG_BUTTON_CHAMFERED, ButtonShape::Chamfered),
    ];
    if button_shapes
        .iter()
        .filter(|(flag, _)| flags & *flag != 0)
        .count()
        > 1
    {
        return None;
    }
    theme.button_shape = button_shapes
        .into_iter()
        .find_map(|(flag, shape)| (flags & flag != 0).then_some(shape))
        .unwrap_or_else(|| match theme.chrome_profile {
            ChromeProfile::NeonGrid => ButtonShape::Wedged,
            ChromeProfile::CommandDeck => ButtonShape::Pill,
            _ => ButtonShape::Default,
        });
    match theme.chrome_profile {
        ChromeProfile::YellowJacket => {
            let familiar = Theme::familiar_light();
            theme.spacing = familiar.spacing;
            theme.metrics = familiar.metrics;
            theme.typography = familiar.typography;
            theme.metrics.control_height = 26;
            theme.metrics.border_width = 2;
            // BeOS used compact furniture, but its application copy was not
            // the tiny Win32 dialog face inherited by Familiar. Keep the
            // title unchanged while enlarging ordinary and secondary copy.
            theme.typography.body_px = 13;
            theme.typography.small_px = 12;
            // The Yellow Jacket selection blue needs light ink; inheriting
            // the black frame-title color makes selected rows unreadable.
            // BeOS selection blue always carries white copy. Do not derive
            // this from a user-edited swatch: medium blues near a luminance
            // threshold can otherwise regress to black-on-blue.
            theme.palette.selection_text = Color::opaque(255, 255, 255);
            theme.palette.accent_text = contrasting_ink(theme.palette.accent);
        }
        ChromeProfile::CommandDeck => {
            if theme.metrics.corner_radius > 0 {
                theme.metrics.corner_radius = theme.metrics.corner_radius.max(12);
            }
            theme.metrics.control_height = 34;
            theme.spacing.control_x = 12;
            theme.typography.title_px = 15;
            // Command Deck uses bright LCARS surfaces for selection and
            // emphasis. Derive foreground ink from each semantic surface;
            // reusing the pale body text makes yellow controls illegible.
            theme.palette.selection_text = contrasting_ink(theme.palette.selection);
            theme.palette.accent_text = contrasting_ink(theme.palette.accent);
        }
        ChromeProfile::Orchard => {
            if theme.metrics.corner_radius > 0 {
                theme.metrics.corner_radius = theme.metrics.corner_radius.max(14);
            }
            // Match the compact optical density of contemporary AppKit: list
            // rows and toolbar controls are smaller than RiceWM's modern
            // default, while the window silhouette remains generously round.
            theme.metrics.control_height = 30;
            theme.metrics.scrollbar_width = 12;
            theme.spacing.control_x = 10;
            theme.spacing.control_y = 5;
            theme.typography.body_px = 13;
            theme.typography.small_px = 11;
            theme.typography.title_px = 14;
            theme.palette.selection_text = Color::opaque(255, 255, 255);
            theme.palette.accent_text = Color::opaque(255, 255, 255);
        }
        ChromeProfile::NeonGrid => {
            theme.metrics.corner_radius = theme.metrics.corner_radius.min(4);
            theme.metrics.control_height = 28;
            theme.spacing.control_x = 10;
            theme.typography.title_px = 13;
            // Electric cyan is a light control surface. Pale body copy has
            // insufficient contrast on it, so active buttons and selections
            // use derived dark ink rather than inheriting frame text.
            theme.palette.accent_text = contrasting_ink(theme.palette.accent);
            theme.palette.selection_text = contrasting_ink(theme.palette.selection);
        }
        ChromeProfile::Redmond98 => {
            let classic = Theme::familiar_light();
            theme.spacing = classic.spacing;
            theme.metrics = classic.metrics;
            theme.typography = classic.typography;
            theme.metrics.control_height = 23;
            theme.metrics.scrollbar_width = 16;
            theme.palette.desktop = Color::opaque(0, 128, 128);
            theme.palette.surface = Color::opaque(192, 192, 192);
            theme.palette.surface_raised = Color::opaque(255, 255, 255);
            theme.palette.text = Color::opaque(0, 0, 0);
            theme.palette.text_muted = Color::opaque(128, 128, 128);
            theme.palette.accent = Color::opaque(0, 0, 128);
            theme.palette.accent_text = Color::opaque(255, 255, 255);
            theme.palette.border = Color::opaque(128, 128, 128);
            theme.palette.focus = Color::opaque(0, 0, 128);
            theme.palette.selection = Color::opaque(0, 0, 128);
            theme.palette.selection_text = Color::opaque(255, 255, 255);
            theme.menu_palette.surface = Color::opaque(192, 192, 192);
            theme.menu_palette.surface_raised = Color::opaque(192, 192, 192);
            theme.menu_palette.text = Color::opaque(0, 0, 0);
            theme.menu_palette.text_muted = Color::opaque(128, 128, 128);
            theme.menu_palette.border = Color::opaque(0, 0, 0);
            theme.menu_palette.selection = Color::opaque(0, 0, 128);
            theme.menu_palette.selection_text = Color::opaque(255, 255, 255);
        }
        ChromeProfile::Standard | ChromeProfile::Familiar => {}
    }
    if flags & FLAG_LIQUID_GLASS != 0 {
        // Native layouts frequently nest a raised panel over the root
        // surface. The two coats combine to about 90% optical density: enough
        // contrast for dialog text while the compositor blur remains visible.
        let orchard = theme.chrome_profile == ChromeProfile::Orchard;
        theme.palette.surface = theme.palette.surface.with_alpha(if orchard {
            238
        } else {
            FROSTED_ICE_SURFACE_ALPHA
        });
        theme.palette.surface_raised = theme.palette.surface_raised.with_alpha(if orchard {
            224
        } else {
            FROSTED_ICE_RAISED_ALPHA
        });
        theme.palette.border = theme
            .palette
            .border
            .with_alpha(if orchard { 150 } else { 190 });
        theme.palette.focus = theme
            .palette
            .focus
            .with_alpha(if orchard { 235 } else { 205 });
        theme.palette.selection =
            theme
                .palette
                .selection
                .with_alpha(if orchard { 235 } else { 170 });
        theme.palette.accent = theme
            .palette
            .accent
            .with_alpha(if orchard { 235 } else { 205 });
    }
    theme.frosted_widgets.buttons = flags & FLAG_FROSTED_BUTTONS != 0;
    theme.frosted_widgets.switches = flags & FLAG_FROSTED_SWITCHES != 0;
    theme.frosted_widgets.sliders = flags & FLAG_FROSTED_SLIDERS != 0;
    theme.frosted_widgets.scrollbars = flags & FLAG_FROSTED_SCROLLBARS != 0;
    theme.glass_sidebars = flags & FLAG_GLASS_SIDEBARS != 0;
    theme.glass_menus = flags & FLAG_GLASS_MENUS != 0;
    if theme.glass_menus {
        theme.palette.surface = rgb_color(frame[3]);
        theme.palette.surface_raised = rgb_color(frame[5]);
        theme.palette.text = rgb_color(frame[4]);
        theme.palette.text_muted = rgb_color(frame[4]).with_alpha(168);
        theme.palette.border = rgb_color(frame[2]);
        theme.menu_palette.selection_text = contrasting_ink(theme.menu_palette.selection);
    }
    if theme.chrome_profile == ChromeProfile::Orchard {
        // Keep reading surfaces white and let group separators recede. The
        // outer window retains its own stronger silhouette border.
        theme.palette.border = theme.palette.border.with_alpha(96);
    }
    if bytes.len() == EXTENDED_THEME_RECORD_BYTES {
        let mask = u32::from_le_bytes(bytes[96..100].try_into().ok()?);
        if mask & !0x1fff != 0 { return None; }
        let bounds = [(8,32),(8,28),(8,36),(12,72),(100,200),(16,72),(16,72),(4,32),(0,6),(0,6),(0,32),(0,24),(0,32)];
        let mut values = [0u16;13];
        for (i, value) in values.iter_mut().enumerate() {
            *value = u16::from_le_bytes(bytes[100+i*2..102+i*2].try_into().ok()?);
            if mask & (1 << i) != 0 && (*value < bounds[i].0 || *value > bounds[i].1) { return None; }
        }
        let fields = [&mut theme.typography.body_px,&mut theme.typography.small_px,&mut theme.typography.title_px,&mut theme.typography.display_px,&mut theme.typography.line_height_percent,&mut theme.metrics.control_height,&mut theme.metrics.menu_row_height,&mut theme.metrics.scrollbar_width,&mut theme.metrics.focus_width,&mut theme.metrics.border_width,&mut theme.spacing.control_x,&mut theme.spacing.control_y,&mut theme.metrics.corner_radius];
        for (i, field) in fields.into_iter().enumerate() { if mask & (1 << i) != 0 { *field = values[i]; } }
    }
    Some(theme)
}

fn rgb_color(value: u32) -> Color {
    let bytes = value.to_be_bytes();
    Color::opaque(bytes[1], bytes[2], bytes[3])
}

fn contrasting_ink(background: Color) -> Color {
    let [red, green, blue, _] = background.channels();
    let luminance = u32::from(red) * 299 + u32::from(green) * 587 + u32::from(blue) * 114;
    if luminance >= 150_000 {
        Color::opaque(10, 8, 13)
    } else {
        Color::opaque(255, 255, 255)
    }
}

fn decode_group<const N: usize>(bytes: &[u8], offset: &mut usize) -> Option<[u32; N]> {
    let mut colors = [0; N];
    for color in &mut colors {
        let end = offset.checked_add(4)?;
        *color = u32::from_le_bytes(bytes.get(*offset..end)?.try_into().ok()?);
        *offset = end;
    }
    Some(colors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rice_ui::{Color, MODERN_CORNER_RADIUS};

    fn record(radius: u16) -> Vec<u8> {
        let mut bytes = Vec::from(*MAGIC);
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        bytes.extend_from_slice(
            &u16::try_from(THEME_RECORD_BYTES)
                .expect("theme record size fits u16")
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&radius.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        for color in 0..21_u32 {
            bytes.extend_from_slice(&(0x0010_1010 + color).to_le_bytes());
        }
        bytes
    }

    #[test]
    fn decodes_exact_bounded_record() {
        let theme = decode_theme(&record(MODERN_CORNER_RADIUS)).unwrap();
        assert_eq!(theme.palette.surface, Color::opaque(0x10, 0x10, 0x17));
        assert_eq!(
            theme.palette.surface_raised,
            Color::opaque(0x10, 0x10, 0x18)
        );
        assert_eq!(theme.palette.border, Color::opaque(0x10, 0x10, 0x19));
        assert_eq!(theme.metrics.corner_radius, MODERN_CORNER_RADIUS);
    }

    #[test]
    fn decodes_frosted_ice_material_flag() {
        let mut glass = record(MODERN_CORNER_RADIUS);
        let flags = FLAG_LIQUID_GLASS
            | FLAG_FROSTED_BUTTONS
            | FLAG_FROSTED_SWITCHES
            | FLAG_FROSTED_SLIDERS
            | FLAG_FROSTED_SCROLLBARS;
        glass[10..12].copy_from_slice(&flags.to_le_bytes());
        let theme = decode_theme(&glass).unwrap();
        assert_eq!(
            theme.palette.surface.channels()[3],
            FROSTED_ICE_SURFACE_ALPHA
        );
        assert_eq!(
            theme.palette.surface_raised.channels()[3],
            FROSTED_ICE_RAISED_ALPHA
        );
        assert_eq!(theme.palette.text.channels()[3], u8::MAX);
        assert!(theme.frosted_widgets.buttons);
        assert!(theme.frosted_widgets.switches);
        assert!(theme.frosted_widgets.sliders);
        assert!(theme.frosted_widgets.scrollbars);
    }

    #[test]
    fn familiar_flag_applies_win32_control_metrics_without_replacing_catalog_colors() {
        let mut bytes = record(0);
        bytes[10..12].copy_from_slice(&FLAG_FAMILIAR.to_le_bytes());
        let theme = decode_theme(&bytes).expect("familiar theme");
        assert_eq!(theme.metrics.corner_radius, 0);
        assert_eq!(theme.metrics.control_height, 24);
        assert_eq!(theme.metrics.scrollbar_width, 17);
        assert_eq!(theme.metrics.focus_width, 1);
        assert_eq!(theme.spacing.control_x, 4);
        assert_eq!(theme.palette.surface, Color::opaque(0x10, 0x10, 0x17));
    }

    #[test]
    fn redmond_marker_applies_windows_98_system_colors_and_metrics() {
        let theme = decode_theme(&record(REDMOND98_RADIUS_MARKER)).expect("Redmond '98 theme");
        assert_eq!(theme.chrome_profile, ChromeProfile::Redmond98);
        assert_eq!(theme.metrics.corner_radius, 0);
        assert_eq!(theme.metrics.control_height, 23);
        assert_eq!(theme.metrics.scrollbar_width, 16);
        assert_eq!(theme.palette.desktop, Color::opaque(0, 128, 128));
        assert_eq!(theme.palette.surface, Color::opaque(192, 192, 192));
        assert_eq!(theme.palette.selection, Color::opaque(0, 0, 128));
    }

    #[test]
    fn yellow_jacket_enlarges_body_copy_and_contrasts_dark_selection_ink() {
        let mut bytes = record(0);
        bytes[10..12].copy_from_slice(&FLAG_YELLOW_JACKET.to_le_bytes());
        bytes[52..56].copy_from_slice(&0x0017_69aa_u32.to_le_bytes());
        let theme = decode_theme(&bytes).expect("Yellow Jacket theme");
        assert_eq!(theme.typography.body_px, 13);
        assert_eq!(theme.typography.small_px, 12);
        assert_eq!(theme.typography.title_px, 11);
        assert_eq!(theme.palette.selection_text, Color::opaque(255, 255, 255));
        assert_eq!(theme.palette.accent_text, Color::opaque(255, 255, 255));
    }

    #[test]
    fn neon_grid_flag_applies_compact_angular_chrome_metrics() {
        let mut bytes = record(18);
        bytes[10..12].copy_from_slice(&FLAG_NEON_GRID.to_le_bytes());
        let theme = decode_theme(&bytes).expect("Neon Grid theme");
        assert_eq!(theme.chrome_profile, ChromeProfile::NeonGrid);
        assert_eq!(theme.metrics.corner_radius, 4);
        assert_eq!(theme.metrics.control_height, 28);
        assert_eq!(theme.spacing.control_x, 10);
        assert_eq!(theme.typography.title_px, 13);
        assert_eq!(theme.button_shape, ButtonShape::Wedged);
        assert_eq!(
            theme.palette.accent_text,
            contrasting_ink(theme.palette.accent)
        );
    }

    #[test]
    fn explicit_button_shape_overrides_profile_default() {
        let mut bytes = record(6);
        let flags = FLAG_NEON_GRID | FLAG_BUTTON_CHAMFERED;
        bytes[10..12].copy_from_slice(&flags.to_le_bytes());
        let theme = decode_theme(&bytes).expect("custom button shape");
        assert_eq!(theme.chrome_profile, ChromeProfile::NeonGrid);
        assert_eq!(theme.button_shape, ButtonShape::Chamfered);
    }

    #[test]
    fn glass_sidebar_flag_is_independent_from_window_and_widget_frost() {
        let mut bytes = record(12);
        bytes[10..12].copy_from_slice(&(FLAG_GLASS_SIDEBARS | FLAG_GLASS_MENUS).to_le_bytes());
        let theme = decode_theme(&bytes).expect("glass sidebar theme");
        assert!(theme.glass_sidebars);
        assert!(theme.glass_menus);
        assert_eq!(theme.menu_palette.surface, Color::opaque(0x10, 0x10, 0x17));
        assert_eq!(theme.palette.surface, Color::opaque(0x10, 0x10, 0x20));
        assert!(!theme.frosted_widgets.buttons);
        assert_eq!(theme.palette.surface.channels()[3], u8::MAX);
    }

    #[test]
    fn neon_grid_uses_dark_ink_on_electric_control_surfaces() {
        let mut bytes = record(3);
        bytes[10..12].copy_from_slice(&FLAG_NEON_GRID.to_le_bytes());
        bytes[36..40].copy_from_slice(&0x0000_f5ff_u32.to_le_bytes());
        bytes[52..56].copy_from_slice(&0x0000_f5ff_u32.to_le_bytes());
        let theme = decode_theme(&bytes).expect("Neon Grid theme");
        assert_eq!(theme.palette.accent_text, Color::opaque(10, 8, 13));
        assert_eq!(theme.palette.selection_text, Color::opaque(10, 8, 13));
    }

    #[test]
    fn command_deck_keeps_selected_control_text_readable() {
        let mut bytes = record(18);
        bytes[10..12].copy_from_slice(&FLAG_COMMAND_DECK.to_le_bytes());
        bytes[36..40].copy_from_slice(&0x00ff_cc66_u32.to_le_bytes());
        bytes[52..56].copy_from_slice(&0x00ff_cc66_u32.to_le_bytes());
        let theme = decode_theme(&bytes).expect("Command Deck theme");
        assert_eq!(theme.chrome_profile, ChromeProfile::CommandDeck);
        assert_eq!(theme.palette.selection_text, Color::opaque(10, 8, 13));
        assert_eq!(theme.palette.accent_text, Color::opaque(10, 8, 13));
    }

    #[test]
    fn signal_controls_preserve_radius_and_leave_other_themes_unchanged() {
        assert!(!decode_theme(&record(0)).unwrap().signal_controls);
        let theme = decode_theme(&record(SIGNAL_RADIUS_MARKER | 6)).unwrap();
        assert!(theme.signal_controls);
        assert_eq!(theme.metrics.corner_radius, 6);
        assert!(decode_theme(&record(SIGNAL_RADIUS_MARKER | 33)).is_none());
    }

    #[test]
    fn rejects_shape_rgb_and_radius_errors() {
        assert!(decode_theme(&record(33)).is_none());
        let mut invalid_rgb = record(0);
        invalid_rgb[12..16].copy_from_slice(&0x0100_0000_u32.to_le_bytes());
        assert!(decode_theme(&invalid_rgb).is_none());
        assert!(decode_theme(&record(0)[..95]).is_none());
    }
}

#[cfg(test)]
mod customization_tests {
    use super::*;
    fn custom() -> Vec<u8> {
        let mut bytes=Vec::from(*b"RTUI");bytes.extend_from_slice(&2u16.to_le_bytes());bytes.extend_from_slice(&(EXTENDED_THEME_RECORD_BYTES as u16).to_le_bytes());bytes.extend_from_slice(&6u16.to_le_bytes());bytes.extend_from_slice(&FLAG_FAMILIAR.to_le_bytes());
        for _ in 0..21 {bytes.extend_from_slice(&0xf0f0f0u32.to_le_bytes());}
        bytes.extend_from_slice(&0x1fffu32.to_le_bytes());
        for value in [18u16,14,22,40,150,44,48,9,3,0,16,8,10] {bytes.extend_from_slice(&value.to_le_bytes());}bytes
    }
    #[test]
    fn explicit_metrics_override_profile_defaults() {
        let theme=decode_theme(&custom()).unwrap();assert_eq!(theme.chrome_profile,ChromeProfile::Familiar);assert_eq!(theme.metrics.control_height,44);assert_eq!(theme.metrics.menu_row_height,48);assert_eq!(theme.metrics.scrollbar_width,9);assert_eq!(theme.metrics.focus_width,3);assert_eq!(theme.metrics.border_width,0);assert_eq!(theme.spacing.control_x,16);assert_eq!(theme.typography.body_px,18);assert_eq!(theme.typography.title_px,22);
    }
    #[test]
    fn malformed_customization_is_rejected_atomically() {
        let mut bytes=custom();bytes[100..102].copy_from_slice(&99u16.to_le_bytes());assert!(decode_theme(&bytes).is_none());
        let mut bytes=custom();bytes[96..100].copy_from_slice(&u32::MAX.to_le_bytes());assert!(decode_theme(&bytes).is_none());
        let mut bytes=custom();bytes.pop();assert!(decode_theme(&bytes).is_none());
    }
}
