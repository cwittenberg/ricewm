//! Renderer-neutral theme tokens and the bounded `RiceWM` catalog adapter.

use core::fmt;

pub const TASKBAR_COLOR_COUNT: usize = 7;
pub const MENU_COLOR_COUNT: usize = 6;
pub const FRAME_COLOR_COUNT: usize = 8;
pub const MODERN_CORNER_RADIUS: u16 = 6;
pub const MAX_CORNER_RADIUS: u16 = 32;

/// Packed sRGBA (`0xRRGGBBAA`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Color(u32);

impl Color {
    #[must_use]
    pub const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self(u32::from_be_bytes([red, green, blue, alpha]))
    }

    #[must_use]
    pub const fn opaque(red: u8, green: u8, blue: u8) -> Self {
        Self::rgba(red, green, blue, u8::MAX)
    }

    #[must_use]
    pub const fn channels(self) -> [u8; 4] {
        self.0.to_be_bytes()
    }

    #[must_use]
    pub const fn with_alpha(self, alpha: u8) -> Self {
        let [red, green, blue, _] = self.channels();
        Self::rgba(red, green, blue, alpha)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Palette {
    pub desktop: Color,
    pub surface: Color,
    pub surface_raised: Color,
    pub text: Color,
    pub text_muted: Color,
    pub accent: Color,
    pub accent_text: Color,
    pub border: Color,
    pub focus: Color,
    pub selection: Color,
    pub selection_text: Color,
    /// Informational emphasis used for explanatory surfaces and illustrations.
    pub information: Color,
    /// Positive emphasis used for ready, private, and completed states.
    pub success: Color,
    /// Warm emphasis used for discovery and attention without implying danger.
    pub warning: Color,
    pub danger: Color,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MenuPalette {
    pub surface: Color,
    pub surface_raised: Color,
    pub text: Color,
    pub text_muted: Color,
    pub border: Color,
    pub selection: Color,
    pub selection_text: Color,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Spacing {
    pub unit: u16,
    pub control_x: u16,
    pub control_y: u16,
    pub panel: u16,
    pub section: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Metrics {
    pub border_width: u16,
    pub focus_width: u16,
    pub corner_radius: u16,
    pub control_height: u16,
    pub scrollbar_width: u16,
    /// Zero retains the profile control height for menu rows.
    pub menu_row_height: u16,
    pub icon_small: u16,
    pub icon_medium: u16,
    pub icon_large: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Typography {
    pub body_px: u16,
    pub small_px: u16,
    pub title_px: u16,
    pub display_px: u16,
    pub line_height_percent: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrostedWidgets {
    pub buttons: bool,
    pub switches: bool,
    pub sliders: bool,
    pub scrollbars: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ChromeProfile {
    #[default]
    Standard,
    Familiar,
    Redmond98,
    YellowJacket,
    CommandDeck,
    Orchard,
    NeonGrid,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ButtonShape {
    #[default]
    Default,
    Wedged,
    Pill,
    Chamfered,
}

impl FrostedWidgets {
    #[must_use]
    pub const fn none() -> Self {
        Self {
            buttons: false,
            switches: false,
            sliders: false,
            scrollbars: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Theme {
    pub palette: Palette,
    /// Popup-specific colors, kept separate so translucent launchers can sit
    /// over otherwise light application surfaces.
    pub menu_palette: MenuPalette,
    pub spacing: Spacing,
    pub metrics: Metrics,
    pub typography: Typography,
    /// Selects which interactive controls use backdrop-sampled glass.
    pub frosted_widgets: FrostedWidgets,
    pub chrome_profile: ChromeProfile,
    /// Geometry used by ordinary command buttons in native applications.
    pub button_shape: ButtonShape,
    /// Fine registration brackets and instrument-style slider controls.
    pub signal_controls: bool,
    /// Paint the leading list in a split view as backdrop-sampled glass.
    pub glass_sidebars: bool,
    /// Paint popup menus as backdrop-sampled glass.
    pub glass_menus: bool,
}

impl Theme {
    /// Compact, modern and immediately recognizable `RiceWM` defaults.
    #[must_use]
    pub const fn ricewm_default() -> Self {
        Self {
            palette: Palette {
                desktop: Color::opaque(43, 49, 61),
                surface: Color::opaque(246, 247, 249),
                surface_raised: Color::opaque(255, 255, 255),
                text: Color::opaque(31, 34, 39),
                text_muted: Color::opaque(96, 102, 112),
                accent: Color::opaque(53, 105, 210),
                accent_text: Color::opaque(255, 255, 255),
                border: Color::opaque(194, 199, 207),
                focus: Color::opaque(36, 96, 200),
                selection: Color::opaque(53, 105, 210),
                selection_text: Color::opaque(255, 255, 255),
                information: Color::opaque(35, 148, 190),
                success: Color::opaque(40, 154, 105),
                warning: Color::opaque(222, 142, 38),
                danger: Color::opaque(190, 52, 58),
            },
            menu_palette: MenuPalette {
                surface: Color::opaque(246, 247, 249),
                surface_raised: Color::opaque(255, 255, 255),
                text: Color::opaque(31, 34, 39),
                text_muted: Color::opaque(96, 102, 112),
                border: Color::opaque(194, 199, 207),
                selection: Color::opaque(53, 105, 210),
                selection_text: Color::opaque(255, 255, 255),
            },
            spacing: Spacing {
                unit: 4,
                control_x: 10,
                control_y: 6,
                panel: 12,
                section: 16,
            },
            metrics: Metrics {
                border_width: 1,
                focus_width: 2,
                corner_radius: MODERN_CORNER_RADIUS,
                control_height: 32,
                menu_row_height: 0,
                scrollbar_width: 16,
                icon_small: 16,
                icon_medium: 24,
                icon_large: 48,
            },
            typography: Typography {
                body_px: 13,
                small_px: 11,
                title_px: 14,
                display_px: 26,
                line_height_percent: 125,
            },
            frosted_widgets: FrostedWidgets::none(),
            chrome_profile: ChromeProfile::Standard,
            button_shape: ButtonShape::Default,
            signal_controls: false,
            glass_sidebars: false,
            glass_menus: false,
        }
    }

    /// Dark companion to [`Self::ricewm_default`] for window-scoped native
    /// application preferences. Every semantic role remains explicit so
    /// dialogs, selection, focus and status surfaces change together.
    #[must_use]
    pub const fn ricewm_dark() -> Self {
        Self {
            palette: Palette {
                desktop: Color::opaque(18, 20, 25),
                surface: Color::opaque(31, 34, 42),
                surface_raised: Color::opaque(43, 47, 57),
                text: Color::opaque(244, 241, 235),
                text_muted: Color::opaque(174, 177, 187),
                accent: Color::opaque(235, 82, 35),
                accent_text: Color::opaque(255, 255, 255),
                border: Color::opaque(86, 90, 103),
                focus: Color::opaque(255, 111, 63),
                selection: Color::opaque(179, 66, 31),
                selection_text: Color::opaque(255, 255, 255),
                information: Color::opaque(75, 166, 202),
                success: Color::opaque(72, 177, 121),
                warning: Color::opaque(231, 159, 65),
                danger: Color::opaque(222, 83, 91),
            },
            menu_palette: MenuPalette {
                surface: Color::opaque(31, 34, 42),
                surface_raised: Color::opaque(43, 47, 57),
                text: Color::opaque(244, 241, 235),
                text_muted: Color::opaque(174, 177, 187),
                border: Color::opaque(86, 90, 103),
                selection: Color::opaque(179, 66, 31),
                selection_text: Color::opaque(255, 255, 255),
            },
            spacing: Self::ricewm_default().spacing,
            metrics: Self::ricewm_default().metrics,
            typography: Self::ricewm_default().typography,
            frosted_widgets: FrostedWidgets::none(),
            chrome_profile: ChromeProfile::Standard,
            button_shape: ButtonShape::Default,
            signal_controls: false,
            glass_sidebars: false,
            glass_menus: false,
        }
    }

    /// Win32-familiar light presentation used by the bundled `Familiar`
    /// theme and by the UI Library's isolated theme preview. The semantic
    /// colors and compact metrics intentionally match the direct USER32
    /// control renderer instead of approximating them with modern RiceWM
    /// controls.
    #[must_use]
    pub const fn familiar_light() -> Self {
        Self {
            palette: Palette {
                desktop: Color::opaque(0, 120, 215),
                surface: Color::opaque(240, 240, 240),
                surface_raised: Color::opaque(255, 255, 255),
                text: Color::opaque(0, 0, 0),
                text_muted: Color::opaque(109, 109, 109),
                accent: Color::opaque(0, 120, 215),
                accent_text: Color::opaque(255, 255, 255),
                border: Color::opaque(118, 118, 118),
                focus: Color::opaque(0, 95, 184),
                selection: Color::opaque(0, 120, 215),
                selection_text: Color::opaque(255, 255, 255),
                information: Color::opaque(0, 95, 184),
                success: Color::opaque(16, 124, 16),
                warning: Color::opaque(157, 93, 0),
                danger: Color::opaque(196, 43, 28),
            },
            menu_palette: MenuPalette {
                surface: Color::opaque(240, 240, 240),
                surface_raised: Color::opaque(255, 255, 255),
                text: Color::opaque(0, 0, 0),
                text_muted: Color::opaque(109, 109, 109),
                border: Color::opaque(160, 160, 160),
                selection: Color::opaque(0, 120, 215),
                selection_text: Color::opaque(255, 255, 255),
            },
            spacing: Spacing {
                unit: 4,
                control_x: 4,
                control_y: 2,
                panel: 8,
                section: 12,
            },
            metrics: Metrics {
                border_width: 1,
                focus_width: 1,
                corner_radius: 0,
                control_height: 24,
                menu_row_height: 0,
                scrollbar_width: 17,
                icon_small: 16,
                icon_medium: 24,
                icon_large: 32,
            },
            typography: Typography {
                // 8 pt at 96 DPI is 10.67 px. Eleven pixels keeps the native
                // layout on the same integer baseline as MS Shell Dlg.
                body_px: 11,
                small_px: 11,
                title_px: 11,
                display_px: 18,
                line_height_percent: 118,
            },
            frosted_widgets: FrostedWidgets::none(),
            chrome_profile: ChromeProfile::Familiar,
            button_shape: ButtonShape::Default,
            signal_controls: false,
            glass_sidebars: false,
            glass_menus: false,
        }
    }

    /// Dark companion for the preview matrix. Geometry and control metrics
    /// stay Win32-familiar while semantic colors remain readable in dark mode.
    #[must_use]
    pub const fn familiar_dark() -> Self {
        Self {
            palette: Palette {
                desktop: Color::opaque(0, 0, 0),
                surface: Color::opaque(32, 32, 32),
                surface_raised: Color::opaque(45, 45, 48),
                text: Color::opaque(255, 255, 255),
                text_muted: Color::opaque(168, 168, 168),
                accent: Color::opaque(0, 120, 215),
                accent_text: Color::opaque(255, 255, 255),
                border: Color::opaque(118, 118, 118),
                focus: Color::opaque(0, 153, 255),
                selection: Color::opaque(0, 95, 184),
                selection_text: Color::opaque(255, 255, 255),
                information: Color::opaque(75, 166, 202),
                success: Color::opaque(90, 180, 103),
                warning: Color::opaque(241, 184, 77),
                danger: Color::opaque(255, 99, 99),
            },
            menu_palette: MenuPalette {
                surface: Color::opaque(32, 32, 32),
                surface_raised: Color::opaque(45, 45, 48),
                text: Color::opaque(255, 255, 255),
                text_muted: Color::opaque(168, 168, 168),
                border: Color::opaque(118, 118, 118),
                selection: Color::opaque(0, 95, 184),
                selection_text: Color::opaque(255, 255, 255),
            },
            spacing: Self::familiar_light().spacing,
            metrics: Self::familiar_light().metrics,
            typography: Self::familiar_light().typography,
            frosted_widgets: FrostedWidgets::none(),
            chrome_profile: ChromeProfile::Familiar,
            button_shape: ButtonShape::Default,
            signal_controls: false,
            glass_sidebars: false,
            glass_menus: false,
        }
    }
}

/// Validated RGB groups from `ricewm-theme-catalog-v1`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CatalogTheme {
    pub taskbar: [u32; TASKBAR_COLOR_COUNT],
    pub menu: [u32; MENU_COLOR_COUNT],
    pub frame: [u32; FRAME_COLOR_COUNT],
    pub corner_radius: u16,
}

impl CatalogTheme {
    /// Validates catalog RGB values and the bounded corner metric.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogThemeError`] for non-RGB values or excessive radius.
    pub fn new(
        taskbar: [u32; TASKBAR_COLOR_COUNT],
        menu: [u32; MENU_COLOR_COUNT],
        frame: [u32; FRAME_COLOR_COUNT],
        corner_radius: u16,
    ) -> Result<Self, CatalogThemeError> {
        if taskbar
            .iter()
            .chain(menu.iter())
            .chain(frame.iter())
            .any(|color| *color > 0x00ff_ffff)
        {
            return Err(CatalogThemeError::InvalidRgb);
        }
        if corner_radius > MAX_CORNER_RADIUS {
            return Err(CatalogThemeError::CornerRadiusExceeded);
        }
        Ok(Self {
            taskbar,
            menu,
            frame,
            corner_radius,
        })
    }

    /// Maps IceWM-facing groups to renderer-neutral native UI tokens.
    #[must_use]
    pub fn to_theme(self) -> Theme {
        let mut theme = Theme::ricewm_default();
        theme.palette.desktop = rgb(self.frame[3]);
        theme.palette.surface = rgb(self.menu[0]);
        theme.palette.surface_raised = rgb(self.menu[1]);
        theme.palette.text = rgb(self.menu[5]);
        theme.palette.text_muted = blend_rgb(self.menu[5], self.menu[0], 55);
        theme.palette.accent = rgb(self.taskbar[6]);
        theme.palette.accent_text = rgb(self.frame[4]);
        theme.palette.border = rgb(self.menu[2]);
        theme.palette.focus = rgb(self.frame[0]);
        theme.palette.selection = rgb(self.menu[3]);
        theme.palette.selection_text = rgb(self.frame[4]);
        theme.palette.danger = rgb(self.frame[6]);
        theme.menu_palette = MenuPalette {
            surface: rgb(self.menu[0]),
            surface_raised: rgb(self.menu[1]),
            text: rgb(self.menu[5]),
            text_muted: blend_rgb(self.menu[5], self.menu[0], 55),
            border: rgb(self.menu[2]),
            selection: rgb(self.menu[3]),
            selection_text: rgb(self.frame[4]),
        };
        theme.metrics.corner_radius = self.corner_radius;
        theme
    }
}

const fn rgb(value: u32) -> Color {
    let bytes = value.to_be_bytes();
    Color::opaque(bytes[1], bytes[2], bytes[3])
}

fn blend_rgb(foreground: u32, background: u32, foreground_percent: u32) -> Color {
    let foreground = foreground.to_be_bytes();
    let background = background.to_be_bytes();
    let blend = |index: usize| {
        u8::try_from(
            (u32::from(foreground[index]) * foreground_percent
                + u32::from(background[index]) * (100 - foreground_percent))
                / 100,
        )
        .unwrap_or(u8::MAX)
    };
    Color::opaque(blend(1), blend(2), blend(3))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogThemeError {
    InvalidRgb,
    CornerRadiusExceeded,
}

impl fmt::Display for CatalogThemeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CatalogThemeError {}

impl Theme {
    /// Distinct formula-reference inks chosen for the current native surface.
    /// This is host presentation state, not document formatting or app CSS.
    #[must_use]
    pub fn formula_reference_ink(&self, slot: u8) -> Color {
        self.formula_reference_ink_on(slot, self.palette.surface)
    }

    /// Reference ink for an explicit document canvas, independent of UI chrome.
    #[must_use]
    pub fn formula_reference_ink_on(&self, slot: u8, surface: Color) -> Color {
        let [r, g, b, _] = surface.channels();
        let dark = u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114 < 128_000;
        let light_inks = [(35,88,194),(183,35,64),(0,120,78),(143,68,0),(119,51,177),(0,109,130),(165,33,131),(101,108,0)];
        let dark_inks = [(116,169,255),(255,130,143),(87,220,153),(255,185,102),(203,154,255),(99,220,235),(255,148,219),(209,216,98)];
        let (r,g,b) = if dark { dark_inks } else { light_inks }[usize::from(slot % 8)];
        Color::opaque(r,g,b)
    }
}
