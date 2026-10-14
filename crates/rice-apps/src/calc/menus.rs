use super::{
    RiceCalc,
    calc_ui::{
        ABOUT, ANGLE_DEGREES, ANGLE_RADIANS, BASE_BINARY, BASE_DECIMAL, BASE_HEX, BASE_OCTAL,
        CLEAR_HISTORY, COPY, EDIT_BACKSPACE, EDIT_CLEAR, EDIT_MENU, HELP_MENU, MENU_BAR,
        MODE_ADVANCED, MODE_BASIC, MODE_CONVERSION, MODE_FINANCIAL, MODE_MENU, MODE_PROGRAMMER,
        SHORTCUTS, TOGGLE_HISTORY, VIEW_MENU, create, enabled,
    },
};
use rice_calc::{AngleUnit, IntegerBase, Mode};
use rice_ui::{MENU_SEPARATOR, Patch, WidgetKind, WidgetState, menu_item};

pub(super) fn build(patches: &mut Vec<Patch>, actor: &RiceCalc) {
    create(
        patches,
        MENU_BAR,
        Some(1),
        WidgetKind::MenuBar,
        None,
        enabled(),
    );
    let headings = if actor.redmond98() {
        vec![
            (EDIT_MENU, "Edit"),
            (VIEW_MENU, "View"),
            (HELP_MENU, "Help"),
        ]
    } else {
        vec![
            (MODE_MENU, "Mode"),
            (EDIT_MENU, "Edit"),
            (VIEW_MENU, "View"),
            (HELP_MENU, "Help"),
        ]
    };
    for (id, label) in headings {
        create(
            patches,
            id,
            Some(MENU_BAR),
            WidgetKind::Menu,
            Some(label),
            enabled(),
        );
    }
    if !actor.redmond98() {
        radio_menu(
            patches,
            MODE_MENU,
            &[
                (
                    MODE_BASIC,
                    "Basic",
                    actor.calculator().mode() == Mode::Basic,
                ),
                (
                    MODE_ADVANCED,
                    "Advanced",
                    actor.calculator().mode() == Mode::Advanced,
                ),
                (
                    MODE_PROGRAMMER,
                    "Programmer",
                    actor.calculator().mode() == Mode::Programmer,
                ),
                (
                    MODE_FINANCIAL,
                    "Financial",
                    actor.calculator().mode() == Mode::Financial,
                ),
                (
                    MODE_CONVERSION,
                    "Conversion",
                    actor.calculator().mode() == Mode::Conversion,
                ),
            ],
        );
        separator(patches, 22, MODE_MENU);
    }
    menu_row(patches, COPY, EDIT_MENU, "Copy Result", Some("Alt+C"));
    menu_row(patches, EDIT_CLEAR, EDIT_MENU, "Clear", Some("Escape"));
    menu_row(
        patches,
        EDIT_BACKSPACE,
        EDIT_MENU,
        "Backspace",
        Some("Backspace"),
    );
    menu_row(
        patches,
        TOGGLE_HISTORY,
        VIEW_MENU,
        if actor.history_visible() {
            "Hide History"
        } else {
            "Show History"
        },
        Some("Alt+H"),
    );
    menu_row(patches, CLEAR_HISTORY, VIEW_MENU, "Clear History", None);
    mode_view_options(patches, actor);
    menu_row(
        patches,
        SHORTCUTS,
        HELP_MENU,
        "Keyboard Shortcuts",
        Some("Alt+?"),
    );
    menu_row(patches, ABOUT, HELP_MENU, "About RiceCalc", None);
}

fn mode_view_options(patches: &mut Vec<Patch>, actor: &RiceCalc) {
    if actor.calculator().mode() == Mode::Advanced {
        separator(patches, 23, VIEW_MENU);
        radio_menu(
            patches,
            VIEW_MENU,
            &[
                (
                    ANGLE_DEGREES,
                    "Degrees",
                    actor.calculator().angle_unit() == AngleUnit::Degrees,
                ),
                (
                    ANGLE_RADIANS,
                    "Radians",
                    actor.calculator().angle_unit() == AngleUnit::Radians,
                ),
            ],
        );
    }
    if actor.calculator().mode() == Mode::Programmer {
        separator(patches, 24, VIEW_MENU);
        radio_menu(
            patches,
            VIEW_MENU,
            &[
                (
                    BASE_BINARY,
                    "Binary",
                    actor.calculator().integer_base() == IntegerBase::Binary,
                ),
                (
                    BASE_OCTAL,
                    "Octal",
                    actor.calculator().integer_base() == IntegerBase::Octal,
                ),
                (
                    BASE_DECIMAL,
                    "Decimal",
                    actor.calculator().integer_base() == IntegerBase::Decimal,
                ),
                (
                    BASE_HEX,
                    "Hexadecimal",
                    actor.calculator().integer_base() == IntegerBase::Hexadecimal,
                ),
            ],
        );
    }
}

fn radio_menu(patches: &mut Vec<Patch>, parent: u64, rows: &[(u64, &str, bool)]) {
    for (id, label, checked) in rows {
        create(
            patches,
            *id,
            Some(parent),
            WidgetKind::RadioButton,
            Some(label),
            if *checked {
                enabled() | WidgetState::CHECKED
            } else {
                enabled()
            },
        );
    }
}

fn menu_row(patches: &mut Vec<Patch>, id: u64, parent: u64, label: &str, shortcut: Option<&str>) {
    create(
        patches,
        id,
        Some(parent),
        WidgetKind::MenuItem,
        Some(&menu_item(label, shortcut)),
        enabled(),
    );
}

fn separator(patches: &mut Vec<Patch>, id: u64, parent: u64) {
    create(
        patches,
        id,
        Some(parent),
        WidgetKind::MenuItem,
        Some(MENU_SEPARATOR),
        WidgetState::empty(),
    );
}
