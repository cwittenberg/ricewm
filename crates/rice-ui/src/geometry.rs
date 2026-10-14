#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    #[must_use]
    pub const fn contains(self, point: Point) -> bool {
        let right = self.origin.x.saturating_add_unsigned(self.size.width);
        let bottom = self.origin.y.saturating_add_unsigned(self.size.height);
        point.x >= self.origin.x && point.y >= self.origin.y && point.x < right && point.y < bottom
    }

    #[must_use]
    pub fn intersection(self, other: Self) -> Option<Self> {
        let left = if self.origin.x > other.origin.x {
            self.origin.x
        } else {
            other.origin.x
        };
        let top = if self.origin.y > other.origin.y {
            self.origin.y
        } else {
            other.origin.y
        };
        let right = self
            .origin
            .x
            .saturating_add_unsigned(self.size.width)
            .min(other.origin.x.saturating_add_unsigned(other.size.width));
        let bottom = self
            .origin
            .y
            .saturating_add_unsigned(self.size.height)
            .min(other.origin.y.saturating_add_unsigned(other.size.height));
        if right <= left || bottom <= top {
            return None;
        }
        Some(Self {
            origin: Point { x: left, y: top },
            size: Size {
                width: right.saturating_sub(left).cast_unsigned(),
                height: bottom.saturating_sub(top).cast_unsigned(),
            },
        })
    }

    #[must_use]
    pub const fn inset(self, insets: Insets) -> Self {
        let horizontal = insets.left.saturating_add(insets.right);
        let vertical = insets.top.saturating_add(insets.bottom);
        Self {
            origin: Point {
                x: self.origin.x.saturating_add_unsigned(insets.left),
                y: self.origin.y.saturating_add_unsigned(insets.top),
            },
            size: Size {
                width: self.size.width.saturating_sub(horizontal),
                height: self.size.height.saturating_sub(vertical),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Insets {
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
}

impl Insets {
    #[must_use]
    pub const fn all(value: u32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Constraints {
    pub min: Size,
    pub max: Size,
}

impl Constraints {
    pub const UNBOUNDED: Self = Self {
        min: Size {
            width: 0,
            height: 0,
        },
        max: Size {
            width: u32::MAX,
            height: u32::MAX,
        },
    };

    #[must_use]
    pub const fn new(min: Size, max: Size) -> Option<Self> {
        if min.width <= max.width && min.height <= max.height {
            Some(Self { min, max })
        } else {
            None
        }
    }

    #[must_use]
    pub const fn constrain(self, size: Size) -> Size {
        Size {
            width: if size.width < self.min.width {
                self.min.width
            } else if size.width > self.max.width {
                self.max.width
            } else {
                size.width
            },
            height: if size.height < self.min.height {
                self.min.height
            } else if size.height > self.max.height {
                self.max.height
            } else {
                size.height
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Axis {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Alignment {
    #[default]
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinearLayout {
    pub axis: Axis,
    pub gap: u16,
    pub cross_alignment: Alignment,
}

impl Default for LinearLayout {
    fn default() -> Self {
        Self {
            axis: Axis::Horizontal,
            gap: 4,
            cross_alignment: Alignment::Stretch,
        }
    }
}
