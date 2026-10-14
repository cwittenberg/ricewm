use crate::{Decimal, DecimalError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FinancialError {
    InvalidPeriods,
    InvalidDomain,
    Arithmetic(DecimalError),
}

impl From<DecimalError> for FinancialError {
    fn from(value: DecimalError) -> Self {
        Self::Arithmetic(value)
    }
}

pub fn future_value(
    present: Decimal,
    periodic_rate: Decimal,
    periods: u32,
) -> Result<Decimal, FinancialError> {
    present
        .checked_mul(compound_factor(periodic_rate, periods)?)
        .map_err(Into::into)
}

pub fn present_value(
    future: Decimal,
    periodic_rate: Decimal,
    periods: u32,
) -> Result<Decimal, FinancialError> {
    future
        .checked_div(compound_factor(periodic_rate, periods)?)
        .map_err(Into::into)
}

pub fn payment(
    principal: Decimal,
    periodic_rate: Decimal,
    periods: u32,
) -> Result<Decimal, FinancialError> {
    if periods == 0 {
        return Err(FinancialError::InvalidPeriods);
    }
    if periodic_rate == Decimal::ZERO {
        return principal
            .checked_div(Decimal::from_i128(i128::from(periods)))
            .map_err(Into::into);
    }
    let factor = compound_factor(periodic_rate, periods)?;
    principal
        .checked_mul(periodic_rate)?
        .checked_mul(factor)?
        .checked_div(factor.checked_sub(Decimal::ONE)?)
        .map_err(Into::into)
}

pub fn periodic_interest_rate(
    present: Decimal,
    future: Decimal,
    periods: u32,
) -> Result<Decimal, FinancialError> {
    if present <= Decimal::ZERO || future < Decimal::ZERO || periods == 0 {
        return Err(FinancialError::InvalidDomain);
    }
    let ratio = future.checked_div(present)?.as_f64();
    Decimal::from_f64(ratio.powf(1.0 / f64::from(periods)) - 1.0).map_err(Into::into)
}

pub fn compounding_term(
    present: Decimal,
    future: Decimal,
    periodic_rate: Decimal,
) -> Result<Decimal, FinancialError> {
    if present <= Decimal::ZERO || future <= Decimal::ZERO || periodic_rate <= Decimal::ZERO {
        return Err(FinancialError::InvalidDomain);
    }
    let base = Decimal::ONE.checked_add(periodic_rate)?.as_f64();
    let periods = (future.checked_div(present)?.as_f64()).ln() / base.ln();
    Decimal::from_f64(periods).map_err(Into::into)
}

pub fn gross_profit_price(
    cost: Decimal,
    margin_percent: Decimal,
) -> Result<Decimal, FinancialError> {
    let hundred = Decimal::from_i128(100);
    if cost < Decimal::ZERO || margin_percent < Decimal::ZERO || margin_percent >= hundred {
        return Err(FinancialError::InvalidDomain);
    }
    cost.checked_mul(hundred)?
        .checked_div(hundred.checked_sub(margin_percent)?)
        .map_err(Into::into)
}

pub fn straight_line_depreciation(
    cost: Decimal,
    salvage: Decimal,
    periods: u32,
) -> Result<Decimal, FinancialError> {
    if periods == 0 || cost < salvage {
        return Err(FinancialError::InvalidDomain);
    }
    cost.checked_sub(salvage)?
        .checked_div(Decimal::from_i128(i128::from(periods)))
        .map_err(Into::into)
}

pub fn double_declining_depreciation(
    book_value: Decimal,
    useful_life_periods: u32,
) -> Result<Decimal, FinancialError> {
    if useful_life_periods == 0 || book_value < Decimal::ZERO {
        return Err(FinancialError::InvalidDomain);
    }
    book_value
        .checked_mul(Decimal::from_i128(2))?
        .checked_div(Decimal::from_i128(i128::from(useful_life_periods)))
        .map_err(Into::into)
}

pub fn sum_of_years_digits_depreciation(
    cost: Decimal,
    salvage: Decimal,
    useful_life_periods: u32,
    period: u32,
) -> Result<Decimal, FinancialError> {
    if useful_life_periods == 0 || period == 0 || period > useful_life_periods || cost < salvage {
        return Err(FinancialError::InvalidDomain);
    }
    let remaining = u64::from(useful_life_periods - period) + 1;
    let numerator = cost
        .checked_sub(salvage)?
        .checked_mul(Decimal::from_i128(i128::from(2 * remaining)))?;
    let life = u64::from(useful_life_periods);
    let denominator = life * (life + 1);
    numerator
        .checked_div(Decimal::from_i128(i128::from(denominator)))
        .map_err(Into::into)
}

fn compound_factor(rate: Decimal, periods: u32) -> Result<Decimal, FinancialError> {
    let base = Decimal::ONE.checked_add(rate)?;
    if base < Decimal::ZERO {
        return Err(FinancialError::InvalidDomain);
    }
    (0..periods)
        .try_fold(Decimal::ONE, |product, _| product.checked_mul(base))
        .map_err(Into::into)
}
