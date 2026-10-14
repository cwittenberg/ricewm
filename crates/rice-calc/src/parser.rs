use crate::{Decimal, DecimalError, programmer};
use std::{f64::consts, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvaluationError {
    Empty,
    InvalidToken,
    UnexpectedToken,
    MissingParenthesis,
    UnknownFunction,
    Domain,
    Decimal(DecimalError),
}

impl From<DecimalError> for EvaluationError {
    fn from(value: DecimalError) -> Self {
        Self::Decimal(value)
    }
}

impl fmt::Display for EvaluationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("Enter a calculation"),
            Self::InvalidToken => formatter.write_str("Invalid character"),
            Self::UnexpectedToken => formatter.write_str("Incomplete expression"),
            Self::MissingParenthesis => formatter.write_str("Missing parenthesis"),
            Self::UnknownFunction => formatter.write_str("Unknown function"),
            Self::Domain => formatter.write_str("Value is outside the function domain"),
            Self::Decimal(DecimalError::DivisionByZero) => formatter.write_str("Division by zero"),
            Self::Decimal(DecimalError::Overflow) => formatter.write_str("Number is too large"),
            Self::Decimal(_) => formatter.write_str("Invalid number"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Number(Decimal),
    Name(String),
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
    Remainder,
    Power,
    And,
    Or,
    Xor,
    ShiftLeft,
    ShiftRight,
}

pub fn evaluate(expression: &str, degrees: bool) -> Result<Decimal, EvaluationError> {
    let tokens = tokenize(expression)?;
    if tokens.is_empty() {
        return Err(EvaluationError::Empty);
    }
    let mut parser = Parser {
        tokens: &tokens,
        cursor: 0,
        degrees,
    };
    let result = parser.expression(0)?;
    if parser.cursor == tokens.len() {
        Ok(result)
    } else {
        Err(EvaluationError::UnexpectedToken)
    }
}

struct Parser<'a> {
    tokens: &'a [Token],
    cursor: usize,
    degrees: bool,
}

impl Parser<'_> {
    fn expression(&mut self, minimum: u8) -> Result<Decimal, EvaluationError> {
        let mut left = self.prefix()?;
        while let Some(Token::Operator(operator)) = self.tokens.get(self.cursor) {
            let (precedence, right_associative) = precedence(*operator);
            if precedence < minimum {
                break;
            }
            let operator = *operator;
            self.cursor += 1;
            let right = self.expression(precedence + u8::from(!right_associative))?;
            left = apply_operator(operator, left, right)?;
        }
        Ok(left)
    }

    fn prefix(&mut self) -> Result<Decimal, EvaluationError> {
        let mut value = match self.tokens.get(self.cursor) {
            Some(Token::Operator(Operator::Subtract)) => {
                self.cursor += 1;
                self.expression(7)?.checked_neg()?
            }
            Some(Token::Operator(Operator::Add)) => {
                self.cursor += 1;
                self.expression(7)?
            }
            Some(Token::Number(value)) => {
                self.cursor += 1;
                *value
            }
            Some(Token::Name(name)) => {
                let name = name.clone();
                self.cursor += 1;
                if self.tokens.get(self.cursor) == Some(&Token::Left) {
                    self.cursor += 1;
                    let argument = self.expression(0)?;
                    if self.tokens.get(self.cursor) != Some(&Token::Right) {
                        return Err(EvaluationError::MissingParenthesis);
                    }
                    self.cursor += 1;
                    apply_function(&name, argument, self.degrees)?
                } else {
                    constant(&name)?
                }
            }
            Some(Token::Left) => {
                self.cursor += 1;
                let nested = self.expression(0)?;
                if self.tokens.get(self.cursor) != Some(&Token::Right) {
                    return Err(EvaluationError::MissingParenthesis);
                }
                self.cursor += 1;
                nested
            }
            _ => return Err(EvaluationError::UnexpectedToken),
        };
        loop {
            match self.tokens.get(self.cursor) {
                Some(Token::Percent) => value = value.checked_percent()?,
                Some(Token::Factorial) => value = factorial(value)?,
                _ => break,
            }
            self.cursor += 1;
        }
        Ok(value)
    }
}

fn tokenize(expression: &str) -> Result<Vec<Token>, EvaluationError> {
    let characters = expression.char_indices().collect::<Vec<_>>();
    let mut cursor = 0;
    let mut tokens = Vec::new();
    while cursor < characters.len() {
        let (offset, character) = characters[cursor];
        if character.is_whitespace() {
            cursor += 1;
            continue;
        }
        let end_offset = |index: usize| {
            characters
                .get(index)
                .map_or(expression.len(), |(offset, _)| *offset)
        };
        if character.is_ascii_digit() || character == '.' {
            let start = offset;
            cursor += 1;
            while cursor < characters.len()
                && (characters[cursor].1.is_ascii_hexdigit()
                    || matches!(characters[cursor].1, '.' | 'x' | 'o' | 'b'))
            {
                cursor += 1;
            }
            let raw = &expression[start..end_offset(cursor)];
            let value = programmer::parse_literal(raw)
                .map_or_else(|| raw.parse::<Decimal>().map_err(EvaluationError::from), Ok)?;
            tokens.push(Token::Number(value));
            continue;
        }
        if character.is_alphabetic() || character == 'π' {
            let start = offset;
            cursor += 1;
            while cursor < characters.len() && characters[cursor].1.is_alphanumeric() {
                cursor += 1;
            }
            let name = expression[start..end_offset(cursor)].to_ascii_lowercase();
            let token = match name.as_str() {
                "and" => Token::Operator(Operator::And),
                "or" => Token::Operator(Operator::Or),
                "xor" => Token::Operator(Operator::Xor),
                "mod" => Token::Operator(Operator::Remainder),
                _ => Token::Name(name),
            };
            tokens.push(token);
            continue;
        }
        let next = characters.get(cursor + 1).map(|entry| entry.1);
        let (token, consumed) = match (character, next) {
            ('<', Some('<')) => (Token::Operator(Operator::ShiftLeft), 2),
            ('>', Some('>')) => (Token::Operator(Operator::ShiftRight), 2),
            ('+', _) => (Token::Operator(Operator::Add), 1),
            ('-' | '−', _) => (Token::Operator(Operator::Subtract), 1),
            ('*' | '×', _) => (Token::Operator(Operator::Multiply), 1),
            ('/' | '÷', _) => (Token::Operator(Operator::Divide), 1),
            ('^', _) => (Token::Operator(Operator::Power), 1),
            ('&', _) => (Token::Operator(Operator::And), 1),
            ('|', _) => (Token::Operator(Operator::Or), 1),
            ('%', _) => (Token::Percent, 1),
            ('!', _) => (Token::Factorial, 1),
            ('(', _) => (Token::Left, 1),
            (')', _) => (Token::Right, 1),
            _ => return Err(EvaluationError::InvalidToken),
        };
        tokens.push(token);
        cursor += consumed;
    }
    Ok(tokens)
}

const fn precedence(operator: Operator) -> (u8, bool) {
    match operator {
        Operator::Or => (1, false),
        Operator::Xor => (2, false),
        Operator::And => (3, false),
        Operator::ShiftLeft | Operator::ShiftRight => (4, false),
        Operator::Add | Operator::Subtract => (5, false),
        Operator::Multiply | Operator::Divide | Operator::Remainder => (6, false),
        Operator::Power => (8, true),
    }
}

fn apply_operator(
    operator: Operator,
    left: Decimal,
    right: Decimal,
) -> Result<Decimal, EvaluationError> {
    let value = match operator {
        Operator::Add => left.checked_add(right)?,
        Operator::Subtract => left.checked_sub(right)?,
        Operator::Multiply => left.checked_mul(right)?,
        Operator::Divide => left.checked_div(right)?,
        Operator::Remainder => left.checked_rem(right)?,
        Operator::Power => scientific(left.as_f64().powf(right.as_f64()))?,
        Operator::And => {
            Decimal::from_i128(programmer::integer(left)? & programmer::integer(right)?)
        }
        Operator::Or => {
            Decimal::from_i128(programmer::integer(left)? | programmer::integer(right)?)
        }
        Operator::Xor => {
            Decimal::from_i128(programmer::integer(left)? ^ programmer::integer(right)?)
        }
        Operator::ShiftLeft => {
            let shift =
                u32::try_from(programmer::integer(right)?).map_err(|_| EvaluationError::Domain)?;
            Decimal::from_i128(
                programmer::integer(left)?
                    .checked_shl(shift)
                    .ok_or(EvaluationError::Domain)?,
            )
        }
        Operator::ShiftRight => {
            let shift =
                u32::try_from(programmer::integer(right)?).map_err(|_| EvaluationError::Domain)?;
            Decimal::from_i128(
                programmer::integer(left)?
                    .checked_shr(shift)
                    .ok_or(EvaluationError::Domain)?,
            )
        }
    };
    Ok(value)
}

fn constant(name: &str) -> Result<Decimal, EvaluationError> {
    match name {
        "pi" | "π" => scientific(consts::PI),
        "e" => scientific(consts::E),
        _ => Err(EvaluationError::UnknownFunction),
    }
}

fn apply_function(name: &str, value: Decimal, degrees: bool) -> Result<Decimal, EvaluationError> {
    if name == "not" {
        return Ok(Decimal::from_i128(!programmer::integer(value)?));
    }
    let raw = value.as_f64();
    let angle = if degrees { raw.to_radians() } else { raw };
    let answer = match name {
        "sin" => angle.sin(),
        "cos" => angle.cos(),
        "tan" => angle.tan(),
        "asin" => raw.asin(),
        "acos" => raw.acos(),
        "atan" => raw.atan(),
        "sinh" => raw.sinh(),
        "cosh" => raw.cosh(),
        "tanh" => raw.tanh(),
        "asinh" => raw.asinh(),
        "acosh" => raw.acosh(),
        "atanh" => raw.atanh(),
        "sqrt" => raw.sqrt(),
        "ln" => raw.ln(),
        "log" | "log10" => raw.log10(),
        "exp" => raw.exp(),
        "abs" => raw.abs(),
        "floor" => raw.floor(),
        "ceil" => raw.ceil(),
        _ => return Err(EvaluationError::UnknownFunction),
    };
    let answer = if degrees && matches!(name, "asin" | "acos" | "atan") {
        answer.to_degrees()
    } else {
        answer
    };
    scientific(answer)
}

fn factorial(value: Decimal) -> Result<Decimal, EvaluationError> {
    let integer = programmer::integer(value)?;
    let maximum = u32::try_from(integer).map_err(|_| EvaluationError::Domain)?;
    let result = (2..=maximum).try_fold(1_i128, |product, factor| {
        product.checked_mul(i128::from(factor))
    });
    result
        .map(Decimal::from_i128)
        .ok_or(EvaluationError::Domain)
}

fn scientific(value: f64) -> Result<Decimal, EvaluationError> {
    if value.is_finite() {
        Decimal::from_f64(value).map_err(Into::into)
    } else {
        Err(EvaluationError::Domain)
    }
}
