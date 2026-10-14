//! Pluggable `RiceWM` tray layout model.
//!
//! Each item type is a Strategy registered with [`TrayPluginRegistry`]. New
//! tray plugins can be added without changing layout parsing or editing code.

#![forbid(unsafe_code)]

pub const DEFAULT_TRAY_LAYOUT: &str = "performance,connectivity,sound,workspaces,clock";
pub const MAX_TRAY_ITEMS: usize = 12;
pub const DEFAULT_LAUNCHER_COLOR: u32 = 0x00ff_ffff;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrayIcon(u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrayIconSpec {
    pub icon: TrayIcon,
    pub slug: &'static str,
    pub title: &'static str,
    pub semantic: u16,
}

macro_rules! icon_catalog {
    ($(($index:literal, $slug:literal, $title:literal, $semantic:literal)),+ $(,)?) => {
        pub const TRAY_ICON_CATALOG: &[TrayIconSpec] = &[
            $(TrayIconSpec {
                icon: TrayIcon($index),
                slug: $slug,
                title: $title,
                semantic: $semantic,
            }),+
        ];
    };
}

icon_catalog!(
    (0, "calculator", "Calculator", 60),
    (1, "chevron-down", "Chevron down", 74),
    (2, "chevron-left", "Chevron left", 31),
    (3, "chevron-right", "Chevron right", 32),
    (4, "chevron-up", "Chevron up", 73),
    (5, "desktop", "Desktop", 70),
    (6, "dialog-information", "Information", 51),
    (7, "dialog-warning", "Warning", 52),
    (8, "document-new", "New document", 10),
    (9, "document-open", "Open document", 11),
    (10, "document-save-as", "Save as", 67),
    (11, "document-save", "Save", 12),
    (12, "drive", "Drive", 64),
    (13, "edit-copy", "Copy", 23),
    (14, "edit-cut", "Cut", 22),
    (15, "edit-delete", "Delete", 25),
    (16, "edit-find", "Find", 68),
    (17, "edit-paste", "Paste", 24),
    (18, "edit-redo", "Redo", 21),
    (19, "edit-rename", "Rename", 69),
    (20, "edit-undo", "Undo", 20),
    (21, "file", "File", 2),
    (22, "files", "Files", 62),
    (23, "folder", "Folder", 3),
    (24, "home", "Home", 5),
    (25, "image-viewer", "Image viewer", 63),
    (26, "image", "Image", 71),
    (27, "lock", "Lock", 66),
    (28, "menu", "Menu", 75),
    (29, "navigate-up", "Navigate up", 33),
    (30, "network", "Network", 65),
    (31, "object-add", "Add", 76),
    (32, "object-remove", "Remove", 77),
    (33, "search", "Search", 30),
    (34, "selection-check", "Selected", 78),
    (35, "settings", "Settings", 50),
    (36, "terminal", "Terminal", 72),
    (37, "text-editor", "Text editor", 61),
    (38, "trash-empty", "Empty trash", 6),
    (39, "trash-full", "Full trash", 58),
    (40, "view-detail", "Detail view", 79),
    (41, "view-grid", "Grid view", 41),
    (42, "view-refresh", "Refresh", 34),
    (43, "window-close", "Close window", 54),
    (44, "window-maximize", "Maximize window", 56),
    (45, "window-minimize", "Minimize window", 55),
    (46, "window-restore", "Restore window", 57),
);

impl TrayIcon {
    #[must_use]
    pub fn from_index(value: u8) -> Option<Self> {
        TRAY_ICON_CATALOG
            .get(usize::from(value))
            .map(|spec| spec.icon)
    }

    #[must_use]
    pub fn from_slug(value: &str) -> Option<Self> {
        TRAY_ICON_CATALOG
            .iter()
            .find(|spec| spec.slug == value)
            .map(|spec| spec.icon)
    }

    #[must_use]
    pub fn from_semantic(value: u16) -> Option<Self> {
        TRAY_ICON_CATALOG
            .iter()
            .find(|spec| spec.semantic == value)
            .map(|spec| spec.icon)
    }

    #[must_use]
    pub fn spec(self) -> &'static TrayIconSpec {
        &TRAY_ICON_CATALOG[usize::from(self.0)]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LauncherTarget {
    Files,
    Terminal,
    ImageViewer,
}

impl LauncherTarget {
    pub const ALL: [Self; 3] = [Self::Files, Self::Terminal, Self::ImageViewer];

    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Files => "files",
            Self::Terminal => "terminal",
            Self::ImageViewer => "image-viewer",
        }
    }

    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Files => "Files",
            Self::Terminal => "Terminal",
            Self::ImageViewer => "Image Viewer",
        }
    }

    #[must_use]
    pub fn from_slug(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|target| target.slug() == value)
    }

    #[must_use]
    pub fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|target| *target == self)
            .unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    #[must_use]
    pub fn default_icon(self) -> TrayIcon {
        match self {
            Self::Files => TrayIcon(22),
            Self::Terminal => TrayIcon(36),
            Self::ImageViewer => TrayIcon(25),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrayLauncher {
    pub target: LauncherTarget,
    pub icon: TrayIcon,
    pub color: u32,
}

impl TrayLauncher {
    #[must_use]
    pub fn new(target: LauncherTarget) -> Self {
        Self {
            target,
            icon: target.default_icon(),
            color: DEFAULT_LAUNCHER_COLOR,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrayItem {
    Clock,
    Workspaces,
    Sound,
    Performance,
    Connectivity,
    Launcher(TrayLauncher),
}

impl TrayItem {
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Clock => "Clock",
            Self::Workspaces => "Workspaces",
            Self::Sound => "Sound",
            Self::Performance => "Performance graph",
            Self::Connectivity => "Connectivity strategy",
            Self::Launcher(_) => "Custom app launcher",
        }
    }

    #[must_use]
    pub fn detail(self) -> Option<&'static str> {
        match self {
            Self::Launcher(launcher) => Some(launcher.target.title()),
            Self::Clock
            | Self::Workspaces
            | Self::Sound
            | Self::Performance
            | Self::Connectivity => None,
        }
    }

    #[must_use]
    pub const fn plugin_key(self) -> &'static str {
        match self {
            Self::Clock => "clock",
            Self::Workspaces => "workspaces",
            Self::Sound => "sound",
            Self::Performance => "performance",
            Self::Connectivity => "connectivity",
            Self::Launcher(_) => "launcher",
        }
    }

    #[must_use]
    pub fn token(self) -> String {
        match self {
            Self::Launcher(launcher) => format!(
                "launcher:{}:{}:{:06x}",
                launcher.target.slug(),
                launcher.icon.spec().slug,
                launcher.color & 0x00ff_ffff,
            ),
            _ => self.plugin_key().to_owned(),
        }
    }
}

/// GOF Strategy seam for a tray item family.
pub trait TrayItemPlugin {
    fn key(&self) -> &'static str;
    fn title(&self) -> &'static str;
    fn create_default(&self) -> TrayItem;
    fn parse(&self, token: &str) -> Option<TrayItem>;

    fn accepts_another(&self, items: &[TrayItem]) -> bool {
        !items.iter().any(|item| item.plugin_key() == self.key())
    }
}

#[derive(Default)]
pub struct TrayPluginRegistry {
    plugins: Vec<Box<dyn TrayItemPlugin>>,
}

impl TrayPluginRegistry {
    #[must_use]
    pub fn with_builtins() -> Self {
        let mut registry = Self::default();
        registry.register(ClockPlugin);
        registry.register(WorkspacesPlugin);
        registry.register(SoundPlugin);
        registry.register(PerformancePlugin);
        registry.register(ConnectivityPlugin);
        registry
    }

    pub fn register(&mut self, plugin: impl TrayItemPlugin + 'static) {
        if !self
            .plugins
            .iter()
            .any(|candidate| candidate.key() == plugin.key())
        {
            self.plugins.push(Box::new(plugin));
        }
    }

    pub fn plugins(&self) -> impl Iterator<Item = &dyn TrayItemPlugin> {
        self.plugins.iter().map(Box::as_ref)
    }

    #[must_use]
    pub fn parse(&self, value: &str) -> Option<Vec<TrayItem>> {
        if value.is_empty() {
            return Some(Vec::new());
        }
        let tokens = value.split(',').collect::<Vec<_>>();
        if tokens.len() > MAX_TRAY_ITEMS || tokens.iter().any(|token| token.is_empty()) {
            return None;
        }
        let mut items = Vec::with_capacity(tokens.len());
        for token in tokens {
            let (plugin, item) = self
                .plugins
                .iter()
                .find_map(|plugin| plugin.parse(token).map(|item| (plugin, item)))?;
            if !plugin.accepts_another(&items) {
                return None;
            }
            items.push(item);
        }
        Some(items)
    }

    #[must_use]
    pub fn serialize(items: &[TrayItem]) -> String {
        items
            .iter()
            .map(|item| item.token())
            .collect::<Vec<_>>()
            .join(",")
    }
}

struct ClockPlugin;
struct WorkspacesPlugin;
struct SoundPlugin;
struct PerformancePlugin;
struct ConnectivityPlugin;

macro_rules! singleton_plugin {
    ($type:ty, $key:literal, $title:literal, $item:expr) => {
        impl TrayItemPlugin for $type {
            fn key(&self) -> &'static str {
                $key
            }
            fn title(&self) -> &'static str {
                $title
            }
            fn create_default(&self) -> TrayItem {
                $item
            }
            fn parse(&self, token: &str) -> Option<TrayItem> {
                (token == $key).then_some($item)
            }
        }
    };
}

singleton_plugin!(ClockPlugin, "clock", "Clock", TrayItem::Clock);
singleton_plugin!(
    WorkspacesPlugin,
    "workspaces",
    "Workspaces",
    TrayItem::Workspaces
);
singleton_plugin!(SoundPlugin, "sound", "Sound", TrayItem::Sound);
singleton_plugin!(
    PerformancePlugin,
    "performance",
    "Performance graph",
    TrayItem::Performance
);
singleton_plugin!(
    ConnectivityPlugin,
    "connectivity",
    "Connectivity strategy",
    TrayItem::Connectivity
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_round_trips_built_in_strategies() {
        let registry = TrayPluginRegistry::with_builtins();
        let items = registry.parse(DEFAULT_TRAY_LAYOUT).expect("default layout");
        assert_eq!(
            items,
            [
                TrayItem::Performance,
                TrayItem::Connectivity,
                TrayItem::Sound,
                TrayItem::Workspaces,
                TrayItem::Clock,
            ]
        );
        assert_eq!(TrayPluginRegistry::serialize(&items), DEFAULT_TRAY_LAYOUT);
        assert_eq!(registry.plugins().count(), 5);
    }

    #[test]
    fn layout_rejects_unknown_and_duplicate_plugins() {
        let registry = TrayPluginRegistry::with_builtins();
        assert!(registry.parse("clock,clock").is_none());
        assert!(registry.parse("weather").is_none());
        assert_eq!(registry.parse(""), Some(Vec::new()));
    }
}
