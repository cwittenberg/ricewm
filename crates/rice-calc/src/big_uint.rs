use std::{cmp::Ordering, fmt, str::FromStr};

const BASE: u64 = 1_000_000_000;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BigUint {
    limbs: Vec<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BigNumberError {
    Invalid,
    DivisionByZero,
}

impl BigUint {
    #[must_use]
    pub fn zero() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn one() -> Self {
        Self { limbs: vec![1] }
    }

    #[must_use]
    pub fn from_u64(value: u64) -> Self {
        if value == 0 {
            return Self::zero();
        }
        let low = u32::try_from(value % BASE).expect("limb is below base");
        let high = u32::try_from(value / BASE).expect("u64 high limb fits");
        if high == 0 {
            Self { limbs: vec![low] }
        } else {
            Self {
                limbs: vec![low, high],
            }
        }
    }

    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    #[must_use]
    pub fn add(&self, rhs: &Self) -> Self {
        let length = self.limbs.len().max(rhs.limbs.len());
        let mut limbs = Vec::with_capacity(length + 1);
        let mut carry = 0_u64;
        for index in 0..length {
            let sum = u64::from(*self.limbs.get(index).unwrap_or(&0))
                + u64::from(*rhs.limbs.get(index).unwrap_or(&0))
                + carry;
            limbs.push(u32::try_from(sum % BASE).expect("limb is below base"));
            carry = sum / BASE;
        }
        if carry != 0 {
            limbs.push(u32::try_from(carry).expect("carry is below base"));
        }
        Self { limbs }
    }

    #[must_use]
    pub fn subtract(&self, rhs: &Self) -> Option<Self> {
        if self < rhs {
            return None;
        }
        let mut limbs = Vec::with_capacity(self.limbs.len());
        let mut borrow = 0_i64;
        for index in 0..self.limbs.len() {
            let mut value = i64::from(self.limbs[index])
                - i64::from(*rhs.limbs.get(index).unwrap_or(&0))
                - borrow;
            if value < 0 {
                value += i64::try_from(BASE).expect("base fits");
                borrow = 1;
            } else {
                borrow = 0;
            }
            limbs.push(u32::try_from(value).expect("normalized limb is positive"));
        }
        Some(normalized(limbs))
    }

    #[must_use]
    pub fn multiply(&self, rhs: &Self) -> Self {
        if self.is_zero() || rhs.is_zero() {
            return Self::zero();
        }
        let mut limbs = vec![0_u32; self.limbs.len() + rhs.limbs.len()];
        for (left_index, left) in self.limbs.iter().enumerate() {
            let mut carry = 0_u64;
            for (right_index, right) in rhs.limbs.iter().enumerate() {
                let index = left_index + right_index;
                let value = u64::from(limbs[index]) + u64::from(*left) * u64::from(*right) + carry;
                limbs[index] = u32::try_from(value % BASE).expect("limb below base");
                carry = value / BASE;
            }
            let mut index = left_index + rhs.limbs.len();
            while carry != 0 {
                let value = u64::from(limbs[index]) + carry;
                limbs[index] = u32::try_from(value % BASE).expect("limb below base");
                carry = value / BASE;
                index += 1;
                if index == limbs.len() && carry != 0 {
                    limbs.push(0);
                }
            }
        }
        normalized(limbs)
    }

    pub fn div_rem(&self, divisor: &Self) -> Result<(Self, Self), BigNumberError> {
        if divisor.is_zero() {
            return Err(BigNumberError::DivisionByZero);
        }
        if self < divisor {
            return Ok((Self::zero(), self.clone()));
        }
        let mut quotient = vec![0_u32; self.limbs.len()];
        let mut remainder = Self::zero();
        for index in (0..self.limbs.len()).rev() {
            remainder.shift_base(self.limbs[index]);
            let mut low = 0_u64;
            let mut high = BASE - 1;
            let mut digit = 0_u64;
            while low <= high {
                let middle = low + (high - low) / 2;
                let product = divisor.multiply_small(middle);
                if product <= remainder {
                    digit = middle;
                    low = middle + 1;
                } else if middle == 0 {
                    break;
                } else {
                    high = middle - 1;
                }
            }
            if digit != 0 {
                remainder = remainder
                    .subtract(&divisor.multiply_small(digit))
                    .expect("selected quotient digit does not exceed remainder");
            }
            quotient[index] = u32::try_from(digit).expect("digit below base");
        }
        Ok((normalized(quotient), remainder))
    }

    pub fn div_rem_small(&self, divisor: u32) -> Result<(Self, u32), BigNumberError> {
        if divisor == 0 {
            return Err(BigNumberError::DivisionByZero);
        }
        let mut quotient = vec![0_u32; self.limbs.len()];
        let mut remainder = 0_u64;
        for index in (0..self.limbs.len()).rev() {
            let value = remainder * BASE + u64::from(self.limbs[index]);
            quotient[index] =
                u32::try_from(value / u64::from(divisor)).expect("quotient limb below base");
            remainder = value % u64::from(divisor);
        }
        Ok((
            normalized(quotient),
            u32::try_from(remainder).expect("remainder below divisor"),
        ))
    }

    #[must_use]
    pub fn gcd(mut left: Self, mut right: Self) -> Self {
        while !right.is_zero() {
            let (_, remainder) = left
                .div_rem(&right)
                .expect("nonzero Euclidean divisor is valid");
            left = right;
            right = remainder;
        }
        left
    }

    #[must_use]
    pub fn pow10(power: usize) -> Self {
        let mut result = Self::one();
        for _ in 0..power {
            result = result.multiply_small(10);
        }
        result
    }

    #[must_use]
    pub fn multiply_small(&self, rhs: u64) -> Self {
        if rhs == 0 || self.is_zero() {
            return Self::zero();
        }
        let mut limbs = Vec::with_capacity(self.limbs.len() + 1);
        let mut carry = 0_u64;
        for limb in &self.limbs {
            let value = u64::from(*limb) * rhs + carry;
            limbs.push(u32::try_from(value % BASE).expect("limb below base"));
            carry = value / BASE;
        }
        while carry != 0 {
            limbs.push(u32::try_from(carry % BASE).expect("limb below base"));
            carry /= BASE;
        }
        Self { limbs }
    }

    #[must_use]
    pub fn to_u64(&self) -> Option<u64> {
        let mut value = 0_u64;
        for limb in self.limbs.iter().rev() {
            value = value.checked_mul(BASE)?.checked_add(u64::from(*limb))?;
        }
        Some(value)
    }

    fn shift_base(&mut self, low: u32) {
        if self.is_zero() && low == 0 {
            return;
        }
        self.limbs.insert(0, low);
    }
}

impl Ord for BigUint {
    fn cmp(&self, rhs: &Self) -> Ordering {
        self.limbs
            .len()
            .cmp(&rhs.limbs.len())
            .then_with(|| self.limbs.iter().rev().cmp(rhs.limbs.iter().rev()))
    }
}

impl PartialOrd for BigUint {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}

impl FromStr for BigUint {
    type Err = BigNumberError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        if raw.is_empty() || !raw.chars().all(|character| character.is_ascii_digit()) {
            return Err(BigNumberError::Invalid);
        }
        let mut value = Self::zero();
        for byte in raw.bytes() {
            value = value
                .multiply_small(10)
                .add(&Self::from_u64(u64::from(byte - b'0')));
        }
        Ok(value)
    }
}

impl fmt::Display for BigUint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some(last) = self.limbs.last() else {
            return formatter.write_str("0");
        };
        last.fmt(formatter)?;
        for limb in self.limbs.iter().rev().skip(1) {
            write!(formatter, "{limb:09}")?;
        }
        Ok(())
    }
}

fn normalized(mut limbs: Vec<u32>) -> BigUint {
    while limbs.last() == Some(&0) {
        limbs.pop();
    }
    BigUint { limbs }
}
