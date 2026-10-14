use std::collections::BTreeMap;

use yserver_protocol::x11::{self, AtomId, ResourceId};

pub(crate) const MAX_PROPERTY_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug)]
pub(crate) struct Surface {
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) depth: u8,
    pub(crate) background_pixel: u32,
    pub(crate) pixels: Option<Vec<u32>>,
    pub(crate) alpha_mode: SurfaceAlphaMode,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum SurfaceAlphaMode {
    #[default]
    Unknown,
    Argb,
    OpaqueXrgb,
}

impl Surface {
    pub(crate) fn new(width: u16, height: u16, depth: u8, background_pixel: u32) -> Self {
        let depth_mask = if depth >= 32 {
            u32::MAX
        } else {
            (1u32 << depth) - 1
        };
        Self {
            width,
            height,
            depth,
            background_pixel: background_pixel & depth_mask,
            pixels: None,
            alpha_mode: SurfaceAlphaMode::Unknown,
        }
    }

    pub(crate) fn pixel_count(&self) -> Option<usize> {
        usize::from(self.width).checked_mul(usize::from(self.height))
    }

    pub(crate) fn materialized_bytes(&self) -> usize {
        self.pixels
            .as_ref()
            .map_or(0, |pixels| pixels.len().saturating_mul(4))
    }

    pub(crate) fn pixel(&self, x: u16, y: u16) -> u32 {
        let index = usize::from(y)
            .saturating_mul(usize::from(self.width))
            .saturating_add(usize::from(x));
        self.pixels
            .as_ref()
            .and_then(|pixels| pixels.get(index))
            .copied()
            .unwrap_or(self.background_pixel)
    }

    pub(crate) fn materialize(&mut self) -> bool {
        if self.pixels.is_some() {
            return true;
        }
        let Some(pixel_count) = self.pixel_count() else {
            return false;
        };
        self.pixels = Some(vec![self.background_pixel; pixel_count]);
        true
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Pixmap {
    pub(crate) id: ResourceId,
    pub(crate) owner: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct Font {
    pub(crate) owner: u32,
    pub(crate) name: String,
}

#[derive(Clone, Debug)]
pub(crate) struct Colormap {
    pub(crate) owner: Option<u32>,
    pub(crate) visual: ResourceId,
}

#[derive(Clone, Debug)]
pub(crate) struct Cursor {
    pub(crate) owner: u32,
    // Window references keep the cursor alive after FreeCursor removes its XID.
    pub(crate) shape: std::sync::Arc<std::sync::atomic::AtomicU32>,
}

impl Cursor {
    pub(crate) fn new(owner: u32, shape: u32) -> Self {
        Self { owner, shape: std::sync::Arc::new(std::sync::atomic::AtomicU32::new(shape)) }
    }

    pub(crate) fn shape(&self) -> u32 {
        self.shape.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub(crate) fn set_shape(&self, shape: u32) {
        self.shape.store(shape, std::sync::atomic::Ordering::Relaxed);
    }
}

#[derive(Clone, Debug)]
pub(crate) struct GradientStop {
    pub(crate) position: i32,
    pub(crate) color: u32,
}

#[derive(Clone, Debug)]
pub(crate) enum PictureSource {
    Drawable(ResourceId),
    Solid(u32),
    LinearGradient {
        first: (i32, i32),
        second: (i32, i32),
        stops: Vec<GradientStop>,
    },
    RadialGradient {
        inner: (i32, i32),
        outer: (i32, i32),
        inner_radius: i32,
        outer_radius: i32,
        stops: Vec<GradientStop>,
    },
    ConicalGradient {
        center: (i32, i32),
        angle: i32,
        stops: Vec<GradientStop>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PictureFilter {
    Nearest,
    Bilinear,
}

#[derive(Clone, Debug)]
pub(crate) struct Picture {
    pub(crate) owner: u32,
    pub(crate) source: PictureSource,
    pub(crate) format: u32,
    pub(crate) repeat: u8,
    pub(crate) component_alpha: bool,
    pub(crate) clip_x_origin: i16,
    pub(crate) clip_y_origin: i16,
    pub(crate) clip_mask: Option<ResourceId>,
    pub(crate) clip_rectangles: Option<Vec<(i16, i16, u16, u16)>>,
    pub(crate) transform: [i32; 9],
    pub(crate) filter: PictureFilter,
}

#[derive(Clone, Debug)]
pub(crate) struct Glyph {
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) x: i16,
    pub(crate) y: i16,
    pub(crate) x_off: i16,
    pub(crate) y_off: i16,
    /// Dense row-major alpha coverage.
    pub(crate) alpha: Vec<u8>,
}

#[derive(Clone, Debug)]
pub(crate) struct GlyphSet {
    pub(crate) owner: u32,
    pub(crate) format: u32,
    /// Server-owned storage shared by every `ReferenceGlyphSet` alias.
    pub(crate) storage: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct GraphicsContext {
    pub(crate) owner: u32,
    pub(crate) depth: u8,
    pub(crate) function: u8,
    pub(crate) plane_mask: u32,
    pub(crate) foreground: u32,
    pub(crate) background: u32,
    pub(crate) line_width: u16,
    pub(crate) line_style: u8,
    pub(crate) cap_style: u8,
    pub(crate) join_style: u8,
    pub(crate) fill_style: u8,
    pub(crate) fill_rule: u8,
    pub(crate) tile: Option<ResourceId>,
    pub(crate) stipple: Option<ResourceId>,
    pub(crate) tile_x_origin: i16,
    pub(crate) tile_y_origin: i16,
    pub(crate) font: Option<ResourceId>,
    pub(crate) subwindow_mode: u8,
    pub(crate) graphics_exposures: bool,
    pub(crate) clip_x_origin: i16,
    pub(crate) clip_y_origin: i16,
    pub(crate) clip_mask: Option<ResourceId>,
    pub(crate) clip_rectangles: Option<x11::ClipRectangles>,
    pub(crate) dash_offset: u16,
    pub(crate) dashes: Vec<u8>,
    pub(crate) arc_mode: u8,
}

impl GraphicsContext {
    pub(crate) fn new(owner: u32, depth: u8) -> Self {
        Self {
            owner,
            depth,
            function: 3,
            plane_mask: u32::MAX,
            foreground: 0,
            background: 0x00ff_ffff,
            line_width: 0,
            line_style: 0,
            cap_style: 1,
            join_style: 0,
            fill_style: 0,
            fill_rule: 0,
            tile: None,
            stipple: None,
            tile_x_origin: 0,
            tile_y_origin: 0,
            font: None,
            subwindow_mode: 0,
            graphics_exposures: true,
            clip_x_origin: 0,
            clip_y_origin: 0,
            clip_mask: None,
            clip_rectangles: None,
            dash_offset: 0,
            dashes: vec![4, 4],
            arc_mode: 1,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Property {
    pub(crate) r#type: AtomId,
    pub(crate) format: u8,
    pub(crate) data: Vec<u8>,
}

#[derive(Clone, Debug)]
pub(crate) struct Window {
    pub(crate) id: ResourceId,
    pub(crate) owner: Option<u32>,
    pub(crate) parent: ResourceId,
    pub(crate) children: Vec<ResourceId>,
    pub(crate) x: i16,
    pub(crate) y: i16,
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) border_width: u16,
    pub(crate) depth: u8,
    pub(crate) class: u16,
    pub(crate) visual: ResourceId,
    pub(crate) background_pixel: u32,
    pub(crate) bit_gravity: u8,
    pub(crate) win_gravity: u8,
    pub(crate) backing_store: u8,
    pub(crate) backing_planes: u32,
    pub(crate) backing_pixel: u32,
    pub(crate) override_redirect: bool,
    pub(crate) save_under: bool,
    pub(crate) do_not_propagate_mask: u16,
    pub(crate) colormap: ResourceId,
    pub(crate) mapped: bool,
    pub(crate) properties: BTreeMap<AtomId, Property>,
}

impl Window {
    pub(crate) fn root(
        id: ResourceId,
        visual: ResourceId,
        colormap: ResourceId,
        width: u16,
        height: u16,
    ) -> Self {
        Self {
            id,
            owner: None,
            parent: id,
            children: Vec::new(),
            x: 0,
            y: 0,
            width,
            height,
            border_width: 0,
            depth: 24,
            class: 1,
            visual,
            background_pixel: 0x0010_151c,
            bit_gravity: 0,
            win_gravity: 0,
            backing_store: 0,
            backing_planes: u32::MAX,
            backing_pixel: 0,
            override_redirect: false,
            save_under: false,
            do_not_propagate_mask: 0,
            colormap,
            mapped: true,
            properties: BTreeMap::new(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct AtomTable {
    by_name: BTreeMap<String, AtomId>,
    by_id: BTreeMap<u32, String>,
    next_id: u32,
}

impl AtomTable {
    pub(crate) fn new() -> Self {
        Self {
            by_name: BTreeMap::new(),
            by_id: BTreeMap::new(),
            next_id: 69,
        }
    }

    pub(crate) fn intern(&mut self, name: &str, only_if_exists: bool) -> AtomId {
        if let Some(atom) = x11::well_known_atom(name) {
            return atom;
        }
        if let Some(atom) = self.by_name.get(name) {
            return *atom;
        }
        if only_if_exists {
            return AtomId(0);
        }
        let atom = AtomId(self.next_id);
        self.next_id = self.next_id.saturating_add(1).max(69);
        self.by_name.insert(name.to_owned(), atom);
        self.by_id.insert(atom.0, name.to_owned());
        atom
    }

    pub(crate) fn name(&self, atom: AtomId) -> Option<&str> {
        x11::well_known_atom_name(atom).or_else(|| self.by_id.get(&atom.0).map(String::as_str))
    }

    pub(crate) fn exists(&self, atom: AtomId) -> bool {
        atom.0 != 0 && self.name(atom).is_some()
    }
}
