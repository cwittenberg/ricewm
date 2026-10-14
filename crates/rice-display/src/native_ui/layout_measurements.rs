//! Measurements live for one immutable tree/theme render, never across updates.
use rice_native_app_protocol::WidgetId;
use rice_ui::{Rect, RetainedTree, Size, Theme};
use std::{cell::RefCell, collections::HashMap};

struct Measurements {
    compact: bool,
    navigation_open: Option<WidgetId>,
    navigation_offset: u32,
    navigation_maximum: u32,
    navigation_headers: Vec<(WidgetId, Rect)>,
    navigation_grids: HashMap<WidgetId, u32>,
    tree: usize,
    theme: usize,
    sizes: HashMap<WidgetId, Size>,
    heights: HashMap<(WidgetId, u32), u32>,
}

thread_local! {
    static CURRENT: RefCell<Option<Measurements>> = const { RefCell::new(None) };
}

pub(super) struct RenderMeasurements<'a> {
    previous: Option<Measurements>,
    // Keep the measured inputs immutable until the cache is discarded.
    _inputs: (&'a RetainedTree, &'a Theme),
}

impl<'a> RenderMeasurements<'a> {
    pub(super) fn new(tree: &'a RetainedTree, theme: &'a Theme, compact: bool) -> Self {
        let previous = CURRENT.with(|slot| {
            slot.replace(Some(Measurements {
                compact,
                navigation_open: None,
                navigation_offset: 0,
                navigation_maximum: 0,
                navigation_headers: Vec::new(),
                navigation_grids: HashMap::new(),
                tree: std::ptr::from_ref(tree) as usize,
                theme: std::ptr::from_ref(theme) as usize,
                sizes: HashMap::new(),
                heights: HashMap::new(),
            }))
        });
        Self {
            previous,
            _inputs: (tree, theme),
        }
    }
}

impl Drop for RenderMeasurements<'_> {
    fn drop(&mut self) {
        CURRENT.with(|slot| {
            slot.replace(self.previous.take());
        });
    }
}

fn with_cache<T>(
    tree: &RetainedTree,
    theme: &Theme,
    action: impl FnOnce(&mut Measurements) -> T,
) -> Option<T> {
    CURRENT.with(|slot| {
        let mut current = slot.borrow_mut();
        let cache = current.as_mut()?;
        if cache.tree != std::ptr::from_ref(tree) as usize
            || cache.theme != std::ptr::from_ref(theme) as usize
        {
            return None;
        }
        Some(action(cache))
    })
}

pub(super) fn size(tree: &RetainedTree, theme: &Theme, id: WidgetId) -> Option<Size> {
    with_cache(tree, theme, |cache| cache.sizes.get(&id).copied()).flatten()
}

pub(super) fn save_size(tree: &RetainedTree, theme: &Theme, id: WidgetId, size: Size) {
    with_cache(tree, theme, |cache| {
        if cache.sizes.len() < rice_ui::MAX_WIDGETS {
            cache.sizes.insert(id, size);
        }
    });
}

pub(super) fn height(tree: &RetainedTree, theme: &Theme, id: WidgetId, width: u32) -> Option<u32> {
    with_cache(tree, theme, |cache| {
        cache.heights.get(&(id, width)).copied()
    })
    .flatten()
}

pub(super) fn save_height(
    tree: &RetainedTree,
    theme: &Theme,
    id: WidgetId,
    width: u32,
    height: u32,
) {
    with_cache(tree, theme, |cache| {
        // Width-dependent callers may measure a widget more than once. Bound
        // retained entries without changing layout when the cache fills.
        if cache.heights.len() < rice_ui::MAX_WIDGETS * 4 {
            cache.heights.insert((id, width), height);
        }
    });
}

pub(super) fn compact_form_factor() -> bool {
    CURRENT.with(|slot| slot.borrow().as_ref().is_some_and(|cache| cache.compact))
}

pub(super) fn navigation_grid_extent(id: WidgetId) -> Option<u32> {
    CURRENT.with(|slot| slot.borrow().as_ref()?.navigation_grids.get(&id).copied())
}

pub(super) fn record_navigation_grid(id: WidgetId, extent: u32) {
    CURRENT.with(|slot| {
        if let Some(cache) = slot.borrow_mut().as_mut()
            && cache.navigation_grids.len() < rice_ui::MAX_WIDGETS
        {
            cache.navigation_grids.insert(id, extent);
        }
    });
}

pub(super) fn set_navigation_open(id: Option<WidgetId>) {
    CURRENT.with(|slot| { if let Some(cache) = slot.borrow_mut().as_mut() { cache.navigation_open = id; } });
}
pub(super) fn navigation_open(id: WidgetId) -> bool {
    CURRENT.with(|slot| slot.borrow().as_ref().is_some_and(|cache| cache.navigation_open == Some(id)))
}
pub(super) fn record_navigation_header(id: WidgetId, rect: Rect) {
    CURRENT.with(|slot| { if let Some(cache) = slot.borrow_mut().as_mut() { cache.navigation_headers.push((id, rect)); } });
}
pub(super) fn navigation_headers() -> Vec<(WidgetId, Rect)> {
    CURRENT.with(|slot| slot.borrow().as_ref().map_or_else(Vec::new, |cache| cache.navigation_headers.clone()))
}

pub(super) fn set_navigation_offset(offset: u32) { CURRENT.with(|slot| { if let Some(cache)=slot.borrow_mut().as_mut(){cache.navigation_offset=offset;} }); }
pub(super) fn navigation_scroll(maximum: u32) -> u32 { CURRENT.with(|slot| { let mut slot=slot.borrow_mut(); let Some(cache)=slot.as_mut() else {return 0;};cache.navigation_maximum=maximum;cache.navigation_offset.min(maximum) }) }
pub(super) fn navigation_maximum() -> u32 { CURRENT.with(|slot| slot.borrow().as_ref().map_or(0,|cache|cache.navigation_maximum)) }
