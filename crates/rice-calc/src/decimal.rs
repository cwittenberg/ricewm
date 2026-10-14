use std::{fmt, str::FromStr};

const MAX_SCALE: u32 = 12;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Decimal {
    coefficient: i128,
    scale: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecimalError {
    Invalid,
    Overflow,
    DivisionByZero,
    NonInteger,
}

impl Decimal {
    pub const ZERO: Self = Self::new_raw(0, 0);
    pub const ONE: Self = Self::new_raw(1, 0);

    #[must_use]
    pub const fn new_raw(coefficient: i128, scale: u32) -> Self {
        Self { coefficient, scale }
    }

    #[must_use]
    pub fn from_i128(value: i128) -> Self {
        Self::new_raw(value, 0)
    }

    pub fn from_f64(value: f64) -> Result<Self, DecimalError> {
        if !value.is_finite() {
            return Err(DecimalError::Invalid);
        }
        format!("{value:.12}").parse()
    }

    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn as_f64(self) -> f64 {
        self.coefficient as f64 / 10_f64.powi(i32::try_from(self.scale).unwrap_or(12))
    }

    pub fn as_i128(self) -> Result<i128, DecimalError> {
        if self.scale == 0 {
            Ok(self.coefficient)
        } else {
            let factor = power10(self.scale)?;
            if self.coefficient % factor == 0 {
                Ok(self.coefficient / factor)
            } else {
                Err(DecimalError::NonInteger)
            }
        }
    }

    pub fn checked_add(self, rhs: Self) -> Result<Self, DecimalError> {
        let (left, right, scale) = align(self, rhs)?;
        left.checked_add(right)
            .map(|value| normalize(Self::new_raw(value, scale)))
            .ok_or(DecimalError::Overflow)
    }

    pub fn checked_sub(self, rhs: Self) -> Result<Self, DecimalError> {
        self.checked_add(rhs.checked_neg()?)
    }

    pub fn checked_mul(self, rhs: Self) -> Result<Self, DecimalError> {
        let coefficient = self
            .coefficient
            .checked_mul(rhs.coefficient)
            .ok_or(DecimalError::Overflow)?;
        let scale = self
            .scale
            .checked_add(rhs.scale)
            .ok_or(DecimalError::Overflow)?;
        if scale <= MAX_SCALE {
            return Ok(normalize(Self::new_raw(coefficient, scale)));
        }
        let divisor = power10(scale - MAX_SCALE)?;
        Ok(normalize(Self::new_raw(
            rounded_div(coefficient, divisor)?,
            MAX_SCALE,
        )))
    }

    pub fn checked_div(self, rhs: Self) -> Result<Self, DecimalError> {
        if rhs.coefficient == 0 {
            return Err(DecimalError::DivisionByZero);
        }
        let numerator_scale = MAX_SCALE
            .checked_add(rhs.scale)
            .ok_or(DecimalError::Overflow)?;
        let numerator = self
            .coefficient
            .checked_mul(power10(numerator_scale)?)
            .ok_or(DecimalError::Overflow)?;
        let divisor = rhs
            .coefficient
            .checked_mul(power10(self.scale)?)
            .ok_or(DecimalError::Overflow)?;
        Ok(normalize(Self::new_raw(
            rounded_div(numerator, divisor)?,
            MAX_SCALE,
        )))
    }

    pub fn checked_rem(self, rhs: Self) -> Result<Self, DecimalError> {
        if rhs.coefficient == 0 {
            return Err(DecimalError::DivisionByZero);
        }
        let (left, right, scale) = align(self, rhs)?;
        Ok(normalize(Self::new_raw(left % right, scale)))
    }

    pub fn checked_neg(self) -> Result<Self, DecimalError> {
        self.coefficient
            .checked_neg()
            .map(|value| Self::new_raw(value, self.scale))
            .ok_or(DecimalError::Overflow)
    }

    pub fn checked_percent(self) -> Result<Self, DecimalError> {
        self.checked_div(Self::from_i128(100))
    }
}

impl FromStr for Decimal {
    type Err = DecimalError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(DecimalError::Invalid);
        }
        if raw.contains(['e', 'E']) {
            let value = raw.parse::<f64>().map_err(|_| DecimalError::Invalid)?;
            return Self::from_f64(value);
        }
        let negative = raw.starts_with('-');
        let unsigned = raw.strip_prefix(['-', '+']).unwrap_or(raw);
        let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
        if whole.is_empty() && fraction.is_empty()
            || !whole.chars().all(|character| character.is_ascii_digit())
            || !fraction.chars().all(|character| character.is_ascii_digit())
        {
            return Err(DecimalError::Invalid);
        }
        let scale = u32::try_from(fraction.len().min(MAX_SCALE as usize))
            .map_err(|_| DecimalError::Overflow)?;
        let mut digits = String::from(if whole.is_empty() { "0" } else { whole });
        digits.push_str(&fraction[..scale as usize]);
        let mut coefficient = digits.parse::<i128>().map_err(|_| DecimalError::Overflow)?;
        if negative {
            coefficient = coefficient.checked_neg().ok_or(DecimalError::Overflow)?;
        }
        Ok(normalize(Self::new_raw(coefficient, scale)))
    }
}

impl fmt::Display for Decimal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.scale == 0 {
            return self.coefficient.fmt(formatter);
        }
        let negative = self.coefficient < 0;
        let digits = self.coefficient.unsigned_abs().to_string();
        let scale = usize::try_from(self.scale).unwrap_or(0);
        if negative {
            formatter.write_str("-")?;
        }
        if digits.len() <= scale {
            formatter.write_str("0.")?;
            for _ in 0..scale.saturating_sub(digits.len()) {
                formatter.write_str("0")?;
            }
            formatter.write_str(&digits)
        } else {
            let split = digits.len() - scale;
            write!(formatter, "{}.{}", &digits[..split], &digits[split..])
        }
    }
}

fn align(left: Decimal, right: Decimal) -> Result<(i128, i128, u32), DecimalError> {
    let scale = left.scale.max(right.scale);
    let left = left
        .coefficient
        .checked_mul(power10(scale - left.scale)?)
        .ok_or(DecimalError::Overflow)?;
    let right = right
        .coefficient
        .checked_mul(power10(scale - right.scale)?)
        .ok_or(DecimalError::Overflow)?;
    Ok((left, right, scale))
}

fn normalize(mut value: Decimal) -> Decimal {
    while value.scale > 0 && value.coefficient % 10 == 0 {
        value.coefficient /= 10;
        value.scale -= 1;
    }
    value
}

fn power10(power: u32) -> Result<i128, DecimalError> {
    10_i128.checked_pow(power).ok_or(DecimalError::Overflow)
}

fn rounded_div(numerator: i128, denominator: i128) -> Result<i128, DecimalError> {
    let quotient = numerator
        .checked_div(denominator)
        .ok_or(DecimalError::Overflow)?;
    let remainder = numerator
        .checked_rem(denominator)
        .ok_or(DecimalError::Overflow)?;
    if remainder == 0 {
        return Ok(quotient);
    }
    let twice = remainder
        .unsigned_abs()
        .checked_mul(2)
        .ok_or(DecimalError::Overflow)?;
    if twice >= denominator.unsigned_abs() {
        quotient
            .checked_add(if numerator.signum() == denominator.signum() {
                1
            } else {
                -1
            })
            .ok_or(DecimalError::Overflow)
    } else {
        Ok(quotient)
    }
}
