use crate::{
    BigRational, BitWidth, Decimal, EvaluationError, IntegerBase, evaluate, evaluate_exact,
};
use std::{collections::VecDeque, fmt};

const MAX_EXPRESSION_BYTES: usize = 4096;
const MAX_HISTORY: usize = 100;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Mode {
    #[default]
    Basic,
    Advanced,
    Financial,
    Programmer,
    Conversion,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AngleUnit {
    #[default]
    Degrees,
    Radians,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CalculatorError {
    ExpressionTooLong,
    Evaluation(EvaluationError),
    InvalidEntropy,
}

impl fmt::Display for CalculatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpressionTooLong => formatter.write_str("Expression is too long"),
            Self::Evaluation(error) => error.fmt(formatter),
            Self::InvalidEntropy => formatter.write_str("Entropy response is invalid"),
        }
    }
}

impl From<EvaluationError> for CalculatorError {
    fn from(value: EvaluationError) -> Self {
        Self::Evaluation(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Calculator {
    expression: String,
    result: Decimal,
    exact_result: BigRational,
    result_text: String,
    error: Option<CalculatorError>,
    memory: BigRational,
    history: VecDeque<(String, String)>,
    mode: Mode,
    angle: AngleUnit,
    base: IntegerBase,
    width: BitWidth,
}

impl Default for Calculator {
    fn default() -> Self {
        Self {
            expression: String::new(),
            result: Decimal::ZERO,
            exact_result: BigRational::zero(),
            result_text: String::from("0"),
            error: None,
            memory: BigRational::zero(),
            history: VecDeque::new(),
            mode: Mode::Basic,
            angle: AngleUnit::Degrees,
            base: IntegerBase::Decimal,
            width: BitWidth::Bits128,
        }
    }
}

impl Calculator {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn expression(&self) -> &str {
        &self.expression
    }

    #[must_use]
    pub const fn result(&self) -> Decimal {
        self.result
    }

    #[must_use]
    pub fn display(&self) -> String {
        self.error
            .as_ref()
            .map_or_else(|| self.result_text.clone(), ToString::to_string)
    }

    #[must_use]
    pub fn error(&self) -> Option<&CalculatorError> {
        self.error.as_ref()
    }

    #[must_use]
    pub fn history(&self) -> &VecDeque<(String, String)> {
        &self.history
    }

    #[must_use]
    pub fn memory(&self) -> &BigRational {
        &self.memory
    }

    #[must_use]
    pub const fn mode(&self) -> Mode {
        self.mode
    }

    #[must_use]
    pub const fn angle_unit(&self) -> AngleUnit {
        self.angle
    }

    #[must_use]
    pub const fn integer_base(&self) -> IntegerBase {
        self.base
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    pub fn set_angle_unit(&mut self, angle: AngleUnit) {
        self.angle = angle;
    }

    pub fn set_integer_base(&mut self, base: IntegerBase) {
        self.base = base;
    }

    pub fn set_bit_width(&mut self, width: BitWidth) {
        self.width = width;
    }

    pub fn replace_expression(&mut self, value: &str) -> Result<(), CalculatorError> {
        if value.len() > MAX_EXPRESSION_BYTES {
            return Err(CalculatorError::ExpressionTooLong);
        }
        value.clone_into(&mut self.expression);
        self.error = None;
        Ok(())
    }

    pub fn input(&mut self, value: &str) -> Result<(), CalculatorError> {
        if self.expression.len().saturating_add(value.len()) > MAX_EXPRESSION_BYTES {
            return Err(CalculatorError::ExpressionTooLong);
        }
        self.expression.push_str(value);
        self.error = None;
        Ok(())
    }

    pub fn backspace(&mut self) {
        self.expression.pop();
        self.error = None;
    }

    pub fn clear(&mut self) {
        self.expression.clear();
        self.result = Decimal::ZERO;
        self.exact_result = BigRational::zero();
        self.result_text = String::from("0");
        self.error = None;
    }

    pub fn evaluate(&mut self) -> Result<Decimal, CalculatorError> {
        self.calculate(true)
    }

    /// Recalculate the current expression without committing a history row.
    /// Incomplete input keeps the previous valid result and does not surface a
    /// transient parse error while the user is still typing.
    pub fn preview(&mut self) -> Result<Decimal, CalculatorError> {
        let result = self.calculate(false);
        if result.is_err() {
            self.error = None;
        }
        result
    }

    fn calculate(&mut self, record: bool) -> Result<Decimal, CalculatorError> {
        if let Ok(exact) = evaluate_exact(&self.expression) {
            let text = exact
                .finite_decimal(MAX_EXPRESSION_BYTES)
                .unwrap_or_else(|| exact.to_string());
            self.result = exact.decimal(12).parse().unwrap_or(Decimal::ZERO);
            self.exact_result = exact;
            text.clone_into(&mut self.result_text);
            if record {
                self.record_history(text);
            }
            self.error = None;
            return Ok(self.result);
        }
        match evaluate(&self.expression, self.angle == AngleUnit::Degrees) {
            Ok(result) => {
                self.result = result;
                self.exact_result = result
                    .to_string()
                    .parse()
                    .unwrap_or_else(|_| BigRational::zero());
                self.result_text = result.to_string();
                if record {
                    self.record_history(self.result_text.clone());
                }
                self.error = None;
                Ok(result)
            }
            Err(error) => {
                let error = CalculatorError::from(error);
                self.error = Some(error.clone());
                Err(error)
            }
        }
    }

    pub fn use_history(&mut self, index: usize) -> bool {
        let Some((expression, result)) = self.history.get(index).cloned() else {
            return false;
        };
        self.expression = expression;
        self.result_text.clone_from(&result);
        self.exact_result = result.parse().unwrap_or_else(|_| BigRational::zero());
        self.result = result.parse().unwrap_or(Decimal::ZERO);
        self.error = None;
        true
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    pub fn memory_clear(&mut self) {
        self.memory = BigRational::zero();
    }

    pub fn memory_store(&mut self) {
        self.memory = self.exact_result.clone();
    }

    pub fn memory_recall(&mut self) -> Result<(), CalculatorError> {
        let value = if self.expression.is_empty() {
            self.memory.to_string()
        } else {
            format!("({})", self.memory)
        };
        self.input(&value)
    }

    pub fn memory_add(&mut self) -> Result<(), CalculatorError> {
        self.memory = self
            .memory
            .add(&self.exact_result)
            .map_err(|_| CalculatorError::Evaluation(EvaluationError::Domain))?;
        Ok(())
    }

    pub fn memory_subtract(&mut self) -> Result<(), CalculatorError> {
        self.memory = self
            .memory
            .subtract(&self.exact_result)
            .map_err(|_| CalculatorError::Evaluation(EvaluationError::Domain))?;
        Ok(())
    }

    #[must_use]
    pub fn programmer_display(&self) -> String {
        let value = self.result.as_i128().unwrap_or(0);
        self.base.format(self.width.mask(value))
    }

    #[must_use]
    pub fn twos_complement_display(&self) -> String {
        let value = self.result.as_i128().unwrap_or(0);
        let encoded = self.width.mask(value);
        let signed = if self.width == BitWidth::Bits128 {
            value
        } else {
            let sign_bit = 1_i128 << (self.width.bits() - 1);
            if encoded & sign_bit == 0 {
                encoded
            } else {
                encoded - (1_i128 << self.width.bits())
            }
        };
        format!(
            "{}-bit · binary {:b} · hexadecimal {:X} · signed {signed}",
            self.width.bits(),
            encoded,
            encoded
        )
    }

    pub fn accept_entropy(&mut self, bytes: &[u8]) -> Result<Decimal, CalculatorError> {
        let bytes: [u8; 8] = bytes
            .get(..8)
            .ok_or(CalculatorError::InvalidEntropy)?
            .try_into()
            .map_err(|_| CalculatorError::InvalidEntropy)?;
        let mantissa = u64::from_le_bytes(bytes) >> 11;
        let result = Decimal::from_i128(i128::from(mantissa))
            .checked_div(Decimal::from_i128(1_i128 << 53))
            .map_err(|_| CalculatorError::InvalidEntropy)?;
        self.result = result;
        self.exact_result = result
            .to_string()
            .parse()
            .unwrap_or_else(|_| BigRational::zero());
        self.result_text = result.to_string();
        self.expression = String::from("random()");
        self.record_history(self.result_text.clone());
        Ok(result)
    }

    fn record_history(&mut self, result: String) {
        if self.expression.is_empty() {
            return;
        }
        if self.history.len() == MAX_HISTORY {
            self.history.pop_back();
        }
        self.history.push_front((self.expression.clone(), result));
    }
}
