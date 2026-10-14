use crate::{BigNumberError, BigUint};

const WHEEL: [u32; 8] = [4, 2, 4, 2, 4, 6, 2, 6];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FactorError {
    ZeroHasNoPrimeFactorization,
    SearchLimit,
    Arithmetic(BigNumberError),
}

impl From<BigNumberError> for FactorError {
    fn from(value: BigNumberError) -> Self {
        Self::Arithmetic(value)
    }
}

pub fn prime_factors(value: &BigUint) -> Result<Vec<BigUint>, FactorError> {
    if value.is_zero() {
        return Err(FactorError::ZeroHasNoPrimeFactorization);
    }
    let mut remaining = value.clone();
    let mut factors = Vec::new();
    for candidate in [2_u32, 3, 5] {
        divide_repeatedly(&mut remaining, candidate, &mut factors)?;
    }
    let mut candidate = 7_u32;
    let mut wheel_index = 0;
    loop {
        let candidate_big = BigUint::from_u64(u64::from(candidate));
        if candidate_big.multiply(&candidate_big) > remaining {
            break;
        }
        divide_repeatedly(&mut remaining, candidate, &mut factors)?;
        candidate = candidate
            .checked_add(WHEEL[wheel_index])
            .ok_or(FactorError::SearchLimit)?;
        wheel_index = (wheel_index + 1) % WHEEL.len();
    }
    if remaining != BigUint::one() {
        factors.push(remaining);
    }
    Ok(factors)
}

fn divide_repeatedly(
    remaining: &mut BigUint,
    divisor: u32,
    factors: &mut Vec<BigUint>,
) -> Result<(), FactorError> {
    loop {
        let (quotient, remainder) = remaining.div_rem_small(divisor)?;
        if remainder != 0 {
            return Ok(());
        }
        factors.push(BigUint::from_u64(u64::from(divisor)));
        *remaining = quotient;
    }
}
