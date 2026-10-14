use crate::big_uint::{BigNumberError, BigUint};
use std::{cmp::Ordering, fmt, str::FromStr};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BigInt {
    negative: bool,
    magnitude: BigUint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BigRational {
    numerator: BigInt,
    denominator: BigUint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RationalError {
    Invalid,
    DivisionByZero,
    NonInteger,
    ExponentTooLarge,
}

impl BigInt {
    #[must_use]
    pub fn zero() -> Self {
        Self {
            negative: false,
            magnitude: BigUint::zero(),
        }
    }

    #[must_use]
    pub fn from_parts(negative: bool, magnitude: BigUint) -> Self {
        Self {
            negative: negative && !magnitude.is_zero(),
            magnitude,
        }
    }

    #[must_use]
    pub fn negate(&self) -> Self {
        Self::from_parts(!self.negative, self.magnitude.clone())
    }

    #[must_use]
    pub fn add(&self, rhs: &Self) -> Self {
        match (self.negative, rhs.negative) {
            (left, right) if left == right => {
                Self::from_parts(left, self.magnitude.add(&rhs.magnitude))
            }
            _ => match self.magnitude.cmp(&rhs.magnitude) {
                Ordering::Greater | Ordering::Equal => Self::from_parts(
                    self.negative,
                    self.magnitude
                        .subtract(&rhs.magnitude)
                        .expect("larger magnitude subtracts"),
                ),
                Ordering::Less => Self::from_parts(
                    rhs.negative,
                    rhs.magnitude
                        .subtract(&self.magnitude)
                        .expect("larger magnitude subtracts"),
                ),
            },
        }
    }

    #[must_use]
    pub fn subtract(&self, rhs: &Self) -> Self {
        self.add(&rhs.negate())
    }

    #[must_use]
    pub fn multiply(&self, rhs: &Self) -> Self {
        Self::from_parts(
            self.negative != rhs.negative,
            self.magnitude.multiply(&rhs.magnitude),
        )
    }

    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.magnitude.is_zero()
    }

    #[must_use]
    pub fn is_negative(&self) -> bool {
        self.negative
    }

    #[must_use]
    pub fn magnitude(&self) -> &BigUint {
        &self.magnitude
    }
}

impl FromStr for BigInt {
    type Err = RationalError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let negative = raw.starts_with('-');
        let unsigned = raw.strip_prefix(['-', '+']).unwrap_or(raw);
        let magnitude = unsigned.parse().map_err(|_| RationalError::Invalid)?;
        Ok(Self::from_parts(negative, magnitude))
    }
}

impl fmt::Display for BigInt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.negative {
            formatter.write_str("-")?;
        }
        self.magnitude.fmt(formatter)
    }
}

impl BigRational {
    #[must_use]
    pub fn zero() -> Self {
        Self {
            numerator: BigInt::zero(),
            denominator: BigUint::one(),
        }
    }

    #[must_use]
    pub fn one() -> Self {
        Self {
            numerator: BigInt::from_parts(false, BigUint::one()),
            denominator: BigUint::one(),
        }
    }

    #[allow(
        clippy::needless_pass_by_value,
        reason = "normalization owns both inputs and avoids mandatory caller clones"
    )]
    pub fn new(numerator: BigInt, denominator: BigUint) -> Result<Self, RationalError> {
        if denominator.is_zero() {
            return Err(RationalError::DivisionByZero);
        }
        if numerator.is_zero() {
            return Ok(Self::zero());
        }
        let divisor = BigUint::gcd(numerator.magnitude().clone(), denominator.clone());
        let numerator_magnitude = numerator.magnitude().div_rem(&divisor).map_err(map_big)?.0;
        let denominator = denominator.div_rem(&divisor).map_err(map_big)?.0;
        Ok(Self {
            numerator: BigInt::from_parts(numerator.is_negative(), numerator_magnitude),
            denominator,
        })
    }

    #[must_use]
    pub fn from_integer(value: BigInt) -> Self {
        Self {
            numerator: value,
            denominator: BigUint::one(),
        }
    }

    pub fn add(&self, rhs: &Self) -> Result<Self, RationalError> {
        let numerator = self
            .numerator
            .multiply(&BigInt::from_parts(false, rhs.denominator.clone()))
            .add(
                &rhs.numerator
                    .multiply(&BigInt::from_parts(false, self.denominator.clone())),
            );
        Self::new(numerator, self.denominator.multiply(&rhs.denominator))
    }

    pub fn subtract(&self, rhs: &Self) -> Result<Self, RationalError> {
        self.add(&rhs.negate())
    }

    pub fn multiply(&self, rhs: &Self) -> Result<Self, RationalError> {
        Self::new(
            self.numerator.multiply(&rhs.numerator),
            self.denominator.multiply(&rhs.denominator),
        )
    }

    pub fn divide(&self, rhs: &Self) -> Result<Self, RationalError> {
        if rhs.numerator.is_zero() {
            return Err(RationalError::DivisionByZero);
        }
        Self::new(
            BigInt::from_parts(
                self.numerator.is_negative() != rhs.numerator.is_negative(),
                self.numerator.magnitude().multiply(&rhs.denominator),
            ),
            self.denominator.multiply(rhs.numerator.magnitude()),
        )
    }

    #[must_use]
    pub fn negate(&self) -> Self {
        Self {
            numerator: self.numerator.negate(),
            denominator: self.denominator.clone(),
        }
    }

    pub fn pow(&self, exponent: i64) -> Result<Self, RationalError> {
        let absolute = exponent.unsigned_abs();
        if absolute > 100_000 {
            return Err(RationalError::ExponentTooLarge);
        }
        let mut result = Self::one();
        let mut base = self.clone();
        let mut remaining = absolute;
        while remaining != 0 {
            if remaining & 1 == 1 {
                result = result.multiply(&base)?;
            }
            remaining >>= 1;
            if remaining != 0 {
                base = base.multiply(&base)?;
            }
        }
        if exponent < 0 {
            Self::one().divide(&result)
        } else {
            Ok(result)
        }
    }

    pub fn integer(&self) -> Result<&BigInt, RationalError> {
        if self.denominator == BigUint::one() {
            Ok(&self.numerator)
        } else {
            Err(RationalError::NonInteger)
        }
    }

    #[must_use]
    pub fn numerator(&self) -> &BigInt {
        &self.numerator
    }

    #[must_use]
    pub fn denominator(&self) -> &BigUint {
        &self.denominator
    }

    #[must_use]
    pub fn decimal(&self, precision: usize) -> String {
        let (whole, mut remainder) = self
            .numerator
            .magnitude()
            .div_rem(&self.denominator)
            .expect("normalized denominator is nonzero");
        let sign = if self.numerator.is_negative() {
            "-"
        } else {
            ""
        };
        if remainder.is_zero() || precision == 0 {
            return format!("{sign}{whole}");
        }
        let mut fraction = String::new();
        for _ in 0..precision {
            remainder = remainder.multiply_small(10);
            let (digit, next) = remainder
                .div_rem(&self.denominator)
                .expect("normalized denominator is nonzero");
            fraction.push_str(&digit.to_string());
            remainder = next;
            if remainder.is_zero() {
                break;
            }
        }
        format!("{sign}{whole}.{fraction}")
    }

    #[must_use]
    pub fn finite_decimal(&self, maximum_digits: usize) -> Option<String> {
        let (whole, mut remainder) = self
            .numerator
            .magnitude()
            .div_rem(&self.denominator)
            .expect("normalized denominator is nonzero");
        let sign = if self.numerator.is_negative() {
            "-"
        } else {
            ""
        };
        if remainder.is_zero() {
            return Some(format!("{sign}{whole}"));
        }
        let mut fraction = String::new();
        for _ in 0..maximum_digits {
            remainder = remainder.multiply_small(10);
            let (digit, next) = remainder
                .div_rem(&self.denominator)
                .expect("normalized denominator is nonzero");
            fraction.push_str(&digit.to_string());
            remainder = next;
            if remainder.is_zero() {
                return Some(format!("{sign}{whole}.{fraction}"));
            }
        }
        None
    }
}

impl FromStr for BigRational {
    type Err = RationalError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let negative = raw.starts_with('-');
        let unsigned = raw.strip_prefix(['-', '+']).unwrap_or(raw);
        let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
        if whole.is_empty() && fraction.is_empty()
            || !whole.chars().all(|character| character.is_ascii_digit())
            || !fraction.chars().all(|character| character.is_ascii_digit())
        {
            return Err(RationalError::Invalid);
        }
        let digits = format!("{}{fraction}", if whole.is_empty() { "0" } else { whole });
        Self::new(
            BigInt::from_parts(
                negative,
                digits.parse().map_err(|_| RationalError::Invalid)?,
            ),
            BigUint::pow10(fraction.len()),
        )
    }
}

impl fmt::Display for BigRational {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.denominator == BigUint::one() {
            self.numerator.fmt(formatter)
        } else {
            write!(formatter, "{}/{}", self.numerator, self.denominator)
        }
    }
}

fn map_big(error: BigNumberError) -> RationalError {
    match error {
        BigNumberError::Invalid => RationalError::Invalid,
        BigNumberError::DivisionByZero => RationalError::DivisionByZero,
    }
}
