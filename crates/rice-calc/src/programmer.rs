use crate::{Decimal, DecimalError};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum IntegerBase {
    Binary,
    Octal,
    #[default]
    Decimal,
    Hexadecimal,
}

impl IntegerBase {
    #[must_use]
    pub const fn radix(self) -> u32 {
        match self {
            Self::Binary => 2,
            Self::Octal => 8,
            Self::Decimal => 10,
            Self::Hexadecimal => 16,
        }
    }

    #[must_use]
    pub fn format(self, value: i128) -> String {
        match self {
            Self::Binary => format!("{value:b}"),
            Self::Octal => format!("{value:o}"),
            Self::Decimal => value.to_string(),
            Self::Hexadecimal => format!("{value:X}"),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BitWidth {
    Bits8,
    Bits16,
    Bits32,
    Bits64,
    #[default]
    Bits128,
}

impl BitWidth {
    #[must_use]
    pub const fn bits(self) -> u32 {
        match self {
            Self::Bits8 => 8,
            Self::Bits16 => 16,
            Self::Bits32 => 32,
            Self::Bits64 => 64,
            Self::Bits128 => 128,
        }
    }

    #[must_use]
    pub fn mask(self, value: i128) -> i128 {
        if self == Self::Bits128 {
            value
        } else {
            let mask = (1_i128 << self.bits()) - 1;
            value & mask
        }
    }
}

pub(crate) fn integer(value: Decimal) -> Result<i128, DecimalError> {
    value.as_i128()
}

pub(crate) fn parse_literal(raw: &str) -> Option<Decimal> {
    let (digits, radix) = if let Some(value) = raw.strip_prefix("0x") {
        (value, 16)
    } else if let Some(value) = raw.strip_prefix("0o") {
        (value, 8)
    } else if let Some(value) = raw.strip_prefix("0b") {
        (value, 2)
    } else {
        return None;
    };
    i128::from_str_radix(digits, radix)
        .ok()
        .map(Decimal::from_i128)
}
