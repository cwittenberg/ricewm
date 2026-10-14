use super::*;
use rice_native_app_protocol::WidgetId;
use workbench::WorkbenchError;

const WORKBENCH_OUTPUT: u64 = 308;

fn actor() -> RiceCalc {
    RiceCalc::launch(AppId::new(90).unwrap(), &[]).unwrap().0
}
fn send(actor: &mut RiceCalc, target: u64, event: UiEvent) -> Vec<Message> {
    actor.handle_event(&UiEventEnvelope {
        app: actor.app, window: actor.window, target: WidgetId::new(target).unwrap(), event,
    }, RequestId::new(9).unwrap()).unwrap()
}
fn activate(actor: &mut RiceCalc, target: u64) {
    send(actor, target, UiEvent::Activate);
}
fn input(actor: &mut RiceCalc, target: u64, value: &str) {
    send(actor, target, UiEvent::ValueChanged(value.into()));
}
fn text(actor: &RiceCalc, id: u64) -> &str {
    actor.tree.get(WidgetId::new(id).unwrap()).unwrap().text.as_deref().unwrap()
}

#[test]
fn event_generated_workbench_errors_retranslate_on_same_actor_in_all_locales() {
    let cases: &[(u64, &[(u64, &str)], u64, WorkbenchError)] = &[
        (MODE_FINANCIAL, &[(FINANCIAL_PRINCIPAL, "not-a-number")], FINANCIAL_FUTURE, WorkbenchError::Decimal),
        (MODE_FINANCIAL, &[(FINANCIAL_PERIODS, "1.5")], FINANCIAL_FUTURE, WorkbenchError::Periods),
        (MODE_FINANCIAL, &[(FINANCIAL_RATE, "100")], FINANCIAL_GROSS_PROFIT, WorkbenchError::Financial),
        (MODE_CONVERSION, &[(UNIT_FROM, "km"), (UNIT_TO, "kg")], UNIT_CONVERT, WorkbenchError::Unit),
        (MODE_PROGRAMMER, &[(COMPLEX_A_REAL, "Save")], COMPLEX_MULTIPLY, WorkbenchError::Complex),
        (MODE_PROGRAMMER, &[(CHARACTER, "中文")], CHARACTER_CONVERT, WorkbenchError::Character),
        (MODE_PROGRAMMER, &[(EXPRESSION, "0")], FACTOR, WorkbenchError::Integer),
        (MODE_PROGRAMMER, &[(EXPRESSION, "(")], TWOS_COMPLEMENT, WorkbenchError::WholeExpression),
    ];
    for &(mode, fields, action, error) in cases {
        let mut actor = actor();
        rice_i18n::with_locale(2, || {
            activate(&mut actor, mode);
            for &(field, value) in fields { input(&mut actor, field, value); }
            activate(&mut actor, action);
        });
        assert_eq!(actor.workbench.output, error.message());
        let state = actor.workbench.clone();
        let expression = actor.calculator.expression().to_owned();
        for &locale in rice_i18n::LOCALE_CODES {
            rice_i18n::with_locale(locale, || {
                assert!(matches!(actor.emit().unwrap(), Message::UiPatch(_)));
                assert_eq!(text(&actor, WORKBENCH_OUTPUT), rice_i18n::t(error.key()), "{error:?}, locale {locale}");
                assert_eq!(actor.workbench, state);
                assert_eq!(actor.calculator.expression(), expression);
            });
        }
    }
}

#[test]
fn category_labels_use_quantity_context_keys_and_preserve_unit_protocol_values() {
    let mut actor = actor();
    activate(&mut actor, MODE_CONVERSION);
    for index in 0..workbench::CATEGORY_COUNT {
        activate(&mut actor, CATEGORY_BASE + index as u64);
        let state = actor.workbench.clone();
        for &locale in rice_i18n::LOCALE_CODES {
            rice_i18n::with_locale(locale, || {
                actor.emit().unwrap();
                for (i, key) in workbench::CATEGORY_KEYS.iter().enumerate() {
                    assert_eq!(text(&actor, CATEGORY_BASE + i as u64), rice_i18n::t(key));
                }
                assert_eq!(actor.workbench, state);
                assert_eq!(text(&actor, UNIT_FROM), state.unit[1]);
                assert_eq!(text(&actor, UNIT_TO), state.unit[2]);
            });
        }
    }
    assert_eq!(workbench::CATEGORY_KEYS[2], "calc.category.volume");
    assert_eq!(workbench::CATEGORY_KEYS[6], "calc.category.time");
}

#[test]
fn character_metadata_refreshes_without_reinterpreting_input_or_changing_raw_output() {
    let mut actor = actor();
    activate(&mut actor, MODE_PROGRAMMER);
    input(&mut actor, CHARACTER, "λ");
    activate(&mut actor, CHARACTER_CONVERT);
    let raw = actor.workbench.output.clone();
    assert_eq!(raw, "U+03BB · decimal 955 · UTF-8 CE BB");
    // Editing the field does not change the last calculated code point.
    input(&mut actor, CHARACTER, "Save {hex}");
    for &locale in rice_i18n::LOCALE_CODES {
        rice_i18n::with_locale(locale, || {
            actor.emit().unwrap();
            assert_eq!(text(&actor, WORKBENCH_OUTPUT), rice_i18n::format(
                "calc.workbench.character-code", &[("hex", "03BB"), ("decimal", "955"), ("bytes", "CE BB")]));
            assert_eq!(actor.workbench.output, raw);
            assert_eq!(text(&actor, CHARACTER), "Save {hex}");
        });
    }
    // A successful numeric operation clears codepoint presentation metadata.
    input(&mut actor, EXPRESSION, "360");
    activate(&mut actor, FACTOR);
    assert_eq!(actor.workbench.output, "2 × 2 × 2 × 3 × 3 × 5");
    for &locale in rice_i18n::LOCALE_CODES {
        rice_i18n::with_locale(locale, || {
            actor.emit().unwrap();
            assert_eq!(text(&actor, WORKBENCH_OUTPUT), "2 × 2 × 2 × 3 × 3 × 5");
        });
    }
}

#[test]
fn numeric_formula_and_copy_contracts_remain_literal_across_locale_changes() {
    let mut actor = actor();
    activate(&mut actor, MODE_CONVERSION);
    input(&mut actor, UNIT_FROM, "invalid");
    activate(&mut actor, UNIT_CONVERT);
    input(&mut actor, UNIT_FROM, "km");
    activate(&mut actor, UNIT_CONVERT);
    assert_eq!(actor.workbench.output, "1000");
    input(&mut actor, EXPRESSION, "6*7");
    activate(&mut actor, EQUALS);
    for &locale in rice_i18n::LOCALE_CODES {
        rice_i18n::with_locale(locale, || {
            actor.emit().unwrap();
            assert_eq!(text(&actor, WORKBENCH_OUTPUT), "1000");
            assert_eq!(actor.calculator.expression(), "6*7");
            let messages = send(&mut actor, COPY, UiEvent::Activate);
            assert!(messages.iter().any(|message| matches!(message,
                Message::ClipboardRequest(ClipboardRequest::Write { data, mime_type, .. })
                    if data == b"42" && mime_type == "text/plain;charset=utf-8")));
        });
    }
    actor.workbench.set_literal("Enter exactly one Unicode character.".into());
    for &locale in rice_i18n::LOCALE_CODES {
        rice_i18n::with_locale(locale, || {
            actor.emit().unwrap();
            assert_eq!(text(&actor, WORKBENCH_OUTPUT), "Enter exactly one Unicode character.");
        });
    }
}
