use rice_calc::{
    AngleUnit, BitWidth, Calculator, ConversionError, CurrencyRateProvider, Decimal, IntegerBase,
    Mode, Unit, compounding_term, convert_currency, convert_unit, evaluate, future_value,
    gross_profit_price, payment, periodic_interest_rate, straight_line_depreciation,
    sum_of_years_digits_depreciation,
};

struct NoRates;

impl CurrencyRateProvider for NoRates {
    fn rate(&self, _: &str, _: &str) -> Option<Decimal> {
        None
    }
}

#[test]
fn exact_decimal_arithmetic_respects_precedence_parentheses_and_percent() {
    assert_eq!(evaluate("0.1 + 0.2", true).unwrap().to_string(), "0.3");
    assert_eq!(evaluate("2 + 3 * 4", true).unwrap().to_string(), "14");
    assert_eq!(evaluate("(2 + 3) * 4", true).unwrap().to_string(), "20");
    assert_eq!(evaluate("250 * 10%", true).unwrap().to_string(), "25");
}

#[test]
fn scientific_and_programmer_operations_are_real() {
    assert_eq!(evaluate("sin(30)", true).unwrap().to_string(), "0.5");
    assert_eq!(evaluate("sqrt(81) + 2^3", true).unwrap().to_string(), "17");
    assert_eq!(evaluate("0xF and 0b1010", true).unwrap().to_string(), "10");
    assert_eq!(evaluate("3 << 4", true).unwrap().to_string(), "48");
    assert_eq!(evaluate("5!", true).unwrap().to_string(), "120");
    assert_eq!(evaluate("cosh(0)", true).unwrap().to_string(), "1");
    assert_eq!(evaluate("not(0)", true).unwrap().to_string(), "-1");
}

#[test]
fn calculator_tracks_memory_history_modes_and_bases() {
    let mut calculator = Calculator::new();
    calculator.replace_expression("6 * 7").unwrap();
    assert_eq!(calculator.evaluate().unwrap().to_string(), "42");
    calculator.memory_store();
    calculator.clear();
    calculator.memory_recall().unwrap();
    assert_eq!(calculator.expression(), "42");
    calculator.set_mode(Mode::Programmer);
    calculator.set_integer_base(IntegerBase::Hexadecimal);
    calculator.set_bit_width(BitWidth::Bits16);
    calculator.evaluate().unwrap();
    assert_eq!(calculator.programmer_display(), "2A");
    calculator.replace_expression("-1").unwrap();
    calculator.evaluate().unwrap();
    assert_eq!(
        calculator.twos_complement_display(),
        "16-bit · binary 1111111111111111 · hexadecimal FFFF · signed -1"
    );
    assert_eq!(calculator.history().len(), 3);
    calculator.set_angle_unit(AngleUnit::Radians);
    assert_eq!(calculator.angle_unit(), AngleUnit::Radians);
}

#[test]
fn physical_conversions_work_and_currency_never_invents_rates() {
    let kilometer = convert_unit("1000".parse().unwrap(), Unit::Meter, Unit::Kilometer).unwrap();
    assert_eq!(kilometer.to_string(), "1");
    let fahrenheit = convert_unit("100".parse().unwrap(), Unit::Celsius, Unit::Fahrenheit).unwrap();
    assert_eq!(fahrenheit.to_string(), "212");
    for (from, to) in [
        (Unit::Hectare, Unit::SquareMeter),
        (Unit::Liter, Unit::Milliliter),
        (Unit::Kilogram, Unit::Gram),
        (Unit::KilometerPerHour, Unit::MilePerHour),
        (Unit::Hour, Unit::Minute),
        (Unit::Bar, Unit::Psi),
        (Unit::KilowattHour, Unit::Kilojoule),
        (Unit::Gigabyte, Unit::Megabyte),
        (Unit::Degree, Unit::Radian),
    ] {
        assert!(convert_unit(Decimal::ONE, from, to).is_ok());
    }

    assert_eq!(
        convert_currency(Decimal::ONE, "EUR", "USD", &NoRates),
        Err(ConversionError::RateUnavailable)
    );
}

#[test]
fn errors_are_bounded_and_explicit() {
    assert!(
        evaluate("1 / 0", true)
            .unwrap_err()
            .to_string()
            .contains("Division")
    );
    let mut calculator = Calculator::new();
    assert!(calculator.replace_expression(&"1".repeat(4097)).is_err());
}

#[test]
fn financial_core_uses_real_formulas_without_network_data() {
    let future = future_value("1000".parse().unwrap(), "0.1".parse().unwrap(), 2).unwrap();
    assert_eq!(future.to_string(), "1210");
    assert_eq!(
        payment("1200".parse().unwrap(), Decimal::ZERO, 12)
            .unwrap()
            .to_string(),
        "100"
    );
    assert_eq!(
        straight_line_depreciation("1000".parse().unwrap(), "100".parse().unwrap(), 9,)
            .unwrap()
            .to_string(),
        "100"
    );
    assert_eq!(
        gross_profit_price("75".parse().unwrap(), "25".parse().unwrap())
            .unwrap()
            .to_string(),
        "100"
    );
    assert_eq!(
        sum_of_years_digits_depreciation("1000".parse().unwrap(), "100".parse().unwrap(), 9, 1,)
            .unwrap()
            .to_string(),
        "180"
    );
    assert_eq!(
        periodic_interest_rate("100".parse().unwrap(), "121".parse().unwrap(), 2)
            .unwrap()
            .to_string(),
        "0.1"
    );
    assert_eq!(
        compounding_term(
            "100".parse().unwrap(),
            "121".parse().unwrap(),
            "0.1".parse().unwrap(),
        )
        .unwrap()
        .to_string(),
        "2"
    );
}
