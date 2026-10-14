//! Wallpaper geometry and workspace policy.

use std::collections::BTreeMap;

/// A stable workspace identifier supplied by the window-manager adapter.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct WorkspaceId(String);

impl WorkspaceId {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// Signed origin permits a centered image to extend beyond the viewport.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    #[must_use]
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl Color {
    pub const BLACK: Self = Self::rgb(0, 0, 0);

    #[must_use]
    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha: 255,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WallpaperMode {
    Center,
    Tile,
    Stretch,
    Fit,
    #[default]
    Crop,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Wallpaper {
    Solid(Color),
    Image {
        resource: String,
        mode: WallpaperMode,
        background: Color,
    },
}

impl Default for Wallpaper {
    fn default() -> Self {
        Self::Solid(Color::BLACK)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImagePlacement {
    /// Region decoded from the source image.
    pub source: Rect,
    /// Region in desktop coordinates.
    pub destination: Rect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WallpaperLayout {
    Solid {
        color: Color,
        viewport: Rect,
    },
    Image {
        resource: String,
        background: Color,
        placement: ImagePlacement,
        clip: Rect,
    },
    Tiled {
        resource: String,
        background: Color,
        tile_size: Size,
        origin: (i32, i32),
        clip: Rect,
    },
}

/// Strategy seam for alternate wallpaper policies without changing settings.
pub trait WallpaperLayoutStrategy {
    fn layout(
        &self,
        wallpaper: &Wallpaper,
        viewport: Size,
        image_intrinsic_size: Option<Size>,
    ) -> WallpaperLayout;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultWallpaperLayout;

impl WallpaperLayoutStrategy for DefaultWallpaperLayout {
    fn layout(
        &self,
        wallpaper: &Wallpaper,
        viewport: Size,
        image_intrinsic_size: Option<Size>,
    ) -> WallpaperLayout {
        let clip = viewport_rect(viewport);
        let Wallpaper::Image {
            resource,
            mode,
            background,
        } = wallpaper
        else {
            let Wallpaper::Solid(color) = wallpaper else {
                unreachable!();
            };
            return WallpaperLayout::Solid {
                color: *color,
                viewport: clip,
            };
        };
        let Some(image) = image_intrinsic_size.filter(|size| !size.is_empty()) else {
            return WallpaperLayout::Solid {
                color: *background,
                viewport: clip,
            };
        };

        match mode {
            WallpaperMode::Tile => WallpaperLayout::Tiled {
                resource: resource.clone(),
                background: *background,
                tile_size: image,
                origin: (0, 0),
                clip,
            },
            mode => WallpaperLayout::Image {
                resource: resource.clone(),
                background: *background,
                placement: image_placement(*mode, viewport, image),
                clip,
            },
        }
    }
}

/// Common wallpaper plus sparse workspace overrides.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WallpaperSettings {
    common: Wallpaper,
    workspace: BTreeMap<WorkspaceId, Wallpaper>,
}

impl WallpaperSettings {
    #[must_use]
    pub fn new(common: Wallpaper) -> Self {
        Self {
            common,
            workspace: BTreeMap::new(),
        }
    }

    #[must_use]
    pub const fn common(&self) -> &Wallpaper {
        &self.common
    }

    pub fn set_common(&mut self, wallpaper: Wallpaper) {
        self.common = wallpaper;
    }

    pub fn set_workspace(&mut self, id: WorkspaceId, wallpaper: Wallpaper) {
        self.workspace.insert(id, wallpaper);
    }

    pub fn clear_workspace(&mut self, id: &WorkspaceId) -> Option<Wallpaper> {
        self.workspace.remove(id)
    }

    #[must_use]
    pub fn for_workspace(&self, id: &WorkspaceId) -> &Wallpaper {
        self.workspace.get(id).unwrap_or(&self.common)
    }

    #[must_use]
    pub fn layout<S: WallpaperLayoutStrategy>(
        &self,
        strategy: &S,
        id: &WorkspaceId,
        viewport: Size,
        image_intrinsic_size: Option<Size>,
    ) -> WallpaperLayout {
        strategy.layout(self.for_workspace(id), viewport, image_intrinsic_size)
    }
}

fn viewport_rect(size: Size) -> Rect {
    Rect::new(0, 0, size.width, size.height)
}

fn image_placement(mode: WallpaperMode, viewport: Size, image: Size) -> ImagePlacement {
    match mode {
        WallpaperMode::Center => ImagePlacement {
            source: Rect::new(0, 0, image.width, image.height),
            destination: centered(viewport, image),
        },
        WallpaperMode::Stretch => ImagePlacement {
            source: Rect::new(0, 0, image.width, image.height),
            destination: viewport_rect(viewport),
        },
        WallpaperMode::Fit => {
            let fitted = aspect_fit(image, viewport);
            ImagePlacement {
                source: Rect::new(0, 0, image.width, image.height),
                destination: centered(viewport, fitted),
            }
        }
        WallpaperMode::Crop => ImagePlacement {
            source: centered_crop(image, viewport),
            destination: viewport_rect(viewport),
        },
        WallpaperMode::Tile => unreachable!("tile is represented without expansion"),
    }
}

fn centered(outer: Size, inner: Size) -> Rect {
    let x = (i64::from(outer.width) - i64::from(inner.width)) / 2;
    let y = (i64::from(outer.height) - i64::from(inner.height)) / 2;
    Rect::new(
        saturating_i32(x),
        saturating_i32(y),
        inner.width,
        inner.height,
    )
}

fn aspect_fit(source: Size, bounds: Size) -> Size {
    if source.is_empty() || bounds.is_empty() {
        return Size::default();
    }
    let width_limited = u64::from(bounds.width) * u64::from(source.height)
        <= u64::from(bounds.height) * u64::from(source.width);
    if width_limited {
        Size::new(
            bounds.width,
            scaled(bounds.width, source.height, source.width),
        )
    } else {
        Size::new(
            scaled(bounds.height, source.width, source.height),
            bounds.height,
        )
    }
}

fn centered_crop(source: Size, bounds: Size) -> Rect {
    if source.is_empty() || bounds.is_empty() {
        return Rect::default();
    }
    let source_is_wider = u64::from(source.width) * u64::from(bounds.height)
        > u64::from(source.height) * u64::from(bounds.width);
    if source_is_wider {
        let width = scaled(source.height, bounds.width, bounds.height).min(source.width);
        Rect::new(
            saturating_i32(i64::from(source.width - width) / 2),
            0,
            width,
            source.height,
        )
    } else {
        let height = scaled(source.width, bounds.height, bounds.width).min(source.height);
        Rect::new(
            0,
            saturating_i32(i64::from(source.height - height) / 2),
            source.width,
            height,
        )
    }
}

fn scaled(value: u32, numerator: u32, denominator: u32) -> u32 {
    let result = u64::from(value) * u64::from(numerator) / u64::from(denominator.max(1));
    u32::try_from(result).unwrap_or(u32::MAX)
}

fn saturating_i32(value: i64) -> i32 {
    i32::try_from(value).unwrap_or(if value.is_negative() {
        i32::MIN
    } else {
        i32::MAX
    })
}
