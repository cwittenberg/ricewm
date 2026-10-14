use core::fmt;

const MIN_SCALE: f64 = 0.01;
const MAX_SCALE: f64 = 64.0;
const ZOOM_STEP: f64 = 1.25;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Size {
    width: f64,
    height: f64,
}

impl Size {
    /// Creates a finite, positive size.
    ///
    /// # Errors
    ///
    /// Returns [`ZoomError::InvalidSize`] for invalid dimensions.
    pub fn new(width: f64, height: f64) -> Result<Self, ZoomError> {
        if width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0 {
            Ok(Self { width, height })
        } else {
            Err(ZoomError::InvalidSize)
        }
    }

    #[must_use]
    pub const fn width(self) -> f64 {
        self.width
    }

    #[must_use]
    pub const fn height(self) -> f64 {
        self.height
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    #[must_use]
    pub fn contains(self, point: Point) -> bool {
        point.x >= self.origin.x
            && point.y >= self.origin.y
            && point.x <= self.origin.x + self.size.width
            && point.y <= self.origin.y + self.size.height
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FitPolicy {
    NeverUpscale,
    AllowUpscale,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Zoom {
    Actual,
    Fit(FitPolicy),
    Custom(f64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewTransform {
    viewport: Size,
    image: Size,
    zoom: Zoom,
    scale: f64,
    /// Image-space point displayed at viewport center.
    center: Point,
}

impl ViewTransform {
    /// Creates a transform for an image and viewport.
    ///
    /// # Errors
    ///
    /// Returns [`ZoomError::InvalidScale`] when zoom cannot be represented.
    pub fn new(viewport: Size, image: Size, zoom: Zoom) -> Result<Self, ZoomError> {
        let scale = scale_for(zoom, viewport, image)?;
        Ok(Self {
            viewport,
            image,
            zoom,
            scale,
            center: Point::new(image.width / 2.0, image.height / 2.0),
        })
    }

    #[must_use]
    pub const fn zoom(self) -> Zoom {
        self.zoom
    }

    #[must_use]
    pub const fn scale(self) -> f64 {
        self.scale
    }

    #[must_use]
    pub const fn center(self) -> Point {
        self.center
    }

    #[must_use]
    pub fn viewport_center(self) -> Point {
        Point::new(self.viewport.width / 2.0, self.viewport.height / 2.0)
    }

    /// Changes zoom while retaining the image point under `focal`.
    ///
    /// # Errors
    ///
    /// Returns [`ZoomError::InvalidScale`] for an unsupported scale.
    pub fn set_zoom(&mut self, zoom: Zoom, focal: Point) -> Result<(), ZoomError> {
        if !focal.x.is_finite() || !focal.y.is_finite() {
            return Err(ZoomError::InvalidPoint);
        }
        let image_focal = self.viewport_to_image(focal);
        let scale = scale_for(zoom, self.viewport, self.image)?;
        self.zoom = zoom;
        self.scale = scale;
        self.center = Point::new(
            image_focal.x - (focal.x - self.viewport.width / 2.0) / scale,
            image_focal.y - (focal.y - self.viewport.height / 2.0) / scale,
        );
        self.clamp_center();
        Ok(())
    }

    /// Increases zoom around a viewport focal point.
    ///
    /// # Errors
    ///
    /// Returns [`ZoomError::InvalidScale`] if the scale is invalid.
    pub fn zoom_in(&mut self, focal: Point) -> Result<(), ZoomError> {
        self.set_zoom(
            Zoom::Custom((self.scale * ZOOM_STEP).clamp(MIN_SCALE, MAX_SCALE)),
            focal,
        )
    }

    /// Decreases zoom around a viewport focal point.
    ///
    /// # Errors
    ///
    /// Returns [`ZoomError::InvalidScale`] if the scale is invalid.
    pub fn zoom_out(&mut self, focal: Point) -> Result<(), ZoomError> {
        self.set_zoom(
            Zoom::Custom((self.scale / ZOOM_STEP).clamp(MIN_SCALE, MAX_SCALE)),
            focal,
        )
    }

    /// Pans by a finite viewport-space delta.
    ///
    /// # Errors
    ///
    /// Returns [`ZoomError::InvalidPoint`] for an infinite or `NaN` delta.
    pub fn pan(&mut self, delta: Point) -> Result<(), ZoomError> {
        if !delta.x.is_finite() || !delta.y.is_finite() {
            return Err(ZoomError::InvalidPoint);
        }
        self.center.x -= delta.x / self.scale;
        self.center.y -= delta.y / self.scale;
        self.clamp_center();
        Ok(())
    }

    /// Reflows for a new viewport while preserving the center.
    ///
    /// # Errors
    ///
    /// Returns [`ZoomError::InvalidScale`] when fit zoom is invalid.
    pub fn resize(&mut self, viewport: Size) -> Result<(), ZoomError> {
        self.viewport = viewport;
        if matches!(self.zoom, Zoom::Fit(_)) {
            self.scale = scale_for(self.zoom, self.viewport, self.image)?;
        }
        self.clamp_center();
        Ok(())
    }

    /// Replaces image geometry and centers the new image.
    ///
    /// # Errors
    ///
    /// Returns [`ZoomError::InvalidScale`] when zoom is invalid.
    pub fn replace_image(&mut self, image: Size) -> Result<(), ZoomError> {
        self.image = image;
        self.scale = scale_for(self.zoom, self.viewport, image)?;
        self.center = Point::new(image.width / 2.0, image.height / 2.0);
        Ok(())
    }

    #[must_use]
    pub fn viewport_to_image(self, point: Point) -> Point {
        Point::new(
            self.center.x + (point.x - self.viewport.width / 2.0) / self.scale,
            self.center.y + (point.y - self.viewport.height / 2.0) / self.scale,
        )
    }

    #[must_use]
    pub fn image_rect(self) -> Rect {
        Rect {
            origin: Point::new(
                self.viewport.width / 2.0 - self.center.x * self.scale,
                self.viewport.height / 2.0 - self.center.y * self.scale,
            ),
            size: Size {
                width: self.image.width * self.scale,
                height: self.image.height * self.scale,
            },
        }
    }

    fn clamp_center(&mut self) {
        self.center.x = clamp_axis(
            self.center.x,
            self.image.width,
            self.viewport.width / self.scale,
        );
        self.center.y = clamp_axis(
            self.center.y,
            self.image.height,
            self.viewport.height / self.scale,
        );
    }
}

fn scale_for(zoom: Zoom, viewport: Size, image: Size) -> Result<f64, ZoomError> {
    let scale = match zoom {
        Zoom::Actual => 1.0,
        Zoom::Fit(policy) => {
            let fit = (viewport.width / image.width).min(viewport.height / image.height);
            match policy {
                FitPolicy::NeverUpscale => fit.min(1.0),
                FitPolicy::AllowUpscale => fit,
            }
        }
        Zoom::Custom(scale) => scale,
    };
    if scale.is_finite() && (MIN_SCALE..=MAX_SCALE).contains(&scale) {
        Ok(scale)
    } else {
        Err(ZoomError::InvalidScale)
    }
}

fn clamp_axis(center: f64, image: f64, visible: f64) -> f64 {
    if visible >= image {
        image / 2.0
    } else {
        center.clamp(visible / 2.0, image - visible / 2.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ZoomError {
    InvalidSize,
    InvalidScale,
    InvalidPoint,
}

impl fmt::Display for ZoomError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ZoomError {}
