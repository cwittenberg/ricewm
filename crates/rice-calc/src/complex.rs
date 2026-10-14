use crate::{BigRational, RationalError};
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Complex {
    real: BigRational,
    imaginary: BigRational,
}

impl Complex {
    #[must_use]
    pub fn new(real: BigRational, imaginary: BigRational) -> Self {
        Self { real, imaginary }
    }

    #[must_use]
    pub fn real(&self) -> &BigRational {
        &self.real
    }

    #[must_use]
    pub fn imaginary(&self) -> &BigRational {
        &self.imaginary
    }

    pub fn add(&self, rhs: &Self) -> Result<Self, RationalError> {
        Ok(Self::new(
            self.real.add(&rhs.real)?,
            self.imaginary.add(&rhs.imaginary)?,
        ))
    }

    pub fn subtract(&self, rhs: &Self) -> Result<Self, RationalError> {
        Ok(Self::new(
            self.real.subtract(&rhs.real)?,
            self.imaginary.subtract(&rhs.imaginary)?,
        ))
    }

    pub fn multiply(&self, rhs: &Self) -> Result<Self, RationalError> {
        let real = self
            .real
            .multiply(&rhs.real)?
            .subtract(&self.imaginary.multiply(&rhs.imaginary)?)?;
        let imaginary = self
            .real
            .multiply(&rhs.imaginary)?
            .add(&self.imaginary.multiply(&rhs.real)?)?;
        Ok(Self::new(real, imaginary))
    }

    pub fn divide(&self, rhs: &Self) -> Result<Self, RationalError> {
        let denominator = rhs
            .real
            .multiply(&rhs.real)?
            .add(&rhs.imaginary.multiply(&rhs.imaginary)?)?;
        let real = self
            .real
            .multiply(&rhs.real)?
            .add(&self.imaginary.multiply(&rhs.imaginary)?)?
            .divide(&denominator)?;
        let imaginary = self
            .imaginary
            .multiply(&rhs.real)?
            .subtract(&self.real.multiply(&rhs.imaginary)?)?
            .divide(&denominator)?;
        Ok(Self::new(real, imaginary))
    }

    #[must_use]
    pub fn conjugate(&self) -> Self {
        Self::new(self.real.clone(), self.imaginary.negate())
    }
}

impl fmt::Display for Complex {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.imaginary.numerator().is_negative() {
            write!(formatter, "{} - {}i", self.real, self.imaginary.negate())
        } else {
            write!(formatter, "{} + {}i", self.real, self.imaginary)
        }
    }
}
