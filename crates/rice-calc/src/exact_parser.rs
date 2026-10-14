use crate::{BigInt, BigRational, BigUint, RationalError};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactEvaluationError {
    Empty,
    InvalidToken,
    UnexpectedToken,
    MissingParenthesis,
    Domain,
    Rational(RationalError),
}

impl From<RationalError> for ExactEvaluationError {
    fn from(value: RationalError) -> Self {
        Self::Rational(value)
    }
}

impl fmt::Display for ExactEvaluationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("Enter a calculation"),
            Self::InvalidToken => formatter.write_str("Invalid exact-number character"),
            Self::UnexpectedToken => formatter.write_str("Incomplete exact expression"),
            Self::MissingParenthesis => formatter.write_str("Missing parenthesis"),
            Self::Domain => formatter.write_str("Value is outside the exact-number domain"),
            Self::Rational(RationalError::DivisionByZero) => {
                formatter.write_str("Division by zero")
            }
            Self::Rational(_) => formatter.write_str("Invalid exact number"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Token {
    Number(BigRational),
    Operator(Operator),
    Left,
    Right,
    Percent,
    Factorial,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Operator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
}

pub fn evaluate_exact(expression: &str) -> Result<BigRational, ExactEvaluationError> {
    let tokens = tokenize(expression)?;
    if tokens.is_empty() {
        return Err(ExactEvaluationError::Empty);
    }
    let mut parser = Parser {
        tokens: &tokens,
        cursor: 0,
    };
    let result = parser.expression(0)?;
    if parser.cursor == tokens.len() {
        Ok(result)
    } else {
        Err(ExactEvaluationError::UnexpectedToken)
    }
}

struct Parser<'a> {
    tokens: &'a [Token],
    cursor: usize,
}

impl Parser<'_> {
    fn expression(&mut self, minimum: u8) -> Result<BigRational, ExactEvaluationError> {
        let mut left = self.prefix()?;
        while let Some(Token::Operator(operator)) = self.tokens.get(self.cursor) {
            let (precedence, right_associative) = precedence(*operator);
            if precedence < minimum {
                break;
            }
            let operator = *operator;
            self.cursor += 1;
            let right = self.expression(precedence + u8::from(!right_associative))?;
            left = apply(operator, &left, &right)?;
        }
        Ok(left)
    }

    fn prefix(&mut self) -> Result<BigRational, ExactEvaluationError> {
        let mut value = match self.tokens.get(self.cursor) {
            Some(Token::Operator(Operator::Subtract)) => {
                self.cursor += 1;
                self.expression(7)?.negate()
            }
            Some(Token::Operator(Operator::Add)) => {
                self.cursor += 1;
                self.expression(7)?
            }
            Some(Token::Number(value)) => {
                self.cursor += 1;
                value.clone()
            }
            Some(Token::Left) => {
                self.cursor += 1;
                let nested = self.expression(0)?;
                if self.tokens.get(self.cursor) != Some(&Token::Right) {
                    return Err(ExactEvaluationError::MissingParenthesis);
                }
                self.cursor += 1;
                nested
            }
            _ => return Err(ExactEvaluationError::UnexpectedToken),
        };
        loop {
            match self.tokens.get(self.cursor) {
                Some(Token::Percent) => {
                    value = value.divide(&"100".parse().expect("constant rational"))?;
                }
                Some(Token::Factorial) => value = factorial(&value)?,
                _ => break,
            }
            self.cursor += 1;
        }
        Ok(value)
    }
}

fn tokenize(expression: &str) -> Result<Vec<Token>, ExactEvaluationError> {
    let characters = expression.char_indices().collect::<Vec<_>>();
    let mut cursor = 0;
    let mut tokens = Vec::new();
    while cursor < characters.len() {
        let (offset, character) = characters[cursor];
        if character.is_whitespace() {
            cursor += 1;
            continue;
        }
        if character.is_ascii_digit() || character == '.' {
            let start = offset;
            cursor += 1;
            while cursor < characters.len()
                && (characters[cursor].1.is_ascii_digit() || characters[cursor].1 == '.')
            {
                cursor += 1;
            }
            let end = characters
                .get(cursor)
                .map_or(expression.len(), |(offset, _)| *offset);
            tokens.push(Token::Number(
                expression[start..end]
                    .parse()
                    .map_err(ExactEvaluationError::from)?,
            ));
            continue;
        }
        tokens.push(match character {
            '+' => Token::Operator(Operator::Add),
            '-' | '−' => Token::Operator(Operator::Subtract),
            '*' | '×' => Token::Operator(Operator::Multiply),
            '/' | '÷' => Token::Operator(Operator::Divide),
            '^' => Token::Operator(Operator::Power),
            '%' => Token::Percent,
            '!' => Token::Factorial,
            '(' => Token::Left,
            ')' => Token::Right,
            _ => return Err(ExactEvaluationError::InvalidToken),
        });
        cursor += 1;
    }
    Ok(tokens)
}

const fn precedence(operator: Operator) -> (u8, bool) {
    match operator {
        Operator::Add | Operator::Subtract => (5, false),
        Operator::Multiply | Operator::Divide => (6, false),
        Operator::Power => (8, true),
    }
}

fn apply(
    operator: Operator,
    left: &BigRational,
    right: &BigRational,
) -> Result<BigRational, ExactEvaluationError> {
    match operator {
        Operator::Add => left.add(right).map_err(Into::into),
        Operator::Subtract => left.subtract(right).map_err(Into::into),
        Operator::Multiply => left.multiply(right).map_err(Into::into),
        Operator::Divide => left.divide(right).map_err(Into::into),
        Operator::Power => {
            let exponent = right.integer()?;
            let magnitude = exponent
                .magnitude()
                .to_u64()
                .and_then(|value| i64::try_from(value).ok())
                .ok_or(ExactEvaluationError::Domain)?;
            left.pow(if exponent.is_negative() {
                -magnitude
            } else {
                magnitude
            })
            .map_err(Into::into)
        }
    }
}

fn factorial(value: &BigRational) -> Result<BigRational, ExactEvaluationError> {
    let integer = value.integer()?;
    if integer.is_negative() {
        return Err(ExactEvaluationError::Domain);
    }
    let maximum = integer
        .magnitude()
        .to_u64()
        .ok_or(ExactEvaluationError::Domain)?;
    if maximum > 100_000 {
        return Err(ExactEvaluationError::Domain);
    }
    let mut product = BigUint::one();
    for factor in 2..=maximum {
        product = product.multiply(&BigUint::from_u64(factor));
    }
    Ok(BigRational::from_integer(BigInt::from_parts(
        false, product,
    )))
}
