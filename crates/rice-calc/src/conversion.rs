use crate::{Decimal, DecimalError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnitCategory {
    Length,
    Mass,
    Temperature,
    Data,
    Angle,
    Area,
    Volume,
    Speed,
    Time,
    Pressure,
    Energy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Unit {
    Meter,
    Kilometer,
    Centimeter,
    Inch,
    Foot,
    Mile,
    Gram,
    Kilogram,
    Ounce,
    Pound,
    Celsius,
    Fahrenheit,
    Kelvin,
    Byte,
    Kilobyte,
    Megabyte,
    Gigabyte,
    Radian,
    Degree,
    SquareMeter,
    Hectare,
    Acre,
    Liter,
    Milliliter,
    GallonUs,
    MeterPerSecond,
    KilometerPerHour,
    MilePerHour,
    Second,
    Minute,
    Hour,
    Day,
    Pascal,
    Bar,
    Psi,
    Joule,
    Kilojoule,
    Calorie,
    KilowattHour,
}

impl Unit {
    #[must_use]
    pub const fn category(self) -> UnitCategory {
        match self {
            Self::Meter
            | Self::Kilometer
            | Self::Centimeter
            | Self::Inch
            | Self::Foot
            | Self::Mile => UnitCategory::Length,
            Self::Gram | Self::Kilogram | Self::Ounce | Self::Pound => UnitCategory::Mass,
            Self::Celsius | Self::Fahrenheit | Self::Kelvin => UnitCategory::Temperature,
            Self::Byte | Self::Kilobyte | Self::Megabyte | Self::Gigabyte => UnitCategory::Data,
            Self::Radian | Self::Degree => UnitCategory::Angle,
            Self::SquareMeter | Self::Hectare | Self::Acre => UnitCategory::Area,
            Self::Liter | Self::Milliliter | Self::GallonUs => UnitCategory::Volume,
            Self::MeterPerSecond | Self::KilometerPerHour | Self::MilePerHour => {
                UnitCategory::Speed
            }
            Self::Second | Self::Minute | Self::Hour | Self::Day => UnitCategory::Time,
            Self::Pascal | Self::Bar | Self::Psi => UnitCategory::Pressure,
            Self::Joule | Self::Kilojoule | Self::Calorie | Self::KilowattHour => {
                UnitCategory::Energy
            }
        }
    }

    #[must_use]
    pub fn parse(code: &str) -> Option<Self> {
        Some(match code.trim().to_ascii_lowercase().as_str() {
            "m" | "meter" => Self::Meter,
            "km" | "kilometer" => Self::Kilometer,
            "cm" | "centimeter" => Self::Centimeter,
            "in" | "inch" => Self::Inch,
            "ft" | "foot" => Self::Foot,
            "mi" | "mile" => Self::Mile,
            "g" | "gram" => Self::Gram,
            "kg" | "kilogram" => Self::Kilogram,
            "oz" | "ounce" => Self::Ounce,
            "lb" | "pound" => Self::Pound,
            "c" | "celsius" => Self::Celsius,
            "f" | "fahrenheit" => Self::Fahrenheit,
            "k" | "kelvin" => Self::Kelvin,
            "b" | "byte" => Self::Byte,
            "kb" | "kilobyte" => Self::Kilobyte,
            "mb" | "megabyte" => Self::Megabyte,
            "gb" | "gigabyte" => Self::Gigabyte,
            "rad" | "radian" => Self::Radian,
            "deg" | "degree" => Self::Degree,
            "m2" | "square-meter" => Self::SquareMeter,
            "ha" | "hectare" => Self::Hectare,
            "acre" => Self::Acre,
            "l" | "liter" => Self::Liter,
            "ml" | "milliliter" => Self::Milliliter,
            "gal" | "gallon-us" => Self::GallonUs,
            "m/s" => Self::MeterPerSecond,
            "km/h" => Self::KilometerPerHour,
            "mph" => Self::MilePerHour,
            "s" | "second" => Self::Second,
            "min" | "minute" => Self::Minute,
            "h" | "hour" => Self::Hour,
            "day" => Self::Day,
            "pa" | "pascal" => Self::Pascal,
            "bar" => Self::Bar,
            "psi" => Self::Psi,
            "j" | "joule" => Self::Joule,
            "kj" | "kilojoule" => Self::Kilojoule,
            "cal" | "calorie" => Self::Calorie,
            "kwh" => Self::KilowattHour,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConversionError {
    CategoryMismatch,
    InvalidRate,
    RateUnavailable,
    Arithmetic(DecimalError),
}

impl From<DecimalError> for ConversionError {
    fn from(value: DecimalError) -> Self {
        Self::Arithmetic(value)
    }
}

pub trait CurrencyRateProvider {
    fn rate(&self, from_iso_4217: &str, to_iso_4217: &str) -> Option<Decimal>;
}

pub fn convert_currency(
    amount: Decimal,
    from_iso_4217: &str,
    to_iso_4217: &str,
    rates: &impl CurrencyRateProvider,
) -> Result<Decimal, ConversionError> {
    if from_iso_4217.eq_ignore_ascii_case(to_iso_4217) {
        return Ok(amount);
    }
    let rate = rates
        .rate(from_iso_4217, to_iso_4217)
        .ok_or(ConversionError::RateUnavailable)?;
    if rate <= Decimal::ZERO {
        return Err(ConversionError::InvalidRate);
    }
    amount.checked_mul(rate).map_err(Into::into)
}

pub fn convert_unit(amount: Decimal, from: Unit, to: Unit) -> Result<Decimal, ConversionError> {
    if from.category() != to.category() {
        return Err(ConversionError::CategoryMismatch);
    }
    if from == to {
        return Ok(amount);
    }
    let base = to_base(amount, from)?;
    from_base(base, to)
}

fn to_base(amount: Decimal, unit: Unit) -> Result<Decimal, ConversionError> {
    match unit {
        Unit::Meter
        | Unit::Gram
        | Unit::Celsius
        | Unit::Byte
        | Unit::Radian
        | Unit::SquareMeter
        | Unit::Liter
        | Unit::MeterPerSecond
        | Unit::Second
        | Unit::Pascal
        | Unit::Joule => Ok(amount),
        Unit::Kilometer | Unit::Kilogram | Unit::Kilobyte | Unit::Kilojoule => {
            multiply(amount, "1000")
        }
        Unit::Centimeter => multiply(amount, "0.01"),
        Unit::Inch => multiply(amount, "0.0254"),
        Unit::Foot => multiply(amount, "0.3048"),
        Unit::Mile => multiply(amount, "1609.344"),
        Unit::Ounce => multiply(amount, "28.349523125"),
        Unit::Pound => multiply(amount, "453.59237"),
        Unit::Fahrenheit => amount
            .checked_sub(decimal("32")?)?
            .checked_mul(decimal("5")?)?
            .checked_div(decimal("9")?)
            .map_err(Into::into),
        Unit::Kelvin => amount.checked_sub(decimal("273.15")?).map_err(Into::into),
        Unit::Megabyte => multiply(amount, "1000000"),
        Unit::Gigabyte => multiply(amount, "1000000000"),
        Unit::Degree => multiply(amount, "0.01745329252"),
        Unit::Hectare => multiply(amount, "10000"),
        Unit::Acre => multiply(amount, "4046.8564224"),
        Unit::Milliliter => multiply(amount, "0.001"),
        Unit::GallonUs => multiply(amount, "3.785411784"),
        Unit::KilometerPerHour => multiply(amount, "0.277777777778"),
        Unit::MilePerHour => multiply(amount, "0.44704"),
        Unit::Minute => multiply(amount, "60"),
        Unit::Hour => multiply(amount, "3600"),
        Unit::Day => multiply(amount, "86400"),
        Unit::Bar => multiply(amount, "100000"),
        Unit::Psi => multiply(amount, "6894.757293168"),
        Unit::Calorie => multiply(amount, "4.184"),
        Unit::KilowattHour => multiply(amount, "3600000"),
    }
}

fn from_base(amount: Decimal, unit: Unit) -> Result<Decimal, ConversionError> {
    match unit {
        Unit::Meter
        | Unit::Gram
        | Unit::Celsius
        | Unit::Byte
        | Unit::Radian
        | Unit::SquareMeter
        | Unit::Liter
        | Unit::MeterPerSecond
        | Unit::Second
        | Unit::Pascal
        | Unit::Joule => Ok(amount),
        Unit::Kilometer | Unit::Kilogram | Unit::Kilobyte | Unit::Kilojoule => {
            divide(amount, "1000")
        }
        Unit::Centimeter => divide(amount, "0.01"),
        Unit::Inch => divide(amount, "0.0254"),
        Unit::Foot => divide(amount, "0.3048"),
        Unit::Mile => divide(amount, "1609.344"),
        Unit::Ounce => divide(amount, "28.349523125"),
        Unit::Pound => divide(amount, "453.59237"),
        Unit::Fahrenheit => amount
            .checked_mul(decimal("9")?)?
            .checked_div(decimal("5")?)?
            .checked_add(decimal("32")?)
            .map_err(Into::into),
        Unit::Kelvin => amount.checked_add(decimal("273.15")?).map_err(Into::into),
        Unit::Megabyte => divide(amount, "1000000"),
        Unit::Gigabyte => divide(amount, "1000000000"),
        Unit::Degree => divide(amount, "0.01745329252"),
        Unit::Hectare => divide(amount, "10000"),
        Unit::Acre => divide(amount, "4046.8564224"),
        Unit::Milliliter => divide(amount, "0.001"),
        Unit::GallonUs => divide(amount, "3.785411784"),
        Unit::KilometerPerHour => divide(amount, "0.277777777778"),
        Unit::MilePerHour => divide(amount, "0.44704"),
        Unit::Minute => divide(amount, "60"),
        Unit::Hour => divide(amount, "3600"),
        Unit::Day => divide(amount, "86400"),
        Unit::Bar => divide(amount, "100000"),
        Unit::Psi => divide(amount, "6894.757293168"),
        Unit::Calorie => divide(amount, "4.184"),
        Unit::KilowattHour => divide(amount, "3600000"),
    }
}

fn decimal(value: &str) -> Result<Decimal, ConversionError> {
    value
        .parse()
        .map_err(|_| ConversionError::Arithmetic(DecimalError::Invalid))
}

fn multiply(amount: Decimal, factor: &str) -> Result<Decimal, ConversionError> {
    amount
        .checked_mul(decimal(factor)?)
        .map_err(ConversionError::from)
}

fn divide(amount: Decimal, factor: &str) -> Result<Decimal, ConversionError> {
    amount
        .checked_div(decimal(factor)?)
        .map_err(ConversionError::from)
}
