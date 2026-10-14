//! Dependency-free calculation policy for `RiceCalc`.
//!
//! The crate deliberately contains no display, browser, clipboard, or network
//! code. Currency data enters through [`CurrencyRateProvider`], making stale or
//! invented exchange rates impossible at this layer.

#![allow(clippy::missing_errors_doc)]
#![allow(
    clippy::missing_panics_doc,
    reason = "internal expect calls document representation invariants, not caller-triggered panics"
)]

mod big_uint;
mod calculator;
mod complex;
mod conversion;
mod decimal;
mod exact_parser;
mod factor;
mod financial;
mod parser;
mod programmer;
mod rational;

pub use big_uint::{BigNumberError, BigUint};
pub use calculator::{AngleUnit, Calculator, CalculatorError, Mode};
pub use complex::Complex;
pub use conversion::{
    ConversionError, CurrencyRateProvider, Unit, UnitCategory, convert_currency, convert_unit,
};
pub use decimal::{Decimal, DecimalError};
pub use exact_parser::{ExactEvaluationError, evaluate_exact};
pub use factor::{FactorError, prime_factors};
pub use financial::{
    FinancialError, compounding_term, double_declining_depreciation, future_value,
    gross_profit_price, payment, periodic_interest_rate, present_value, straight_line_depreciation,
    sum_of_years_digits_depreciation,
};
pub use parser::{EvaluationError, evaluate};
pub use programmer::{BitWidth, IntegerBase};
pub use rational::{BigInt, BigRational, RationalError};
