use super::*;
use rice_native_app_protocol::{UiEvent, UiEventEnvelope, WidgetId};
use rice_ui::WidgetState;

fn app() -> AppId {
    AppId::new(90).unwrap()
}

fn request() -> RequestId {
    RequestId::new(9).unwrap()
}

fn event(target: u64, event: UiEvent) -> UiEventEnvelope {
    UiEventEnvelope {
        app: app(),
        window: window_id(),
        target: WidgetId::new(target).unwrap(),
        event,
    }
}

#[test]
fn launch_has_conventional_menus_expression_result_and_working_keypad() {
    let (mut actor, Message::UiPatch(patch)) = RiceCalc::launch(app(), &[]).unwrap() else {
        panic!("launch patch");
    };
    let text = format!("{:?}", patch.patches);
    for label in ["Mode", "Edit", "View", "Help", "Copy Result", "Basic"] {
        assert!(text.contains(label), "{label}");
    }
    actor
        .handle_event(&event(input_id("6"), UiEvent::Activate), request())
        .unwrap();
    actor
        .handle_event(&event(input_id("*"), UiEvent::Activate), request())
        .unwrap();
    actor
        .handle_event(&event(input_id("7"), UiEvent::Activate), request())
        .unwrap();
    actor
        .handle_event(&event(EQUALS, UiEvent::Activate), request())
        .unwrap();
    assert_eq!(actor.calculator().result().to_string(), "42");
}

#[test]
fn live_theme_changes_do_not_leave_redmond_calculator_content_in_other_themes() {
    let (mut actor, _) =
        RiceCalc::launch(app(), &[String::from("--theme-profile=redmond-98")]).unwrap();
    assert!(actor.redmond98());
    let messages = actor
        .handle_host_event(&HostEvent {
            app: app(),
            event: HostEventKind::RiceBus {
                topic: THEME_CHANGED_TOPIC.to_owned(),
                payload: String::from("orchard"),
            },
        })
        .expect("switch calculator to Orchard");
    assert!(!actor.redmond98());
    assert!(matches!(messages.as_slice(), [Message::UiPatch(_)]));
}

#[test]
fn redmond_classic_keys_expose_real_memory_and_clear_semantics() {
    let (_, Message::UiPatch(initial)) =
        RiceCalc::launch(app(), &[String::from("--theme-profile=redmond-98")]).unwrap()
    else {
        panic!("launch patch");
    };
    for expected in [
        BACKSPACE,
        CLEAR,
        CLASSIC_CLEAR,
        MEMORY_CLEAR,
        MEMORY_RECALL,
        MEMORY_STORE,
        MEMORY_ADD,
    ] {
        assert!(
            initial.patches.iter().any(|patch| matches!(
                patch,
                rice_native_app_protocol::UiPatch::Create { id, state, .. }
                    if *id == WidgetId::new(expected).unwrap()
                        && state & WidgetState::DESTRUCTIVE.bits() != 0
            )),
            "classic red key {expected}"
        );
    }
    assert!(initial.patches.iter().any(|patch| matches!(
        patch,
        rice_native_app_protocol::UiPatch::Create { id, state, .. }
            if *id == WidgetId::new(input_id("7")).unwrap()
                && state & WidgetState::DESTRUCTIVE.bits() == 0
    )));
    assert!(initial.patches.iter().any(|patch| matches!(
        patch,
        rice_native_app_protocol::UiPatch::Create { id, kind, text: Some(text), .. }
            if *id == WidgetId::new(CLASSIC_MEMORY_INDICATOR).unwrap()
                && *kind == rice_ui::WidgetKind::Container as u16
                && text == "panel:redmond98-calculator-memory-indicator"
    )));
}

#[test]
fn typing_previews_results_enter_calculates_and_programmer_scroll_is_live() {
    let (mut actor, Message::UiPatch(initial)) = RiceCalc::launch(app(), &[]).unwrap() else {
        panic!("launch patch");
    };
    assert!(initial.patches.iter().any(|patch| matches!(
        patch,
        rice_native_app_protocol::UiPatch::Create { id, text: Some(text), .. }
            if *id == WidgetId::new(EXPRESSION).unwrap() && text.is_empty()
    )));
    assert!(initial.patches.iter().any(|patch| matches!(
        patch,
        rice_native_app_protocol::UiPatch::Create { id, kind, state, .. }
            if *id == WidgetId::new(input_id("7")).unwrap()
                && *kind == rice_ui::WidgetKind::Button as u16
                && state & WidgetState::ENABLED.bits() != 0
                && state & WidgetState::KEY_NAVIGATION.bits() == 0
    )));

    actor
        .handle_event(
            &event(EXPRESSION, UiEvent::ValueChanged(String::from("6*7"))),
            request(),
        )
        .unwrap();
    assert_eq!(actor.calculator().result().to_string(), "42");
    assert!(actor.calculator().history().is_empty());
    actor
        .handle_event(
            &event(
                input_id("7"),
                UiEvent::Key {
                    pressed: true,
                    key_code: 36,
                    modifiers: 0,
                    text: None,
                },
            ),
            request(),
        )
        .unwrap();
    assert_eq!(actor.calculator().history().front().unwrap().1, "42");

    actor
        .handle_event(&event(MODE_PROGRAMMER, UiEvent::Activate), request())
        .unwrap();
    actor
        .handle_event(
            &event(
                panels::PROGRAMMER_PANEL,
                UiEvent::ValueChanged(String::from("192/800")),
            ),
            request(),
        )
        .unwrap();
    assert_eq!(actor.programmer_scroll(), 192);
}

#[test]
fn keyboard_modes_memory_history_and_copy_are_end_to_end() {
    let (mut actor, _) = RiceCalc::launch(app(), &[]).unwrap();
    actor
        .handle_event(
            &event(EXPRESSION, UiEvent::ValueChanged(String::from("sin(30)"))),
            request(),
        )
        .unwrap();
    actor
        .handle_event(
            &event(
                EXPRESSION,
                UiEvent::Key {
                    pressed: true,
                    key_code: 13,
                    modifiers: 0,
                    text: None,
                },
            ),
            request(),
        )
        .unwrap();
    assert_eq!(actor.calculator().result().to_string(), "0.5");
    actor
        .handle_event(&event(MEMORY_STORE, UiEvent::Activate), request())
        .unwrap();
    assert_eq!(actor.calculator().memory().to_string(), "1/2");
    actor
        .handle_event(&event(TOGGLE_HISTORY, UiEvent::Activate), request())
        .unwrap();
    assert!(actor.history_visible());
    let messages = actor
        .handle_event(&event(COPY, UiEvent::Activate), request())
        .unwrap();
    assert!(matches!(
        messages.as_slice(),
        [Message::ClipboardRequest(_)]
    ));
}

#[test]
fn advanced_and_programmer_controls_only_exist_in_their_modes() {
    let (mut actor, _) = RiceCalc::launch(app(), &[]).unwrap();
    actor
        .handle_event(&event(MODE_ADVANCED, UiEvent::Activate), request())
        .unwrap();
    assert_eq!(actor.calculator().mode(), Mode::Advanced);
    actor
        .handle_event(&event(input_id("sqrt("), UiEvent::Activate), request())
        .unwrap();
    assert_eq!(actor.calculator().expression(), "sqrt(");
    actor
        .handle_event(&event(MODE_PROGRAMMER, UiEvent::Activate), request())
        .unwrap();
    assert_eq!(actor.calculator().mode(), Mode::Programmer);
    actor
        .handle_event(&event(BASE_HEX, UiEvent::Activate), request())
        .unwrap();
    assert_eq!(actor.calculator().integer_base(), IntegerBase::Hexadecimal);
}

#[test]
fn financial_conversion_complex_character_and_factor_controls_execute() {
    let (mut actor, _) = RiceCalc::launch(app(), &[]).unwrap();
    actor
        .handle_event(&event(MODE_FINANCIAL, UiEvent::Activate), request())
        .unwrap();
    actor
        .handle_event(
            &event(
                FINANCIAL_PRINCIPAL,
                UiEvent::ValueChanged(String::from("1200")),
            ),
            request(),
        )
        .unwrap();
    actor
        .handle_event(&event(FINANCIAL_PAYMENT, UiEvent::Activate), request())
        .unwrap();
    assert!(!actor.workbench().output.is_empty());

    actor
        .handle_event(&event(MODE_CONVERSION, UiEvent::Activate), request())
        .unwrap();
    actor
        .handle_event(&event(CATEGORY_BASE + 8, UiEvent::Activate), request())
        .unwrap();
    assert_eq!(actor.workbench().unit[1..], ["kwh", "kj"]);
    for (target, value) in [(UNIT_AMOUNT, "1"), (UNIT_FROM, "km"), (UNIT_TO, "m")] {
        actor
            .handle_event(
                &event(target, UiEvent::ValueChanged(String::from(value))),
                request(),
            )
            .unwrap();
    }
    actor
        .handle_event(&event(UNIT_CONVERT, UiEvent::Activate), request())
        .unwrap();
    assert_eq!(actor.workbench().output, "1000");

    actor
        .handle_event(&event(MODE_PROGRAMMER, UiEvent::Activate), request())
        .unwrap();
    actor
        .handle_event(
            &event(CHARACTER, UiEvent::ValueChanged(String::from("λ"))),
            request(),
        )
        .unwrap();
    actor
        .handle_event(&event(CHARACTER_CONVERT, UiEvent::Activate), request())
        .unwrap();
    assert!(actor.workbench().output.contains("U+03BB"));
    actor
        .handle_event(
            &event(EXPRESSION, UiEvent::ValueChanged(String::from("360"))),
            request(),
        )
        .unwrap();
    actor
        .handle_event(&event(FACTOR, UiEvent::Activate), request())
        .unwrap();
    assert_eq!(actor.workbench().output, "2 × 2 × 2 × 3 × 3 × 5");
    actor
        .handle_event(&event(COMPLEX_MULTIPLY, UiEvent::Activate), request())
        .unwrap();
    assert_eq!(actor.workbench().output, "-5 + 10i");
}

#[test]
fn help_entries_open_real_closeable_dialogs() {
    let (mut actor, _) = RiceCalc::launch(app(), &[]).unwrap();
    let messages = actor
        .handle_event(&event(SHORTCUTS, UiEvent::Activate), request())
        .unwrap();
    assert_eq!(actor.dialog(), Some(DialogKind::Shortcuts));
    assert!(format!("{messages:?}").contains("Enter — calculate"));
    actor
        .handle_event(&event(DIALOG_CLOSE, UiEvent::Activate), request())
        .unwrap();
    assert_eq!(actor.dialog(), None);
    let messages = actor
        .handle_event(&event(ABOUT, UiEvent::Activate), request())
        .unwrap();
    assert_eq!(actor.dialog(), Some(DialogKind::About));
    assert!(format!("{messages:?}").contains("dependency-free calculator"));

    let copy = actor
        .handle_event(
            &event(
                EXPRESSION,
                UiEvent::Key {
                    pressed: true,
                    key_code: u32::from('c'),
                    modifiers: 8,
                    text: Some(String::from("c")),
                },
            ),
            request(),
        )
        .unwrap();
    assert!(matches!(
        copy.as_slice(),
        [Message::ClipboardRequest(ClipboardRequest::Write { .. })]
    ));
    actor
        .handle_event(
            &event(
                EXPRESSION,
                UiEvent::Key {
                    pressed: true,
                    key_code: u32::from('h'),
                    modifiers: 8,
                    text: Some(String::from("h")),
                },
            ),
            request(),
        )
        .unwrap();
    assert!(actor.history_visible());
}

#[test]
fn entropy_and_currency_are_only_requested_with_real_provider_capabilities() {
    let (mut unavailable, initial) = RiceCalc::launch(app(), &[]).unwrap();
    assert!(!format!("{initial:?}").contains("Random"));
    assert!(
        unavailable
            .handle_event(&event(RANDOM, UiEvent::Activate), request())
            .unwrap()
            .is_empty()
    );

    let arguments = vec![
        String::from("--entropy-provider"),
        String::from("--currency-provider"),
    ];
    let (mut actor, _) = RiceCalc::launch(app(), &arguments).unwrap();
    actor
        .handle_event(&event(MODE_ADVANCED, UiEvent::Activate), request())
        .unwrap();
    let random = actor
        .handle_event(&event(RANDOM, UiEvent::Activate), request())
        .unwrap();
    assert!(matches!(
        random.as_slice(),
        [Message::DataProviderRequest(DataProviderRequest {
            operation: DataProviderOperation::Entropy { byte_len: 8 },
            ..
        })]
    ));
    actor
        .handle_data_response(&DataProviderResponse {
            request: request(),
            result: DataProviderResult::Entropy(vec![0x80; 8]),
        })
        .unwrap();
    assert_eq!(actor.calculator().expression(), "random()");

    let currency = actor
        .handle_event(&event(CURRENCY_CONVERT, UiEvent::Activate), request())
        .unwrap();
    assert!(matches!(
        currency.as_slice(),
        [Message::DataProviderRequest(DataProviderRequest {
            operation: DataProviderOperation::CurrencyRate { .. },
            ..
        })]
    ));
    actor
        .handle_data_response(&DataProviderResponse {
            request: request(),
            result: DataProviderResult::CurrencyRate {
                decimal_rate: String::from("1.125"),
                observed_millis: 1_722_000_000_000,
                source: String::from("configured test feed"),
            },
        })
        .unwrap();
    assert!(actor.currency_fields().3.contains("configured test feed"));
}

#[test]
fn provider_notices_refresh_in_all_locales_without_translating_provider_data() {
    let (mut actor, _) = RiceCalc::launch(app(), &[
        "--entropy-provider".into(), "--currency-provider".into(),
    ]).unwrap();
    actor.handle_event(&event(MODE_CONVERSION, UiEvent::Activate), request()).unwrap();
    actor.handle_event(&event(CURRENCY_BASE, UiEvent::ValueChanged("中文 {code}".into())), request()).unwrap();
    actor.handle_event(&event(CURRENCY_CONVERT, UiEvent::Activate), request()).unwrap();
    for &locale in rice_i18n::LOCALE_CODES {
        rice_i18n::with_locale(locale, || {
            let message = actor.emit().unwrap();
            let text = rice_i18n::t("calc.status.invalid-currency-code");
            assert_eq!(actor.currency_fields().3, text);
            assert_eq!(actor.currency_fields().1, "中文 {CODE}");
            assert_eq!(actor.tree.get(rice_ui::WidgetId::new(427).unwrap()).unwrap().text.as_deref(), Some(text));
            assert!(matches!(message, Message::UiPatch(_)));
            assert!(actor.pending_provider.is_none());
        });
    }
    actor.handle_event(&event(RANDOM, UiEvent::Activate), request()).unwrap();
    actor.handle_data_response(&DataProviderResponse {
        request: request(), result: DataProviderResult::Entropy(vec![0x80;8]),
    }).unwrap();
    for &locale in rice_i18n::LOCALE_CODES {
        rice_i18n::with_locale(locale, || {
            actor.emit().unwrap();
            assert_eq!(actor.currency_fields().3, rice_i18n::t("calc.status.secure-random"));
            assert_eq!(actor.calculator.expression(), "random()");
        });
    }
    // Provider prose remains literal even when identical to an owned English notice.
    for literal in ["Currency codes must be three ASCII letters.", "Save {name} 中文 العربية"] {
        actor.handle_event(&event(RANDOM, UiEvent::Activate), request()).unwrap();
        actor.handle_data_response(&DataProviderResponse {
            request: request(), result: DataProviderResult::Unavailable { reason: literal.into() },
        }).unwrap();
        for &locale in rice_i18n::LOCALE_CODES {
            rice_i18n::with_locale(locale, || {
                actor.emit().unwrap();
                assert_eq!(actor.currency_fields().3, literal);
                assert_eq!(actor.tree.get(rice_ui::WidgetId::new(427).unwrap()).unwrap().text.as_deref(), Some(literal));
            });
        }
    }
}
