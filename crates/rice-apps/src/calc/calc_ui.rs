use super::RiceCalc;
use rice_calc::Mode;
use rice_native_app_protocol::{AppId, Message, UiPatchEnvelope, WidgetId, WindowId};
use rice_ui::{Patch, PatchConversionError, WidgetKind, WidgetSpec, WidgetState};

const WINDOW: u64 = 1;
const ROOT: u64 = 1;
pub(super) const MENU_BAR: u64 = 2;
pub(super) const MODE_MENU: u64 = 3;
pub(super) const EDIT_MENU: u64 = 4;
pub(super) const VIEW_MENU: u64 = 5;
pub(super) const HELP_MENU: u64 = 6;
pub(super) const MODE_BASIC: u64 = 10;
pub(super) const MODE_ADVANCED: u64 = 11;
pub(super) const MODE_PROGRAMMER: u64 = 12;
pub(super) const MODE_FINANCIAL: u64 = 53;
pub(super) const MODE_CONVERSION: u64 = 54;
pub(super) const COPY: u64 = 13;
pub(super) const TOGGLE_HISTORY: u64 = 14;
pub(super) const CLEAR_HISTORY: u64 = 15;
pub(super) const ANGLE_DEGREES: u64 = 16;
pub(super) const ANGLE_RADIANS: u64 = 17;
pub(super) const BASE_BINARY: u64 = 18;
pub(super) const BASE_OCTAL: u64 = 19;
pub(super) const BASE_DECIMAL: u64 = 20;
pub(super) const BASE_HEX: u64 = 21;
pub(super) const EDIT_CLEAR: u64 = 27;
pub(super) const EDIT_BACKSPACE: u64 = 28;
pub(super) const RANDOM: u64 = 29;
pub(super) const SHORTCUTS: u64 = 25;
pub(super) const ABOUT: u64 = 26;
pub(super) const EXPRESSION: u64 = 30;
const RESULT: u64 = 31;
const BODY: u64 = 32;
pub(super) const KEYPAD: u64 = 33;
const HISTORY: u64 = 34;
const DISPLAY: u64 = 35;
const HISTORY_HEADING: u64 = 36;
pub(super) const CLASSIC_MEMORY_INDICATOR: u64 = 57;
const CLASSIC_CLEAR_ROW: u64 = 58;
pub(super) const MEMORY_CLEAR: u64 = 40;
pub(super) const MEMORY_RECALL: u64 = 41;
pub(super) const MEMORY_STORE: u64 = 42;
pub(super) const MEMORY_ADD: u64 = 43;
pub(super) const MEMORY_SUBTRACT: u64 = 44;
pub(super) const CLEAR: u64 = 45;
pub(super) const BACKSPACE: u64 = 46;
pub(super) const EQUALS: u64 = 47;
pub(super) const DIALOG_CLOSE: u64 = 48;
pub(super) const PLUS_MINUS: u64 = 49;
pub(super) const RECIPROCAL: u64 = 50;
pub(super) const CLASSIC_CLEAR: u64 = 51;
pub(super) const FINANCIAL_PRINCIPAL: u64 = 310;
pub(super) const FINANCIAL_RATE: u64 = 311;
pub(super) const FINANCIAL_PERIODS: u64 = 312;
pub(super) const FINANCIAL_SALVAGE: u64 = 313;
pub(super) const FINANCIAL_FINAL_VALUE: u64 = 314;
pub(super) const FINANCIAL_FUTURE: u64 = 320;
pub(super) const FINANCIAL_PRESENT: u64 = 321;
pub(super) const FINANCIAL_PAYMENT: u64 = 322;
pub(super) const FINANCIAL_STRAIGHT_LINE: u64 = 323;
pub(super) const FINANCIAL_DOUBLE_DECLINING: u64 = 324;
pub(super) const FINANCIAL_TERM: u64 = 325;
pub(super) const FINANCIAL_GROSS_PROFIT: u64 = 326;
pub(super) const FINANCIAL_PERIODIC_RATE: u64 = 327;
pub(super) const FINANCIAL_SUM_OF_YEARS: u64 = 328;
pub(super) const UNIT_AMOUNT: u64 = 330;
pub(super) const UNIT_FROM: u64 = 331;
pub(super) const UNIT_TO: u64 = 332;
pub(super) const UNIT_CONVERT: u64 = 333;
pub(super) const COMPLEX_A_REAL: u64 = 340;
pub(super) const COMPLEX_A_IMAGINARY: u64 = 341;
pub(super) const COMPLEX_B_REAL: u64 = 342;
pub(super) const COMPLEX_B_IMAGINARY: u64 = 343;
pub(super) const COMPLEX_ADD: u64 = 344;
pub(super) const COMPLEX_SUBTRACT: u64 = 345;
pub(super) const COMPLEX_MULTIPLY: u64 = 346;
pub(super) const COMPLEX_DIVIDE: u64 = 347;
pub(super) const COMPLEX_CONJUGATE: u64 = 348;
pub(super) const CHARACTER: u64 = 350;
pub(super) const CHARACTER_CONVERT: u64 = 351;
pub(super) const WIDTH_8: u64 = 352;
pub(super) const WIDTH_16: u64 = 353;
pub(super) const WIDTH_32: u64 = 354;
pub(super) const WIDTH_64: u64 = 355;
pub(super) const WIDTH_128: u64 = 356;
pub(super) const FACTOR: u64 = 357;
pub(super) const TWOS_COMPLEMENT: u64 = 358;
pub(super) const CURRENCY_AMOUNT: u64 = 360;
pub(super) const CURRENCY_BASE: u64 = 361;
pub(super) const CURRENCY_QUOTE: u64 = 362;
pub(super) const CURRENCY_CONVERT: u64 = 363;
const INPUT_BASE: u64 = 100;
const HISTORY_BASE: u64 = 500;

const BASIC_ROWS: &[&[(&str, &str)]] = &[
    &[("(", "("), (")", ")"), ("%", "%"), ("÷", "/")],
    &[("7", "7"), ("8", "8"), ("9", "9"), ("×", "*")],
    &[("4", "4"), ("5", "5"), ("6", "6"), ("−", "-")],
    &[("1", "1"), ("2", "2"), ("3", "3"), ("+", "+")],
    &[("0", "0"), (".", "."), ("π", "pi"), ("=", "=")],
];
const ADVANCED_ROW_ONE: &[(&str, &str)] = &[
    ("sin", "sin("),
    ("cos", "cos("),
    ("tan", "tan("),
    ("√", "sqrt("),
    ("ln", "ln("),
];
const ADVANCED_ROW_TWO: &[(&str, &str)] = &[("log", "log("), ("x!", "!"), ("xʸ", "^"), ("e", "e")];
const PROGRAMMER_ROW: &[(&str, &str)] = &[
    ("AND", " and "),
    ("OR", " or "),
    ("XOR", " xor "),
    ("<<", " << "),
    (">>", " >> "),
    ("mod", " mod "),
];

pub(super) fn full_patches(actor: &RiceCalc) -> Vec<Patch> {
    let mut patches = vec![Patch::Clear];
    create(
        &mut patches,
        ROOT,
        None,
        WidgetKind::Column,
        Some("panel:clamp:520"),
        enabled(),
    );
    super::menus::build(&mut patches, actor);
    if actor.redmond98() {
        classic_calculator(&mut patches, actor);
        super::panels::dialog(&mut patches, actor);
        return patches;
    }
    create(
        &mut patches,
        DISPLAY,
        Some(ROOT),
        WidgetKind::Container,
        Some("panel:fixed"),
        enabled(),
    );
    create(
        &mut patches,
        EXPRESSION,
        Some(DISPLAY),
        WidgetKind::TextField,
        Some(&expression_text(actor)),
        expression_state(actor),
    );
    if !actor.calculator().expression().is_empty() {
        create(
            &mut patches,
            RESULT,
            Some(DISPLAY),
            WidgetKind::Label,
            Some(&result_text(actor)),
            result_state(actor),
        );
    }
    create(
        &mut patches,
        BODY,
        Some(ROOT),
        WidgetKind::SplitPane,
        Some("split:700"),
        enabled(),
    );
    create(
        &mut patches,
        KEYPAD,
        Some(BODY),
        WidgetKind::Column,
        (actor.calculator().mode() != Mode::Programmer).then_some("panel:distribute"),
        enabled(),
    );
    match actor.calculator().mode() {
        Mode::Financial => super::panels::financial(&mut patches, actor),
        Mode::Conversion => super::panels::conversion(&mut patches, actor),
        mode => {
            memory_row(&mut patches);
            if mode == Mode::Advanced {
                button_row(&mut patches, 70, ADVANCED_ROW_ONE);
                button_row(&mut patches, 72, ADVANCED_ROW_TWO);
                if actor.entropy_available() {
                    create(
                        &mut patches,
                        RANDOM,
                        Some(72),
                        WidgetKind::Button,
                        Some(rice_i18n::t("Random")),
                        enabled(),
                    );
                }
            }
            if mode == Mode::Programmer {
                button_row(&mut patches, 71, PROGRAMMER_ROW);
                super::panels::programmer(&mut patches, actor);
            }
            for (index, row) in BASIC_ROWS.iter().enumerate() {
                button_row(&mut patches, 80 + u64::try_from(index).unwrap_or(0), row);
            }
        }
    }
    if actor.history_visible() {
        history(&mut patches, actor);
    }
    super::panels::dialog(&mut patches, actor);
    patches
}

fn classic_calculator(patches: &mut Vec<Patch>, actor: &RiceCalc) {
    let display_text = if actor.calculator().expression().is_empty() {
        actor.calculator().display()
    } else {
        actor.calculator().expression().to_owned()
    };
    create(
        patches,
        DISPLAY,
        Some(ROOT),
        WidgetKind::Container,
        Some("panel:fixed-height:34:redmond98-calculator-display"),
        enabled(),
    );
    create(
        patches,
        EXPRESSION,
        Some(DISPLAY),
        WidgetKind::TextField,
        Some(&display_text),
        enabled() | WidgetState::FOCUSED | WidgetState::END_ALIGNED,
    );
    create(
        patches,
        KEYPAD,
        Some(ROOT),
        WidgetKind::Column,
        Some("panel:redmond98-calculator-keypad"),
        enabled(),
    );
    create(
        patches,
        59,
        Some(KEYPAD),
        WidgetKind::Row,
        Some("split:160"),
        enabled(),
    );
    create(
        patches,
        CLASSIC_MEMORY_INDICATOR,
        Some(59),
        WidgetKind::Container,
        Some("panel:redmond98-calculator-memory-indicator"),
        enabled(),
    );
    create(
        patches,
        CLASSIC_CLEAR_ROW,
        Some(59),
        WidgetKind::Row,
        Some("panel:equal"),
        enabled(),
    );
    for (id, label) in [
        (BACKSPACE, "Backspace"),
        (CLEAR, "CE"),
        (CLASSIC_CLEAR, "C"),
    ] {
        create(
            patches,
            id,
            Some(CLASSIC_CLEAR_ROW),
            WidgetKind::Button,
            Some(label),
            enabled() | WidgetState::DESTRUCTIVE,
        );
    }
    for (row, entries) in [
        [
            (MEMORY_CLEAR, "MC"),
            (input_id("7"), "7"),
            (input_id("8"), "8"),
            (input_id("9"), "9"),
            (input_id("/"), "/"),
            (input_id("sqrt("), "sqrt"),
        ],
        [
            (MEMORY_RECALL, "MR"),
            (input_id("4"), "4"),
            (input_id("5"), "5"),
            (input_id("6"), "6"),
            (input_id("*"), "*"),
            (input_id("%"), "%"),
        ],
        [
            (MEMORY_STORE, "MS"),
            (input_id("1"), "1"),
            (input_id("2"), "2"),
            (input_id("3"), "3"),
            (input_id("-"), "-"),
            (RECIPROCAL, "1/x"),
        ],
        [
            (MEMORY_ADD, "M+"),
            (input_id("0"), "0"),
            (PLUS_MINUS, "+/-"),
            (input_id("."), "."),
            (input_id("+"), "+"),
            (EQUALS, "="),
        ],
    ]
    .into_iter()
    .enumerate()
    {
        let row_id = 80 + u64::try_from(row).unwrap_or_default();
        create(
            patches,
            row_id,
            Some(KEYPAD),
            WidgetKind::Row,
            Some("panel:equal-fixed"),
            enabled(),
        );
        for (id, label) in entries {
            let state = if matches!(id, MEMORY_CLEAR | MEMORY_RECALL | MEMORY_STORE | MEMORY_ADD) {
                enabled() | WidgetState::DESTRUCTIVE
            } else {
                enabled()
            };
            create(
                patches,
                id,
                Some(row_id),
                WidgetKind::Button,
                Some(label),
                state,
            );
        }
    }
}

fn result_text(actor: &RiceCalc) -> String {
    if actor.calculator().mode() == Mode::Programmer && actor.calculator().error().is_none() {
        actor.calculator().programmer_display()
    } else {
        actor.calculator().display()
    }
}

fn expression_text(actor: &RiceCalc) -> String {
    if actor.calculator().expression().is_empty() {
        String::new()
    } else {
        actor.calculator().expression().to_owned()
    }
}

fn expression_state(actor: &RiceCalc) -> WidgetState {
    if actor.calculator().expression().is_empty() {
        enabled()
            | WidgetState::FOCUSED
            | WidgetState::DISPLAY
            | WidgetState::END_ALIGNED
            | WidgetState::FLAT
    } else {
        enabled()
            | WidgetState::FOCUSED
            | WidgetState::END_ALIGNED
            | WidgetState::FLAT
            | WidgetState::MUTED
    }
}

fn result_state(actor: &RiceCalc) -> WidgetState {
    let mut state = enabled() | WidgetState::DISPLAY | WidgetState::END_ALIGNED;
    if actor.calculator().error().is_some() {
        state = state | WidgetState::DESTRUCTIVE;
    }
    state
}

fn memory_row(patches: &mut Vec<Patch>) {
    create(
        patches,
        60,
        Some(KEYPAD),
        WidgetKind::Row,
        Some("panel:equal-fixed"),
        enabled(),
    );
    for (id, label, state) in [
        (MEMORY_CLEAR, "MC", enabled()),
        (MEMORY_RECALL, "MR", enabled()),
        (MEMORY_STORE, "MS", enabled()),
        (MEMORY_ADD, "M+", enabled()),
        (MEMORY_SUBTRACT, "M−", enabled()),
        (CLEAR, "C", enabled() | WidgetState::DESTRUCTIVE),
        (BACKSPACE, "←", enabled()),
    ] {
        create(
            patches,
            id,
            Some(60),
            WidgetKind::Button,
            Some(label),
            state,
        );
    }
}

fn button_row(patches: &mut Vec<Patch>, row_id: u64, entries: &[(&str, &str)]) {
    create(
        patches,
        row_id,
        Some(KEYPAD),
        WidgetKind::Row,
        Some("panel:equal"),
        enabled(),
    );
    for (label, value) in entries {
        let id = if *value == "=" {
            EQUALS
        } else {
            input_id(value)
        };
        let state = if id == EQUALS {
            enabled() | WidgetState::ACCENT
        } else if matches!(*value, "/" | "*" | "-" | "+" | "%" | "^") {
            enabled() | WidgetState::EMPHASIZED
        } else {
            enabled()
        };
        create(
            patches,
            id,
            Some(row_id),
            WidgetKind::Button,
            Some(label),
            state,
        );
    }
}

fn history(patches: &mut Vec<Patch>, actor: &RiceCalc) {
    create(
        patches,
        HISTORY,
        Some(BODY),
        WidgetKind::ScrollView,
        Some("panel:history"),
        enabled(),
    );
    create(
        patches,
        HISTORY_HEADING,
        Some(HISTORY),
        WidgetKind::Label,
        Some(rice_i18n::t("History")),
        enabled() | WidgetState::EMPHASIZED,
    );
    if actor.calculator().history().is_empty() {
        create(
            patches,
            HISTORY_BASE,
            Some(HISTORY),
            WidgetKind::Label,
            Some("No calculations yet"),
            enabled() | WidgetState::MUTED,
        );
    }
    for (index, (expression, result)) in actor.calculator().history().iter().enumerate() {
        create(
            patches,
            HISTORY_BASE + u64::try_from(index).unwrap_or(0),
            Some(HISTORY),
            WidgetKind::Button,
            Some(&format!("{expression}\n= {result}")),
            enabled(),
        );
    }
}

pub(super) fn input_for(id: u64) -> Option<&'static str> {
    BASIC_ROWS
        .iter()
        .flat_map(|row| row.iter())
        .chain(ADVANCED_ROW_ONE.iter())
        .chain(ADVANCED_ROW_TWO.iter())
        .chain(PROGRAMMER_ROW.iter())
        .find_map(|(_, value)| (input_id(value) == id).then_some(*value))
}

pub(super) fn history_index(id: u64) -> Option<usize> {
    id.checked_sub(HISTORY_BASE)
        .and_then(|value| value.try_into().ok())
}

pub(super) fn input_id(value: &str) -> u64 {
    INPUT_BASE
        + BASIC_ROWS
            .iter()
            .flat_map(|row| row.iter())
            .chain(ADVANCED_ROW_ONE.iter())
            .chain(ADVANCED_ROW_TWO.iter())
            .chain(PROGRAMMER_ROW.iter())
            .position(|(_, candidate)| *candidate == value)
            .and_then(|index| u64::try_from(index).ok())
            .expect("RiceCalc input values come from the bounded keypad catalog")
}

pub(super) fn create(
    patches: &mut Vec<Patch>,
    id: u64,
    parent: Option<u64>,
    kind: WidgetKind,
    text: Option<&str>,
    state: WidgetState,
) {
    let parent = parent.map(widget_id);
    let index = patches
        .iter()
        .filter(|patch| {
            matches!(
                patch,
                Patch::Create { parent: candidate, .. } if *candidate == parent
            )
        })
        .count();
    patches.push(Patch::Create {
        widget: WidgetSpec {
            id: widget_id(id),
            kind,
            state,
            text: text.map(str::to_owned),
            icon: None,
        },
        parent,
        index,
    });
}

pub(super) const fn enabled() -> WidgetState {
    WidgetState::ENABLED
}

pub(super) fn message(
    app: AppId,
    window: WindowId,
    sequence: u64,
    patches: &[Patch],
) -> Result<Message, PatchConversionError> {
    Ok(Message::UiPatch(UiPatchEnvelope {
        app,
        window,
        sequence,
        patches: patches
            .iter()
            .map(Patch::to_protocol)
            .collect::<Result<_, _>>()?,
    }))
}

fn widget_id(value: u64) -> WidgetId {
    WidgetId::new(value).expect("RiceCalc widget IDs are nonzero")
}

pub(super) fn window_id() -> WindowId {
    WindowId::new(WINDOW).expect("RiceCalc window ID is nonzero")
}
