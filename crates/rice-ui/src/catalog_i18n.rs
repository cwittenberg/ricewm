//! Localized display vocabulary; canonical identifiers and SDK text stay literal.
use super::{ComponentCategory, ComponentImplementation, ComponentSpec};

impl ComponentSpec {
    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self.id {
            "label" => rice_i18n::t("gallery.component.label"),
            "button" => rice_i18n::t("gallery.component.button"),
            "command-link" => rice_i18n::t("gallery.component.command-link"),
            "split-button" => rice_i18n::t("gallery.component.split-button"),
            "toggle" => rice_i18n::t("gallery.component.toggle"),
            "check-box" => rice_i18n::t("gallery.component.check-box"),
            "radio-button" => rice_i18n::t("gallery.component.radio-button"),
            "drop-down-list" => rice_i18n::t("gallery.component.drop-down-list"),
            "combo-box" => rice_i18n::t("gallery.component.combo-box"),
            "link" => rice_i18n::t("editor.link"),
            "text-field" => rice_i18n::t("gallery.component.text-field"),
            "password-field" => rice_i18n::t("gallery.component.password-field"),
            "search-box" => rice_i18n::t("gallery.component.search-box"),
            "text-editor" => rice_i18n::t("icons.text-editor"),
            "spin-control" => rice_i18n::t("gallery.component.spin-control"),
            "slider" => rice_i18n::t("gallery.component.slider"),
            "scroll-bar" => rice_i18n::t("gallery.component.scroll-bar"),
            "progress-bar" => rice_i18n::t("gallery.component.progress-bar"),
            "activity-indicator" => rice_i18n::t("gallery.component.activity-indicator"),
            "group-box" => rice_i18n::t("gallery.component.group-box"),
            "image" => rice_i18n::t("settings.image"),
            "markdown" => rice_i18n::t("gallery.component.markdown"),
            "list-box" => rice_i18n::t("gallery.component.list-box"),
            "list-view" => rice_i18n::t("gallery.component.list-view"),
            "table" => rice_i18n::t("gallery.component.table"),
            "tree-view" => rice_i18n::t("gallery.component.tree-view"),
            "icon-grid" => rice_i18n::t("gallery.component.icon-grid"),
            "tabs" => rice_i18n::t("gallery.component.tabs"),
            "breadcrumb" => rice_i18n::t("gallery.component.breadcrumb"),
            "accordion" => rice_i18n::t("gallery.component.accordion"),
            "menu-bar" => rice_i18n::t("gallery.component.menu-bar"),
            "menu" => rice_i18n::t("icons.menu"),
            "submenu" => rice_i18n::t("gallery.component.submenu"),
            "context-menu" => rice_i18n::t("gallery.component.context-menu"),
            "tool-bar" => rice_i18n::t("gallery.component.tool-bar"),
            "status-bar" => rice_i18n::t("gallery.component.status-bar"),
            "ribbon" => rice_i18n::t("gallery.component.ribbon"),
            "tooltip" => rice_i18n::t("gallery.component.tooltip"),
            "infotip" => rice_i18n::t("gallery.component.infotip"),
            "balloon" => rice_i18n::t("gallery.component.balloon"),
            "notification" => rice_i18n::t("gallery.component.notification"),
            "in-place-message" => rice_i18n::t("gallery.component.in-place-message"),
            "error-message" => rice_i18n::t("gallery.component.error-message"),
            "warning-message" => rice_i18n::t("gallery.component.warning-message"),
            "progress-feedback" => rice_i18n::t("gallery.component.progress-feedback"),
            "empty-state" => rice_i18n::t("gallery.component.empty-state"),
            "pointer-feedback" => rice_i18n::t("gallery.component.pointer-feedback"),
            "dialog" => rice_i18n::t("window.dialog"),
            "confirmation-dialog" => rice_i18n::t("gallery.component.confirmation-dialog"),
            "warning-dialog" => rice_i18n::t("gallery.component.warning-dialog"),
            "error-dialog" => rice_i18n::t("gallery.component.error-dialog"),
            "property-window" => rice_i18n::t("gallery.component.property-window"),
            "settings-page" => rice_i18n::t("gallery.component.settings-page"),
            "open-dialog" => rice_i18n::t("gallery.component.open-dialog"),
            "save-dialog" => rice_i18n::t("gallery.component.save-dialog"),
            "popover" => rice_i18n::t("gallery.component.popover"),
            "taskbar" => rice_i18n::t("gallery.component.taskbar"),
            "permission-prompt" => rice_i18n::t("gallery.component.permission-prompt"),
            "desktop-notification" => rice_i18n::t("gallery.component.desktop-notification"),
            "window-frame" => rice_i18n::t("gallery.component.window-frame"),
            "type-ramp" => rice_i18n::t("gallery.component.type-ramp"),
            "semantic-icons" => rice_i18n::t("gallery.component.semantic-icons"),
            "focus-indicator" => rice_i18n::t("gallery.component.focus-indicator"),
            "control-states" => rice_i18n::t("gallery.component.control-states"),
            "container" => rice_i18n::t("gallery.component.container"),
            "row-column" => rice_i18n::t("gallery.component.row-column"),
            "stack" => rice_i18n::t("gallery.component.stack"),
            "split-pane" => rice_i18n::t("gallery.component.split-pane"),
            "scroll-view" => rice_i18n::t("gallery.component.scroll-view"),
            "surface-levels" => rice_i18n::t("gallery.component.surface-levels"),
            "web-surface" => rice_i18n::t("gallery.component.web-surface"),
            "terminal-surface" => rice_i18n::t("gallery.component.terminal-surface"),
            "host-surface" => rice_i18n::t("gallery.component.host-surface"),
            _ => self.name,
        }
    }
}

impl ComponentCategory {
    #[must_use]
    pub fn display_title(self) -> &'static str {
        match self {
            Self::Controls => rice_i18n::t("gallery.category.controls"),
            Self::Collections => rice_i18n::t("gallery.category.collections"),
            Self::Commands => rice_i18n::t("gallery.category.commands"),
            Self::Feedback => rice_i18n::t("gallery.category.feedback"),
            Self::Windows => rice_i18n::t("gallery.category.windows"),
            Self::Environment => rice_i18n::t("gallery.category.environment"),
            Self::Visuals => rice_i18n::t("gallery.category.visuals"),
            Self::Layout => rice_i18n::t("gallery.category.layout"),
        }
    }
}

impl ComponentImplementation {
    #[must_use]
    pub fn display_label(self) -> &'static str {
        match self {
            Self::Primitive => rice_i18n::t("gallery.implementation.primitive"),
            Self::Composite => rice_i18n::t("gallery.implementation.composite"),
            Self::Pattern => rice_i18n::t("gallery.implementation.pattern"),
            Self::HostOwned => rice_i18n::t("gallery.implementation.hostowned"),
            Self::NotAdopted => rice_i18n::t("gallery.implementation.notadopted"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::COMPONENT_CATALOG;

    #[test]
    fn english_display_names_resolve_without_changing_canonical_vocabulary() {
        rice_i18n::with_locale(0, || {
            for component in COMPONENT_CATALOG {
                assert_eq!(component.display_name(), component.name, "{}", component.id);
            }
            for category in ComponentCategory::ALL {
                assert_eq!(category.display_title(), category.title());
            }
            for (implementation, expected) in [
                (ComponentImplementation::Primitive, "Primitive"),
                (ComponentImplementation::Composite, "Composite"),
                (ComponentImplementation::Pattern, "Pattern"),
                (ComponentImplementation::HostOwned, "Host-owned"),
                (ComponentImplementation::NotAdopted, "Not adopted"),
            ] { assert_eq!(implementation.display_label(), expected); }
        });
        let custom = ComponentSpec { id: "custom-code-id", name: "Settings",
            category: ComponentCategory::Controls, implementation: ComponentImplementation::Pattern,
            standard: "#custom-code-id" };
        for &locale in rice_i18n::LOCALE_CODES {
            rice_i18n::with_locale(locale, || assert_eq!(custom.display_name(), "Settings"));
        }
    }
}
