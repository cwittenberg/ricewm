use rice_calc::{
    BigRational, Complex, Decimal, Unit, compounding_term, convert_unit,
    double_declining_depreciation, future_value, gross_profit_price, payment,
    periodic_interest_rate, present_value, prime_factors, straight_line_depreciation,
    sum_of_years_digits_depreciation,
};

pub(super) const CATEGORY_BASE: u64 = 600;
pub(super) const CATEGORY_COUNT: usize = 11;
pub(super) const CATEGORY_KEYS: [&str; CATEGORY_COUNT] = [
    "calc.category.length",
    "calc.category.area",
    "calc.category.volume",
    "calc.category.mass",
    "calc.category.temperature",
    "calc.category.speed",
    "calc.category.time",
    "calc.category.pressure",
    "calc.category.energy",
    "calc.category.data",
    "calc.category.angle",
];
const CATEGORY_DEFAULTS: [(&str, &str); CATEGORY_COUNT] = [
    ("km", "m"),
    ("ha", "m2"),
    ("l", "ml"),
    ("kg", "g"),
    ("c", "f"),
    ("km/h", "mph"),
    ("hour", "min"),
    ("bar", "psi"),
    ("kwh", "kj"),
    ("gb", "mb"),
    ("deg", "rad"),
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Workbench {
    pub financial: [String; 5],
    pub unit: [String; 3],
    pub complex: [String; 4],
    pub character: String,
    // Raw output remains stable for arithmetic and existing consumers.
    pub output: String,
    presentation: OutputPresentation,
    pub category: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum OutputPresentation {
    Literal,
    Error(WorkbenchError),
    Character(char),
}

impl Default for Workbench {
    fn default() -> Self {
        Self {
            financial: [
                String::from("1000"),
                String::from("0.05"),
                String::from("12"),
                String::from("100"),
                String::from("2000"),
            ],
            unit: [String::from("1"), String::from("km"), String::from("m")],
            complex: [
                String::from("1"),
                String::from("2"),
                String::from("3"),
                String::from("4"),
            ],
            character: String::new(),
            output: String::new(),
            presentation: OutputPresentation::Literal,
            category: 0,
        }
    }
}

impl Workbench {
    pub(super) fn set_literal(&mut self, value: String) {
        self.output = value;
        self.presentation = OutputPresentation::Literal;
    }

    pub(super) fn set_error(&mut self, error: WorkbenchError) {
        self.output = error.message().to_owned();
        self.presentation = OutputPresentation::Error(error);
    }

    pub(super) fn display_output(&self) -> String {
        match self.presentation {
            OutputPresentation::Literal => self.output.clone(),
            OutputPresentation::Error(error) => rice_i18n::t(error.key()).to_owned(),
            OutputPresentation::Character(character) => {
                let code = u32::from(character);
                let bytes = character.to_string().as_bytes().iter()
                    .map(|byte| format!("{byte:02X}")).collect::<Vec<_>>().join(" ");
                rice_i18n::format("calc.workbench.character-code", &[
                    ("hex", &format!("{code:04X}")), ("decimal", &code.to_string()), ("bytes", &bytes),
                ])
            }
        }
    }

    pub(super) fn select_category(&mut self, index: usize) -> bool {
        let Some((from, to)) = CATEGORY_DEFAULTS.get(index) else {
            return false;
        };
        self.category = index;
        self.unit[1] = (*from).to_owned();
        self.unit[2] = (*to).to_owned();
        self.set_literal(String::new());
        true
    }

    pub(super) fn financial(
        &mut self,
        operation: FinancialOperation,
    ) -> Result<(), WorkbenchError> {
        let principal = decimal(&self.financial[0])?;
        let rate = decimal(&self.financial[1])?;
        let periods = self.financial[2]
            .parse::<u32>()
            .map_err(|_| WorkbenchError::Periods)?;
        let salvage = decimal(&self.financial[3])?;
        let final_value = decimal(&self.financial[4])?;
        let result = match operation {
            FinancialOperation::Future => future_value(principal, rate, periods),
            FinancialOperation::Present => present_value(principal, rate, periods),
            FinancialOperation::Payment => payment(principal, rate, periods),
            FinancialOperation::StraightLine => {
                straight_line_depreciation(principal, salvage, periods)
            }
            FinancialOperation::DoubleDeclining => {
                double_declining_depreciation(principal, periods)
            }
            FinancialOperation::CompoundingTerm => compounding_term(principal, final_value, rate),
            FinancialOperation::GrossProfit => gross_profit_price(principal, rate),
            FinancialOperation::PeriodicRate => {
                periodic_interest_rate(principal, final_value, periods)
            }
            FinancialOperation::SumOfYearsDigits => {
                let period = self.financial[4]
                    .parse::<u32>()
                    .map_err(|_| WorkbenchError::Periods)?;
                sum_of_years_digits_depreciation(principal, salvage, periods, period)
            }
        }
        .map_err(|_| WorkbenchError::Financial)?;
        self.set_literal(result.to_string());
        Ok(())
    }

    pub(super) fn convert_unit(&mut self) -> Result<(), WorkbenchError> {
        let amount = decimal(&self.unit[0])?;
        let from = Unit::parse(&self.unit[1]).ok_or(WorkbenchError::Unit)?;
        let to = Unit::parse(&self.unit[2]).ok_or(WorkbenchError::Unit)?;
        let result = convert_unit(amount, from, to).map_err(|_| WorkbenchError::Unit)?;
        self.set_literal(result.to_string());
        Ok(())
    }

    pub(super) fn complex(&mut self, operation: ComplexOperation) -> Result<(), WorkbenchError> {
        let left = Complex::new(rational(&self.complex[0])?, rational(&self.complex[1])?);
        let right = Complex::new(rational(&self.complex[2])?, rational(&self.complex[3])?);
        let result = match operation {
            ComplexOperation::Add => left.add(&right),
            ComplexOperation::Subtract => left.subtract(&right),
            ComplexOperation::Multiply => left.multiply(&right),
            ComplexOperation::Divide => left.divide(&right),
            ComplexOperation::Conjugate => Ok(left.conjugate()),
        }
        .map_err(|_| WorkbenchError::Complex)?;
        self.set_literal(result.to_string());
        Ok(())
    }

    pub(super) fn character_code(&mut self) -> Result<(), WorkbenchError> {
        let mut characters = self.character.chars();
        let character = characters.next().ok_or(WorkbenchError::Character)?;
        if characters.next().is_some() {
            return Err(WorkbenchError::Character);
        }
        self.presentation = OutputPresentation::Character(character);
        self.output = format!(
            "U+{:04X} · decimal {} · UTF-8 {}",
            u32::from(character),
            u32::from(character),
            character
                .to_string()
                .as_bytes()
                .iter()
                .map(|byte| format!("{byte:02X}"))
                .collect::<Vec<_>>()
                .join(" ")
        );
        Ok(())
    }

    pub(super) fn factor(&mut self, raw: &str) -> Result<(), WorkbenchError> {
        let value = raw.parse().map_err(|_| WorkbenchError::Integer)?;
        let output = prime_factors(&value)
            .map_err(|_| WorkbenchError::Integer)?
            .into_iter()
            .map(|factor| factor.to_string())
            .collect::<Vec<_>>()
            .join(" × ");
        self.set_literal(output);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FinancialOperation {
    Future,
    Present,
    Payment,
    StraightLine,
    DoubleDeclining,
    CompoundingTerm,
    GrossProfit,
    PeriodicRate,
    SumOfYearsDigits,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ComplexOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Conjugate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WorkbenchError {
    Decimal,
    Periods,
    Financial,
    Unit,
    Complex,
    Character,
    Integer,
    WholeExpression,
}

impl WorkbenchError {
    pub(super) const fn key(self) -> &'static str {
        match self {
            Self::Decimal => "calc.workbench.error.decimal",
            Self::Periods => "calc.workbench.error.periods",
            Self::Financial => "calc.workbench.error.financial",
            Self::Unit => "calc.workbench.error.unit",
            Self::Complex => "calc.workbench.error.complex",
            Self::Character => "calc.workbench.error.character",
            Self::Integer => "calc.workbench.error.integer",
            Self::WholeExpression => "calc.workbench.error.whole-expression",
        }
    }

    pub(super) const fn message(self) -> &'static str {
        match self {
            Self::Decimal => "Enter valid decimal values.",
            Self::Periods => "Periods must be a positive whole number.",
            Self::Financial => "Those values do not define a valid financial calculation.",
            Self::Unit => "Use compatible unit symbols, such as km and m.",
            Self::Complex => "Enter valid rational complex-number components.",
            Self::Character => "Enter exactly one Unicode character.",
            Self::Integer => "Enter a whole number greater than one.",
            Self::WholeExpression => "Enter a valid whole-number expression.",
        }
    }
}

fn decimal(raw: &str) -> Result<Decimal, WorkbenchError> {
    raw.parse().map_err(|_| WorkbenchError::Decimal)
}

fn rational(raw: &str) -> Result<BigRational, WorkbenchError> {
    raw.parse().map_err(|_| WorkbenchError::Complex)
}
