//! Canonical vocabulary for the RiceWM native UI component library.
//!
//! This deliberately lives beside the retained widget primitives. The visual
//! reference, documentation, and conformance tooling all consume these stable
//! IDs instead of maintaining independent lists of controls.

#[path = "catalog_i18n.rs"]
mod localization;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ComponentCategory {
    Controls,
    Collections,
    Commands,
    Feedback,
    Windows,
    Environment,
    Visuals,
    Layout,
}

impl ComponentCategory {
    pub const ALL: [Self; 8] = [
        Self::Controls,
        Self::Collections,
        Self::Commands,
        Self::Feedback,
        Self::Windows,
        Self::Environment,
        Self::Visuals,
        Self::Layout,
    ];

    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Controls => "Controls",
            Self::Collections => "Collections & navigation",
            Self::Commands => "Commands",
            Self::Feedback => "Feedback & messages",
            Self::Windows => "Windows & dialogs",
            Self::Environment => "Desktop environment",
            Self::Visuals => "Visual language",
            Self::Layout => "Layout & surfaces",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComponentImplementation {
    /// One retained `WidgetKind` is the complete public control.
    Primitive,
    /// A reusable construction made from multiple retained primitives.
    Composite,
    /// A documented interaction/presentation contract rather than one widget.
    Pattern,
    /// Supplied by the RiceWM shell or a capability host, not by applications.
    HostOwned,
    /// Considered and deliberately excluded; the replacement is documented.
    NotAdopted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComponentSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub category: ComponentCategory,
    pub implementation: ComponentImplementation,
    /// Anchor in `RICE_UI_CONTROL_CATALOG.md`.
    pub standard: &'static str,
}

macro_rules! component {
    ($id:literal, $name:literal, $category:ident, $implementation:ident) => {
        ComponentSpec {
            id: $id,
            name: $name,
            category: ComponentCategory::$category,
            implementation: ComponentImplementation::$implementation,
            standard: concat!("#", $id),
        }
    };
}

/// Exhaustive public component index. Additions require a normative catalog
/// section, a gallery specimen (or an explicit host/not-adopted explanation),
/// and an update to the compliance audit.
pub const COMPONENT_CATALOG: &[ComponentSpec] = &[
    component!("label", "Label", Controls, Primitive),
    component!("button", "Command button", Controls, Primitive),
    component!("command-link", "Command link", Controls, Composite),
    component!("split-button", "Split button", Controls, Composite),
    component!("toggle", "Toggle switch", Controls, Primitive),
    component!("check-box", "Check box", Controls, Primitive),
    component!("radio-button", "Radio button", Controls, Primitive),
    component!("drop-down-list", "Drop-down list", Controls, Composite),
    component!("combo-box", "Combo box", Controls, Composite),
    component!("link", "Link", Controls, Composite),
    component!("text-field", "Text field", Controls, Primitive),
    component!("password-field", "Password field", Controls, Primitive),
    component!("search-box", "Search box", Controls, Composite),
    component!("text-editor", "Text editor", Controls, Primitive),
    component!("spin-control", "Spin control", Controls, Composite),
    component!("slider", "Slider", Controls, Primitive),
    component!("scroll-bar", "Scroll bar", Controls, Primitive),
    component!("progress-bar", "Progress bar", Controls, Primitive),
    component!(
        "activity-indicator",
        "Activity indicator",
        Controls,
        Primitive
    ),
    component!("group-box", "Group box", Controls, Primitive),
    component!("image", "Image", Controls, Primitive),
    component!("markdown", "Markdown view", Controls, Primitive),
    component!("list-box", "List box", Collections, Composite),
    component!("list-view", "List view", Collections, Primitive),
    component!("table", "Table", Collections, Primitive),
    component!("tree-view", "Tree view", Collections, Primitive),
    component!("icon-grid", "Icon grid", Collections, Primitive),
    component!("tabs", "Tabs", Collections, Primitive),
    component!("breadcrumb", "Breadcrumb", Collections, Primitive),
    component!(
        "accordion",
        "Progressive disclosure",
        Collections,
        Primitive
    ),
    component!("menu-bar", "Menu bar", Commands, Primitive),
    component!("menu", "Menu", Commands, Primitive),
    component!("submenu", "Submenu", Commands, Composite),
    component!("context-menu", "Context menu", Commands, Composite),
    component!("tool-bar", "Tool bar", Commands, Primitive),
    component!("status-bar", "Status bar", Commands, Composite),
    component!("ribbon", "Ribbon", Commands, NotAdopted),
    component!("tooltip", "Tooltip", Feedback, Primitive),
    component!("infotip", "Infotip", Feedback, Composite),
    component!("balloon", "Balloon", Feedback, Composite),
    component!("notification", "Notification", Feedback, Primitive),
    component!("in-place-message", "In-place message", Feedback, Pattern),
    component!("error-message", "Error message", Feedback, Pattern),
    component!("warning-message", "Warning message", Feedback, Pattern),
    component!("progress-feedback", "Progress feedback", Feedback, Pattern),
    component!("empty-state", "Empty state", Feedback, Pattern),
    component!("pointer-feedback", "Pointer feedback", Feedback, HostOwned),
    component!("dialog", "Dialog", Windows, Primitive),
    component!(
        "confirmation-dialog",
        "Confirmation dialog",
        Windows,
        Composite
    ),
    component!("warning-dialog", "Warning dialog", Windows, Composite),
    component!("error-dialog", "Error dialog", Windows, Composite),
    component!("property-window", "Property window", Windows, Composite),
    component!(
        "settings-page",
        "Settings and preferences page",
        Windows,
        Composite
    ),
    component!("open-dialog", "Open dialog", Windows, Composite),
    component!("save-dialog", "Save dialog", Windows, Composite),
    component!("popover", "Popover", Windows, Primitive),
    component!("taskbar", "Taskbar", Environment, HostOwned),
    component!(
        "permission-prompt",
        "Permission prompt",
        Environment,
        HostOwned
    ),
    component!(
        "desktop-notification",
        "Desktop notification",
        Environment,
        HostOwned
    ),
    component!("window-frame", "Window frame", Visuals, HostOwned),
    component!("type-ramp", "Type ramp", Visuals, Pattern),
    component!("semantic-icons", "Semantic icons", Visuals, Pattern),
    component!("focus-indicator", "Focus indicator", Visuals, Pattern),
    component!("control-states", "Control states", Visuals, Pattern),
    component!("container", "Container", Layout, Primitive),
    component!("row-column", "Row and column", Layout, Primitive),
    component!("stack", "Stack", Layout, Primitive),
    component!("split-pane", "Split pane", Layout, Primitive),
    component!("scroll-view", "Scroll view", Layout, Primitive),
    component!("surface-levels", "Surface levels", Layout, Pattern),
    component!("web-surface", "Web surface", Layout, HostOwned),
    component!("terminal-surface", "Terminal surface", Layout, HostOwned),
    component!("host-surface", "Host surface", Layout, HostOwned),
];

#[must_use]
pub fn component_by_id(id: &str) -> Option<&'static ComponentSpec> {
    COMPONENT_CATALOG
        .iter()
        .find(|component| component.id == id)
}

pub fn components_in(category: ComponentCategory) -> impl Iterator<Item = &'static ComponentSpec> {
    COMPONENT_CATALOG
        .iter()
        .filter(move |component| component.category == category)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn component_ids_are_unique_and_resolve() {
        let ids = COMPONENT_CATALOG
            .iter()
            .map(|component| component.id)
            .collect::<BTreeSet<_>>();
        assert_eq!(ids.len(), COMPONENT_CATALOG.len());
        assert!(COMPONENT_CATALOG.len() >= 70);
        for component in COMPONENT_CATALOG {
            assert_eq!(component_by_id(component.id), Some(component));
            assert_eq!(component.standard, format!("#{}", component.id));
        }
    }

    #[test]
    fn every_visual_index_category_has_components() {
        for category in ComponentCategory::ALL {
            assert!(components_in(category).next().is_some(), "{category:?}");
        }
    }
}
