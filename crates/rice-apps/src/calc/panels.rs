use super::{
    DialogKind, RiceCalc,
    calc_ui::{
        CHARACTER, CHARACTER_CONVERT, COMPLEX_A_IMAGINARY, COMPLEX_A_REAL, COMPLEX_ADD,
        COMPLEX_B_IMAGINARY, COMPLEX_B_REAL, COMPLEX_CONJUGATE, COMPLEX_DIVIDE, COMPLEX_MULTIPLY,
        COMPLEX_SUBTRACT, CURRENCY_AMOUNT, CURRENCY_BASE, CURRENCY_CONVERT, CURRENCY_QUOTE,
        DIALOG_CLOSE, FACTOR, FINANCIAL_DOUBLE_DECLINING, FINANCIAL_FINAL_VALUE, FINANCIAL_FUTURE,
        FINANCIAL_GROSS_PROFIT, FINANCIAL_PAYMENT, FINANCIAL_PERIODIC_RATE, FINANCIAL_PERIODS,
        FINANCIAL_PRESENT, FINANCIAL_PRINCIPAL, FINANCIAL_RATE, FINANCIAL_SALVAGE,
        FINANCIAL_STRAIGHT_LINE, FINANCIAL_SUM_OF_YEARS, FINANCIAL_TERM, KEYPAD, TWOS_COMPLEMENT,
        UNIT_AMOUNT, UNIT_CONVERT, UNIT_FROM, UNIT_TO, WIDTH_8, WIDTH_16, WIDTH_32, WIDTH_64,
        WIDTH_128, create, enabled,
    },
    workbench::{CATEGORY_BASE, CATEGORY_KEYS},
};
use rice_ui::{Patch, WidgetKind, WidgetState};

const FINANCIAL_PANEL: u64 = 300;
const FINANCIAL_ACTIONS: u64 = 301;
const CONVERSION_PANEL: u64 = 302;
const UNIT_ACTIONS: u64 = 303;
const CURRENCY_ACTIONS: u64 = 304;
pub(super) const PROGRAMMER_PANEL: u64 = 305;
const PROGRAMMER_CONTENT: u64 = 368;
const COMPLEX_ACTIONS: u64 = 306;
const WIDTH_ACTIONS: u64 = 307;
const WORKBENCH_OUTPUT: u64 = 308;
const FINANCIAL_HEADING: u64 = 370;
const CONVERSION_HEADING: u64 = 371;
const PROGRAMMER_HEADING: u64 = 372;
const CURRENCY_HEADING: u64 = 373;
const DIALOG_TITLE: u64 = 389;
const DIALOG: u64 = 390;
const DIALOG_CONTENT: u64 = 391;
const DIALOG_ACTIONS: u64 = 392;

pub(super) fn financial(patches: &mut Vec<Patch>, actor: &RiceCalc) {
    let values = &actor.workbench().financial;
    create(
        patches,
        FINANCIAL_PANEL,
        Some(KEYPAD),
        WidgetKind::Column,
        Some("panel:worksheet"),
        enabled(),
    );
    heading(patches, FINANCIAL_PANEL, FINANCIAL_HEADING, "Financial");
    field(
        patches,
        FINANCIAL_PANEL,
        410,
        FINANCIAL_PRINCIPAL,
        "Principal / cost",
        &values[0],
    );
    field(
        patches,
        FINANCIAL_PANEL,
        411,
        FINANCIAL_RATE,
        "Periodic interest rate",
        &values[1],
    );
    field(
        patches,
        FINANCIAL_PANEL,
        412,
        FINANCIAL_PERIODS,
        "Number of periods",
        &values[2],
    );
    field(
        patches,
        FINANCIAL_PANEL,
        413,
        FINANCIAL_SALVAGE,
        "Salvage value",
        &values[3],
    );
    field(
        patches,
        FINANCIAL_PANEL,
        414,
        FINANCIAL_FINAL_VALUE,
        "Future value / current period",
        &values[4],
    );
    button_group(
        patches,
        FINANCIAL_PANEL,
        FINANCIAL_ACTIONS,
        &[
            (FINANCIAL_FUTURE, "Future Value"),
            (FINANCIAL_PRESENT, "Present Value"),
            (FINANCIAL_PAYMENT, "Periodic Payment"),
        ],
    );
    button_group(
        patches,
        FINANCIAL_PANEL,
        315,
        &[
            (FINANCIAL_STRAIGHT_LINE, "Straight Line"),
            (FINANCIAL_DOUBLE_DECLINING, "Double Declining"),
            (FINANCIAL_SUM_OF_YEARS, "Sum of Years"),
        ],
    );
    button_group(
        patches,
        FINANCIAL_PANEL,
        316,
        &[
            (FINANCIAL_TERM, "Term"),
            (FINANCIAL_GROSS_PROFIT, "Gross Profit"),
            (FINANCIAL_PERIODIC_RATE, "Periodic Rate"),
        ],
    );
    output(patches, FINANCIAL_PANEL, actor.workbench().display_output().as_str());
}

pub(super) fn conversion(patches: &mut Vec<Patch>, actor: &RiceCalc) {
    let values = &actor.workbench().unit;
    create(
        patches,
        CONVERSION_PANEL,
        Some(KEYPAD),
        WidgetKind::Column,
        Some("panel:worksheet"),
        enabled(),
    );
    heading(patches, CONVERSION_PANEL, CONVERSION_HEADING, "Convert");
    category_group(
        patches,
        CONVERSION_PANEL,
        317,
        &CATEGORY_KEYS[..6],
        0,
        actor,
    );
    category_group(
        patches,
        CONVERSION_PANEL,
        318,
        &CATEGORY_KEYS[6..],
        6,
        actor,
    );
    field(
        patches,
        CONVERSION_PANEL,
        420,
        UNIT_AMOUNT,
        "Amount",
        &values[0],
    );
    field(
        patches,
        CONVERSION_PANEL,
        421,
        UNIT_FROM,
        "From unit",
        &values[1],
    );
    field(
        patches,
        CONVERSION_PANEL,
        422,
        UNIT_TO,
        "To unit",
        &values[2],
    );
    button_group(
        patches,
        CONVERSION_PANEL,
        UNIT_ACTIONS,
        &[(UNIT_CONVERT, "Convert units")],
    );

    let (amount, base, quote, status) = actor.currency_fields();
    if actor.currency_available() {
        heading(patches, CONVERSION_PANEL, CURRENCY_HEADING, "Currency");
        field(
            patches,
            CONVERSION_PANEL,
            424,
            CURRENCY_AMOUNT,
            "Amount",
            amount,
        );
        field(
            patches,
            CONVERSION_PANEL,
            425,
            CURRENCY_BASE,
            "From ISO 4217",
            base,
        );
        field(
            patches,
            CONVERSION_PANEL,
            426,
            CURRENCY_QUOTE,
            "To ISO 4217",
            quote,
        );
        button_group(
            patches,
            CONVERSION_PANEL,
            CURRENCY_ACTIONS,
            &[(CURRENCY_CONVERT, "Convert currency")],
        );
        if !status.is_empty() {
            create(
                patches,
                427,
                Some(CONVERSION_PANEL),
                WidgetKind::Label,
                Some(status),
                enabled(),
            );
        }
    }
    output(patches, CONVERSION_PANEL, actor.workbench().display_output().as_str());
}

pub(super) fn programmer(patches: &mut Vec<Patch>, actor: &RiceCalc) {
    create(
        patches,
        PROGRAMMER_PANEL,
        Some(KEYPAD),
        WidgetKind::ScrollView,
        Some(&format!("{}/1000", actor.programmer_scroll())),
        enabled(),
    );
    create(
        patches,
        PROGRAMMER_CONTENT,
        Some(PROGRAMMER_PANEL),
        WidgetKind::Column,
        Some("panel:worksheet"),
        enabled(),
    );
    heading(
        patches,
        PROGRAMMER_CONTENT,
        PROGRAMMER_HEADING,
        "Programmer tools",
    );
    button_group(
        patches,
        PROGRAMMER_CONTENT,
        WIDTH_ACTIONS,
        &[
            (WIDTH_8, "8-bit"),
            (WIDTH_16, "16-bit"),
            (WIDTH_32, "32-bit"),
            (WIDTH_64, "64-bit"),
            (WIDTH_128, "128-bit"),
        ],
    );
    button_group(
        patches,
        PROGRAMMER_CONTENT,
        309,
        &[
            (TWOS_COMPLEMENT, "Two’s-complement representation"),
            (FACTOR, "Prime factors"),
        ],
    );
    let values = &actor.workbench().complex;
    field(
        patches,
        PROGRAMMER_CONTENT,
        430,
        COMPLEX_A_REAL,
        "A real",
        &values[0],
    );
    field(
        patches,
        PROGRAMMER_CONTENT,
        431,
        COMPLEX_A_IMAGINARY,
        "A imaginary",
        &values[1],
    );
    field(
        patches,
        PROGRAMMER_CONTENT,
        432,
        COMPLEX_B_REAL,
        "B real",
        &values[2],
    );
    field(
        patches,
        PROGRAMMER_CONTENT,
        433,
        COMPLEX_B_IMAGINARY,
        "B imaginary",
        &values[3],
    );
    button_group(
        patches,
        PROGRAMMER_CONTENT,
        COMPLEX_ACTIONS,
        &[
            (COMPLEX_ADD, "A + B"),
            (COMPLEX_SUBTRACT, "A − B"),
            (COMPLEX_MULTIPLY, "A × B"),
            (COMPLEX_DIVIDE, "A ÷ B"),
            (COMPLEX_CONJUGATE, "Conjugate A"),
        ],
    );
    field(
        patches,
        PROGRAMMER_CONTENT,
        434,
        CHARACTER,
        "Character",
        actor.workbench().character.as_str(),
    );
    button_group(
        patches,
        PROGRAMMER_CONTENT,
        435,
        &[(CHARACTER_CONVERT, "Character code")],
    );
    output(
        patches,
        PROGRAMMER_CONTENT,
        actor.workbench().display_output().as_str(),
    );
}

pub(super) fn dialog(patches: &mut Vec<Patch>, actor: &RiceCalc) {
    let Some(dialog) = actor.dialog() else {
        return;
    };
    let (title, content) = match dialog {
        DialogKind::Shortcuts => (
            "Keyboard Shortcuts",
            "Enter — calculate\nEscape — clear\nBackspace — delete previous character\nAlt+C — copy result\nAlt+H — show or hide history\nAlt+? — keyboard shortcuts",
        ),
        DialogKind::About => (
            "About RiceCalc",
            "RiceCalc\nA dependency-free calculator for RiceWM.\nArbitrary-precision arithmetic, scientific, financial, conversion and programmer tools.",
        ),
    };
    create(
        patches,
        DIALOG,
        Some(1),
        WidgetKind::Dialog,
        Some("panel:dialog"),
        enabled(),
    );
    create(
        patches,
        DIALOG_TITLE,
        Some(DIALOG),
        WidgetKind::Label,
        Some(title),
        enabled() | WidgetState::EMPHASIZED,
    );
    create(
        patches,
        DIALOG_CONTENT,
        Some(DIALOG),
        WidgetKind::Label,
        Some(content),
        enabled(),
    );
    create(
        patches,
        DIALOG_ACTIONS,
        Some(DIALOG),
        WidgetKind::Row,
        Some("panel:actions"),
        enabled(),
    );
    create(
        patches,
        DIALOG_CLOSE,
        Some(DIALOG_ACTIONS),
        WidgetKind::Button,
        Some(rice_i18n::t("Close")),
        enabled() | WidgetState::ACCENT,
    );
}

fn field(patches: &mut Vec<Patch>, parent: u64, row: u64, id: u64, label: &str, value: &str) {
    create(
        patches,
        row,
        Some(parent),
        WidgetKind::Row,
        Some("panel:equal"),
        enabled(),
    );
    create(
        patches,
        row + 1000,
        Some(row),
        WidgetKind::Label,
        Some(label),
        enabled() | WidgetState::MUTED,
    );
    create(
        patches,
        id,
        Some(row),
        WidgetKind::TextField,
        Some(value),
        enabled(),
    );
}

fn button_group(patches: &mut Vec<Patch>, parent: u64, row: u64, buttons: &[(u64, &str)]) {
    create(
        patches,
        row,
        Some(parent),
        WidgetKind::Row,
        Some("panel:equal"),
        enabled(),
    );
    for (id, label) in buttons {
        let state = if buttons.len() == 1 {
            enabled() | WidgetState::ACCENT
        } else {
            enabled()
        };
        create(
            patches,
            *id,
            Some(row),
            WidgetKind::Button,
            Some(label),
            state,
        );
    }
}

fn output(patches: &mut Vec<Patch>, parent: u64, value: &str) {
    if !value.is_empty() {
        create(
            patches,
            WORKBENCH_OUTPUT,
            Some(parent),
            WidgetKind::Label,
            Some(value),
            enabled() | WidgetState::EMPHASIZED | WidgetState::END_ALIGNED,
        );
    }
}

fn heading(patches: &mut Vec<Patch>, parent: u64, id: u64, label: &str) {
    create(
        patches,
        id,
        Some(parent),
        WidgetKind::Label,
        Some(label),
        enabled() | WidgetState::EMPHASIZED,
    );
}

fn category_group(
    patches: &mut Vec<Patch>,
    parent: u64,
    row: u64,
    labels: &[&str],
    offset: usize,
    actor: &RiceCalc,
) {
    create(
        patches,
        row,
        Some(parent),
        WidgetKind::Row,
        Some("panel:equal"),
        enabled(),
    );
    for (local, label) in labels.iter().enumerate() {
        let index = offset + local;
        let state = if actor.workbench().category == index {
            enabled() | WidgetState::SELECTED
        } else {
            enabled()
        };
        create(
            patches,
            CATEGORY_BASE + u64::try_from(index).unwrap_or(0),
            Some(row),
            WidgetKind::Button,
            Some(rice_i18n::t(label)),
            state,
        );
    }
}
