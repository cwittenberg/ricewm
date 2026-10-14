//! Real `RiceFiles` operation and information dialogs.

use rice_ui::{IconId, Patch, WidgetKind, WidgetState};

use super::{
    files_properties::{self, PropertiesState},
    files_view::{ROOT, create, create_with_state, tail_elided_path},
};

// Keep modal controls outside every directory-row, tab, completion, detail, and
// open-with range. Directory rows deliberately start at 1_000.
pub(super) const DIALOG: u64 = 300_000;
pub(super) const FIELD: u64 = 300_001;
pub(super) const CONFIRM: u64 = 300_002;
pub(super) const CANCEL: u64 = 300_003;
pub(super) const PROPERTIES_OK: u64 = 300_006;
pub(super) const CONTENT: u64 = 300_004;
const ACTIONS: u64 = 300_005;
pub(super) const CONFLICT_SKIP: u64 = 300_020;
pub(super) const CONFLICT_REPLACE: u64 = 300_021;
pub(super) const CONFLICT_APPLY_ALL: u64 = 300_022;
const MESSAGE: u64 = 300_010;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Dialog {
    CreateFolder {
        value: String,
    },
    Rename {
        value: String,
    },
    MoveTo {
        value: String,
    },
    CopyTo {
        value: String,
    },
    DeletePermanently,
    Properties(PropertiesState),
    About {
        section: usize,
        scroll: u32,
    },
    Conflict {
        destination: String,
        rename: String,
        apply_all: bool,
    },
}

pub(super) fn patches(patches: &mut Vec<Patch>, dialog: &Dialog, redmond98: bool) {
    if let Dialog::About { section, scroll } = dialog {
        about_patches(patches, *section, *scroll);
        return;
    }
    let (title, icon) = title(dialog, redmond98);
    create(
        patches,
        DIALOG,
        Some(ROOT),
        WidgetKind::Dialog,
        Some(&title),
        Some(icon),
    );
    create(
        patches,
        CONTENT,
        Some(DIALOG),
        WidgetKind::Column,
        None,
        None,
    );
    create(
        patches,
        ACTIONS,
        Some(DIALOG),
        WidgetKind::Row,
        Some("panel:actions"),
        None,
    );
    match dialog {
        Dialog::CreateFolder { value } => operation(patches, value, "Create"),
        Dialog::Rename { value } => operation(patches, value, "Rename"),
        Dialog::MoveTo { value } => operation(patches, value, "Move"),
        Dialog::CopyTo { value } => operation(patches, value, "Copy"),
        Dialog::DeletePermanently => {
            create(
                patches,
                MESSAGE,
                Some(CONTENT),
                WidgetKind::Label,
                Some(rice_i18n::t("Permanently delete the selected items? This cannot be undone.")),
                Some(IconId::Warning),
            );
            button(patches, CONFIRM, "Delete Permanently", Some(IconId::Trash));
            button(patches, CANCEL, "Cancel", Some(IconId::Close));
        }
        Dialog::Properties(state) => {
            files_properties::patches(patches, state, redmond98);
            if redmond98 {
                button(patches, PROPERTIES_OK, "OK", None);
                button(patches, CANCEL, "Cancel", None);
                create_with_state(
                    patches,
                    files_properties::APPLY_MODE,
                    Some(ACTIONS),
                    WidgetKind::Button,
                    Some(rice_i18n::t("Apply")),
                    None,
                    if state.apply_enabled() {
                        WidgetState::ENABLED
                    } else {
                        WidgetState::empty()
                    },
                );
            } else {
                button(patches, CANCEL, "Close", Some(IconId::Close));
            }
        }
        Dialog::About { .. } => unreachable!(),
        Dialog::Conflict {
            destination,
            rename,
            apply_all,
        } => {
            let destination = tail_elided_path(destination, 48);
            create(
                patches,
                MESSAGE,
                Some(CONTENT),
                WidgetKind::Label,
                Some(&rice_i18n::format("“{destination}” already exists.", &[("destination", &destination)])),
                Some(IconId::Warning),
            );
            create(
                patches,
                FIELD,
                Some(CONTENT),
                WidgetKind::TextField,
                Some(rename),
                Some(IconId::EditRedo),
            );
            create_with_state(
                patches,
                CONFLICT_APPLY_ALL,
                Some(CONTENT),
                WidgetKind::CheckBox,
                Some(rice_i18n::t("Apply Skip or Replace to all remaining conflicts")),
                None,
                WidgetState::ENABLED
                    | if *apply_all {
                        WidgetState::CHECKED
                    } else {
                        WidgetState::empty()
                    },
            );
            button(patches, CONFIRM, "Rename", Some(IconId::EditRedo));
            button(patches, CONFLICT_REPLACE, "Replace", Some(IconId::Refresh));
            button(patches, CONFLICT_SKIP, "Skip", Some(IconId::GoForward));
            button(patches, CANCEL, "Cancel Operation", Some(IconId::Close));
        }
    }
}

fn operation(patches: &mut Vec<Patch>, value: &str, action: &str) {
    create(
        patches,
        FIELD,
        Some(CONTENT),
        WidgetKind::TextField,
        Some(value),
        Some(IconId::FolderOpen),
    );
    button(patches, CONFIRM, action, Some(IconId::GoForward));
    button(patches, CANCEL, "Cancel", Some(IconId::Close));
}

fn button(patches: &mut Vec<Patch>, id: u64, text: &str, icon: Option<IconId>) {
    create(
        patches,
        id,
        Some(ACTIONS),
        WidgetKind::Button,
        Some(rice_i18n::t(text)),
        icon,
    );
}

fn title(dialog: &Dialog, redmond98: bool) -> (String, IconId) {
    match dialog {
        Dialog::CreateFolder { .. } => (String::from(rice_i18n::t("New Folder")), IconId::Folder),
        Dialog::Rename { .. } => (String::from(rice_i18n::t("Rename Item")), IconId::EditRedo),
        Dialog::MoveTo { .. } => (String::from(rice_i18n::t("Move Items")), IconId::GoForward),
        Dialog::CopyTo { .. } => (String::from(rice_i18n::t("Copy Items")), IconId::EditCopy),
        Dialog::DeletePermanently => (String::from(rice_i18n::t("Delete Permanently")), IconId::Warning),
        Dialog::Properties(state) => (
            if redmond98 {
                state.redmond_title()
            } else {
                state.title()
            },
            IconId::Information,
        ),
        Dialog::About { .. } => (String::from(rice_i18n::t("About Files")), IconId::Folder),
        Dialog::Conflict { .. } => (String::from(rice_i18n::t("File Conflict")), IconId::Warning),
    }
}

pub(super) const ABOUT_NAV: u64 = 300_100;
pub(super) const ABOUT_SCROLL: u64 = 300_110;

fn about_patches(patches: &mut Vec<Patch>, section: usize, scroll: u32) {
    let enabled = WidgetState::ENABLED;
    for (id, parent, kind, text, icon, state) in [
        (
            DIALOG,
            ROOT,
            WidgetKind::Dialog,
            "About Files",
            Some(IconId::Folder),
            enabled | WidgetState::EXPANSIVE,
        ),
        (
            CONTENT,
            DIALOG,
            WidgetKind::SplitPane,
            "split:220:min-width=210:max-width=260",
            None,
            enabled,
        ),
        (
            300_101,
            CONTENT,
            WidgetKind::Column,
            "panel:welcome-navigation",
            None,
            enabled,
        ),
        (
            300_102,
            300_101,
            WidgetKind::List,
            "zoom:1150",
            None,
            enabled,
        ),
        (
            300_103,
            CONTENT,
            WidgetKind::Column,
            "panel:clamp:760",
            None,
            enabled,
        ),
        (
            300_104,
            300_103,
            WidgetKind::Image,
            "panel:height:120",
            Some(IconId::Folder),
            enabled | WidgetState::LARGE_ICON,
        ),
        (
            300_105,
            300_103,
            WidgetKind::Label,
            "Files",
            None,
            enabled | WidgetState::DISPLAY | WidgetState::CENTER_ALIGNED,
        ),
        (
            300_106,
            300_103,
            WidgetKind::Label,
            "Your files, at home in RiceWM.",
            None,
            enabled | WidgetState::MUTED | WidgetState::CENTER_ALIGNED,
        ),
        (
            ABOUT_SCROLL,
            300_103,
            WidgetKind::ScrollView,
            &format!("{scroll}/0"),
            None,
            enabled | WidgetState::EXPANSIVE,
        ),
        (
            300_111,
            ABOUT_SCROLL,
            WidgetKind::Column,
            "panel:welcome-document",
            None,
            enabled,
        ),
        (
            300_112,
            300_111,
            WidgetKind::Markdown,
            ABOUT_SECTIONS[section].1,
            None,
            enabled | WidgetState::START_ALIGNED,
        ),
        (
            ACTIONS,
            300_103,
            WidgetKind::Row,
            "panel:actions",
            None,
            enabled,
        ),
        (
            CANCEL,
            ACTIONS,
            WidgetKind::Button,
            "Close",
            Some(IconId::Close),
            enabled | WidgetState::ACCENT,
        ),
    ] {
        create_with_state(patches, id, Some(parent), kind, Some(if matches!(kind, WidgetKind::Label | WidgetKind::Dialog | WidgetKind::Button | WidgetKind::Markdown) { rice_i18n::t(text) } else { text }), icon, state);
    }
    for (index, (label, _)) in ABOUT_SECTIONS.iter().enumerate() {
        create_with_state(
            patches,
            ABOUT_NAV + 20 + index as u64,
            Some(300_102),
            WidgetKind::Button,
            Some(rice_i18n::t(label)),
            Some(if index == 0 {
                IconId::Folder
            } else {
                IconId::Information
            }),
            enabled
                | WidgetState::START_ALIGNED
                | if index == section {
                    WidgetState::SELECTED | WidgetState::ACCENT
                } else {
                    WidgetState::FLAT
                },
        );
    }
}

const ABOUT_SECTIONS: [(&str, &str); 3] = [
    (
        "Overview",
        "## Files\n\nThe native RiceWM file manager. Browse your home, organize folders, and open files in the applications you choose.\n\nBuilt with RiceWM’s native UI and virtual filesystem services.",
    ),
    (
        "What it can do",
        "## Make yourself at home\n\n- Browse with tabs, split panes, icon views, and detailed lists.\n- Keep favorite folders in Places; right-click a saved folder to remove it.\n- Search, preview images and media, and choose applications with Open With.\n- Create, rename, copy, move, and trash files.\n- Connect to local and remote storage supported by your session.",
    ),
    (
        "Credits & licenses",
        "## Open-source credits\n\nFiles is part of RiceWM, built by the RiceWM contributors and distributed under the MIT license.\n\n### Shared components\n\nRust and the Rust community; Serde and serde_json for serialization; FFmpeg for media decoding and previews. Third-party components retain their own licenses.\n\n### Bowl ad blocker\n\nBowl, RiceWM’s companion browser, uses **adblock-rust** by Brave Software and contributors (Mozilla Public License 2.0). Its network filters come from **EasyList**, by The EasyList authors (Creative Commons Attribution-ShareAlike 3.0).\n\nThe blocker runs in Bowl. Bundled license texts, dependency notices, and filter source attribution accompany its package.",
    ),
];

#[cfg(test)]
mod translation_tests {
    use super::*;
    #[test]
    fn about_pages_translate_prose_and_preserve_license_names_and_layout_tokens() {
        for locale in [1, 2] {
            rice_i18n::with_locale(locale, || {
                for section in 0..ABOUT_SECTIONS.len() {
                    let mut output = Vec::new();
                    about_patches(&mut output, section, 0);
                    let text = |id| output.iter().find_map(|patch| match patch {
                        Patch::Create {widget, ..} if widget.id.get() == id => widget.text.as_deref(),
                        _ => None,
                    }).unwrap();
                    assert_ne!(text(300_112), ABOUT_SECTIONS[section].1);
                    assert_eq!(text(CONTENT), "split:220:min-width=210:max-width=260");
                    if section == 2 {
                        for name in ["MIT", "Mozilla Public License 2.0", "Creative Commons Attribution-ShareAlike 3.0", "**adblock-rust**", "**EasyList**"] {
                            assert!(text(300_112).contains(name));
                        }
                    }
                }
            });
        }
    }

    #[test]
    fn conflict_dialog_translates_actions_without_changing_paths_or_rename_input() {
        rice_i18n::with_locale(2, || {
            let mut output = Vec::new();
            patches(&mut output, &Dialog::Conflict {
                destination: "/root/{destination}.txt".into(),
                rename: "Copy {name}.txt".into(), apply_all: false,
            }, false);
            let text = |id| output.iter().find_map(|patch| match patch {
                Patch::Create {widget, ..} if widget.id.get() == id => widget.text.as_deref(),
                _ => None,
            });
            assert_eq!(text(DIALOG), Some("文件冲突"));
            assert_eq!(text(MESSAGE), Some("“/root/{destination}.txt”已存在。"));
            assert_eq!(text(FIELD), Some("Copy {name}.txt"));
            assert_eq!(text(CANCEL), Some("取消操作"));
        });
    }
}
