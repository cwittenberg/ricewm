//! Data-only menu row conventions shared by every native application.

pub const MENU_SEPARATOR: &str = "menu:separator";
const SHORTCUT_SEPARATOR: char = '\t';

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MenuText<'a> {
    pub label: &'a str,
    pub shortcut: Option<&'a str>,
    pub separator: bool,
}

impl<'a> MenuText<'a> {
    #[must_use]
    pub fn parse(text: &'a str) -> Self {
        if text == MENU_SEPARATOR {
            return Self {
                label: "",
                shortcut: None,
                separator: true,
            };
        }
        let (label, shortcut) = text
            .split_once(SHORTCUT_SEPARATOR)
            .map_or((text, None), |(label, shortcut)| {
                (label, (!shortcut.is_empty()).then_some(shortcut))
            });
        Self {
            label,
            shortcut,
            separator: false,
        }
    }
}

#[must_use]
pub fn menu_item(label: &str, shortcut: Option<&str>) -> String {
    shortcut.map_or_else(
        || label.to_owned(),
        |shortcut| format!("{label}{SHORTCUT_SEPARATOR}{shortcut}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_rows_roundtrip_labels_shortcuts_and_separators() {
        assert_eq!(
            MenuText::parse(&menu_item("New Window", Some("Ctrl+N"))),
            MenuText {
                label: "New Window",
                shortcut: Some("Ctrl+N"),
                separator: false,
            }
        );
        assert!(MenuText::parse(MENU_SEPARATOR).separator);
        assert_eq!(MenuText::parse("Preferences").shortcut, None);
    }
}
