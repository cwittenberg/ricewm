//! RiceCalc actor: keyboard/controller behavior over the pure calculation core.

#[path = "calc/calc_ui.rs"]
mod calc_ui;
mod menus;
mod panels;
mod workbench;

use calc_ui::*;
use rice_calc::{AngleUnit, BitWidth, Calculator, CalculatorError, Decimal, IntegerBase, Mode};
use rice_native_app_protocol::{
    AppId, ClipboardRequest, ClipboardSelection, DataProviderOperation, DataProviderRequest,
    DataProviderResponse, DataProviderResult, HostEvent, HostEventKind, Message, RequestId,
    UiEvent, UiEventEnvelope, WindowId,
};
use rice_ui::{PatchConversionError, RetainedTree, TreeError};
use workbench::{CATEGORY_BASE, CATEGORY_COUNT, ComplexOperation, FinancialOperation, Workbench};

pub(crate) const ENTRYPOINT: &str = "rice-calc";
const THEME_CHANGED_TOPIC: &str = "org.ricewm.theme/changed";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DialogKind {
    Shortcuts,
    About,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiceCalc {
    app: AppId,
    window: WindowId,
    sequence: u64,
    calculator: Calculator,
    redmond98: bool,
    history_visible: bool,
    programmer_scroll: u32,
    entropy_available: bool,
    currency_available: bool,
    pending_provider: Option<ProviderIntent>,
    currency_amount: String,
    currency_base: String,
    currency_quote: String,
    provider_status: ProviderStatus,
    workbench: Workbench,
    dialog: Option<DialogKind>,
    tree: RetainedTree,
}

// Provider diagnostics and numeric results are literal; owned notices retain identity.
#[derive(Clone, Debug, Eq, PartialEq)]
enum ProviderStatus {
    Literal(String),
    Key(&'static str),
}
impl ProviderStatus {
    fn render(&self) -> &str {
        match self {
            Self::Literal(value) => value,
            Self::Key(key) => rice_i18n::t(key),
        }
    }
}

impl RiceCalc {
    pub(crate) fn launch(
        app: AppId,
        arguments: &[String],
    ) -> Result<(Self, Message), CalcActorError> {
        let mut actor = Self {
            app,
            window: window_id(),
            sequence: 1,
            calculator: Calculator::new(),
            redmond98: arguments
                .iter()
                .any(|value| value == "--theme-profile=redmond-98"),
            history_visible: false,
            programmer_scroll: 0,
            entropy_available: arguments.iter().any(|value| value == "--entropy-provider"),
            currency_available: arguments.iter().any(|value| value == "--currency-provider"),
            pending_provider: None,
            currency_amount: String::from("1"),
            currency_base: String::from("EUR"),
            currency_quote: String::from("USD"),
            provider_status: ProviderStatus::Literal(String::new()),
            workbench: Workbench::default(),
            dialog: None,
            tree: RetainedTree::default(),
        };
        let patches = full_patches(&actor);
        actor.tree.apply(&patches)?;
        Ok((actor, message(app, window_id(), 1, &patches)?))
    }

    pub(crate) fn handle_event(
        &mut self,
        envelope: &UiEventEnvelope,
        request: RequestId,
    ) -> Result<Vec<Message>, CalcActorError> {
        if envelope.app != self.app || envelope.window != self.window {
            return Ok(Vec::new());
        }
        let mut extra = Vec::new();
        let changed = match &envelope.event {
            UiEvent::Pointer {
                kind: rice_native_app_protocol::PointerEventKind::Wheel,
                button,
                ..
            } if envelope.target.get() == panels::PROGRAMMER_PANEL => {
                self.programmer_scroll = if *button == 4 {
                    self.programmer_scroll.saturating_sub(96)
                } else {
                    self.programmer_scroll.saturating_add(96).min(1_000)
                };
                true
            }
            UiEvent::ValueChanged(value) if envelope.target.get() == EXPRESSION => {
                self.calculator.replace_expression(value)?;
                let _ = self.calculator.preview();
                true
            }
            UiEvent::ValueChanged(value) if envelope.target.get() == panels::PROGRAMMER_PANEL => {
                self.programmer_scroll = value
                    .split_once('/')
                    .and_then(|(position, _)| position.parse().ok())
                    .unwrap_or(self.programmer_scroll);
                true
            }
            UiEvent::ValueChanged(value) if envelope.target.get() == CURRENCY_AMOUNT => {
                value.clone_into(&mut self.currency_amount);
                true
            }
            UiEvent::ValueChanged(value) if envelope.target.get() == CURRENCY_BASE => {
                value
                    .to_ascii_uppercase()
                    .clone_into(&mut self.currency_base);
                true
            }
            UiEvent::ValueChanged(value) if envelope.target.get() == CURRENCY_QUOTE => {
                value
                    .to_ascii_uppercase()
                    .clone_into(&mut self.currency_quote);
                true
            }
            UiEvent::ValueChanged(value) => self.update_workbench(envelope.target.get(), value),
            UiEvent::Key {
                pressed: true,
                key_code,
                modifiers,
                text,
            } if alt_pressed(*modifiers) && key_matches(*key_code, text.as_deref(), 'c') => {
                extra.push(self.copy_message(request));
                false
            }
            UiEvent::Key {
                pressed: true,
                key_code,
                modifiers,
                text,
            } if alt_pressed(*modifiers) && key_matches(*key_code, text.as_deref(), 'h') => {
                self.history_visible = !self.history_visible;
                true
            }
            UiEvent::Key {
                pressed: true,
                modifiers,
                text,
                ..
            } if alt_pressed(*modifiers) && text.as_deref() == Some("?") => {
                self.dialog = Some(DialogKind::Shortcuts);
                true
            }
            UiEvent::Key {
                pressed: true,
                key_code,
                text,
                ..
            } => self.handle_key(*key_code, text.as_deref())?,
            UiEvent::Activate => {
                if envelope.target.get() == RANDOM && self.entropy_available {
                    self.pending_provider = Some(ProviderIntent::Entropy {
                        request,
                        expected: 8,
                    });
                    extra.push(Message::DataProviderRequest(DataProviderRequest {
                        request,
                        app: self.app,
                        operation: DataProviderOperation::Entropy { byte_len: 8 },
                    }));
                    false
                } else if envelope.target.get() == CURRENCY_CONVERT && self.currency_available {
                    if !valid_iso_code(&self.currency_base) || !valid_iso_code(&self.currency_quote)
                    {
                        self.provider_status =
                            ProviderStatus::Key("calc.status.invalid-currency-code");
                        return Ok(vec![self.emit()?]);
                    }
                    self.pending_provider = Some(ProviderIntent::Currency { request });
                    extra.push(Message::DataProviderRequest(DataProviderRequest {
                        request,
                        app: self.app,
                        operation: DataProviderOperation::CurrencyRate {
                            base_iso4217: self.currency_base.clone(),
                            quote_iso4217: self.currency_quote.clone(),
                        },
                    }));
                    false
                } else if envelope.target.get() == COPY {
                    extra.push(self.copy_message(request));
                    false
                } else {
                    self.activate(envelope.target.get())?
                }
            }
            _ => false,
        };
        if changed {
            extra.insert(0, self.emit()?);
        }
        Ok(extra)
    }

    pub(crate) fn handle_host_event(
        &mut self,
        event: &HostEvent,
    ) -> Result<Vec<Message>, CalcActorError> {
        if event.app != self.app {
            return Ok(Vec::new());
        }
        let HostEventKind::RiceBus { topic, payload } = &event.event else {
            return Ok(Vec::new());
        };
        if topic != THEME_CHANGED_TOPIC {
            return Ok(Vec::new());
        }
        let redmond98 = payload == "redmond-98";
        if redmond98 == self.redmond98 {
            return Ok(Vec::new());
        }
        self.redmond98 = redmond98;
        self.emit().map(|message| vec![message])
    }

    pub(crate) fn handle_data_response(
        &mut self,
        response: &DataProviderResponse,
    ) -> Result<Vec<Message>, CalcActorError> {
        let Some(pending) = self.pending_provider.take() else {
            return Err(CalcActorError::UnexpectedProviderResponse);
        };
        if pending.request() != response.request {
            self.pending_provider = Some(pending);
            return Err(CalcActorError::UnexpectedProviderResponse);
        }
        match (pending, &response.result) {
            (ProviderIntent::Entropy { expected, .. }, DataProviderResult::Entropy(bytes))
                if bytes.len() == expected =>
            {
                self.calculator.accept_entropy(bytes)?;
                self.provider_status = ProviderStatus::Key("calc.status.secure-random");
            }
            (
                ProviderIntent::Currency { .. },
                DataProviderResult::CurrencyRate {
                    decimal_rate,
                    observed_millis,
                    source,
                },
            ) => {
                let amount: Decimal = self
                    .currency_amount
                    .parse()
                    .map_err(|_| CalcActorError::InvalidProviderData)?;
                let rate: Decimal = decimal_rate
                    .parse()
                    .map_err(|_| CalcActorError::InvalidProviderData)?;
                if rate <= Decimal::ZERO {
                    return Err(CalcActorError::InvalidProviderData);
                }
                let converted = amount
                    .checked_mul(rate)
                    .map_err(|_| CalcActorError::InvalidProviderData)?;
                self.provider_status = ProviderStatus::Literal(format!(
                    "{} {} = {} {} · {} · {}",
                    self.currency_amount,
                    self.currency_base,
                    converted,
                    self.currency_quote,
                    source,
                    observed_millis
                ));
            }
            (_, DataProviderResult::Unavailable { reason }) => {
                self.provider_status = ProviderStatus::Literal(reason.clone());
            }
            _ => return Err(CalcActorError::InvalidProviderData),
        }
        Ok(vec![self.emit()?])
    }

    fn handle_key(&mut self, key_code: u32, text: Option<&str>) -> Result<bool, CalcActorError> {
        match key_code {
            13 | 36 | 104 => {
                let _ = self.calculator.evaluate();
                Ok(true)
            }
            8 | 22 => {
                self.calculator.backspace();
                let _ = self.calculator.preview();
                Ok(true)
            }
            9 | 27 => {
                self.calculator.clear();
                Ok(true)
            }
            _ => {
                let Some(text) = text.filter(|text| allowed_keyboard_text(text)) else {
                    return Ok(false);
                };
                self.calculator.input(text)?;
                let _ = self.calculator.preview();
                Ok(true)
            }
        }
    }

    fn activate(&mut self, target: u64) -> Result<bool, CalcActorError> {
        if let Some(index) = target
            .checked_sub(CATEGORY_BASE)
            .and_then(|value| usize::try_from(value).ok())
            .filter(|index| *index < CATEGORY_COUNT)
        {
            return Ok(self.workbench.select_category(index));
        }
        match target {
            EQUALS => {
                let _ = self.calculator.evaluate();
            }
            CLEAR | EDIT_CLEAR | CLASSIC_CLEAR => self.calculator.clear(),
            BACKSPACE | EDIT_BACKSPACE => {
                self.calculator.backspace();
                let _ = self.calculator.preview();
            }
            PLUS_MINUS => {
                let expression = self.calculator.expression().to_owned();
                self.calculator
                    .replace_expression(&format!("-({expression})"))?;
                let _ = self.calculator.preview();
            }
            RECIPROCAL => {
                let expression = self.calculator.expression().to_owned();
                self.calculator
                    .replace_expression(&format!("1/({expression})"))?;
                let _ = self.calculator.preview();
            }
            MEMORY_CLEAR => self.calculator.memory_clear(),
            MEMORY_RECALL => self.calculator.memory_recall()?,
            MEMORY_STORE => self.calculator.memory_store(),
            MEMORY_ADD => self.calculator.memory_add()?,
            MEMORY_SUBTRACT => self.calculator.memory_subtract()?,
            MODE_BASIC => self.calculator.set_mode(Mode::Basic),
            MODE_ADVANCED => self.calculator.set_mode(Mode::Advanced),
            MODE_PROGRAMMER => self.calculator.set_mode(Mode::Programmer),
            MODE_FINANCIAL => self.calculator.set_mode(Mode::Financial),
            MODE_CONVERSION => self.calculator.set_mode(Mode::Conversion),
            ANGLE_DEGREES => self.calculator.set_angle_unit(AngleUnit::Degrees),
            ANGLE_RADIANS => self.calculator.set_angle_unit(AngleUnit::Radians),
            BASE_BINARY => self.calculator.set_integer_base(IntegerBase::Binary),
            BASE_OCTAL => self.calculator.set_integer_base(IntegerBase::Octal),
            BASE_DECIMAL => self.calculator.set_integer_base(IntegerBase::Decimal),
            BASE_HEX => self.calculator.set_integer_base(IntegerBase::Hexadecimal),
            WIDTH_8 => self.calculator.set_bit_width(BitWidth::Bits8),
            WIDTH_16 => self.calculator.set_bit_width(BitWidth::Bits16),
            WIDTH_32 => self.calculator.set_bit_width(BitWidth::Bits32),
            WIDTH_64 => self.calculator.set_bit_width(BitWidth::Bits64),
            WIDTH_128 => self.calculator.set_bit_width(BitWidth::Bits128),
            TOGGLE_HISTORY => self.history_visible = !self.history_visible,
            CLEAR_HISTORY => self.calculator.clear_history(),
            FINANCIAL_FUTURE => self.run_workbench(FinancialOperation::Future),
            FINANCIAL_PRESENT => self.run_workbench(FinancialOperation::Present),
            FINANCIAL_PAYMENT => self.run_workbench(FinancialOperation::Payment),
            FINANCIAL_STRAIGHT_LINE => self.run_workbench(FinancialOperation::StraightLine),
            FINANCIAL_DOUBLE_DECLINING => {
                self.run_workbench(FinancialOperation::DoubleDeclining);
            }
            FINANCIAL_TERM => self.run_workbench(FinancialOperation::CompoundingTerm),
            FINANCIAL_GROSS_PROFIT => self.run_workbench(FinancialOperation::GrossProfit),
            FINANCIAL_PERIODIC_RATE => self.run_workbench(FinancialOperation::PeriodicRate),
            FINANCIAL_SUM_OF_YEARS => self.run_workbench(FinancialOperation::SumOfYearsDigits),
            UNIT_CONVERT => self.run_unit_conversion(),
            COMPLEX_ADD => self.run_complex(ComplexOperation::Add),
            COMPLEX_SUBTRACT => self.run_complex(ComplexOperation::Subtract),
            COMPLEX_MULTIPLY => self.run_complex(ComplexOperation::Multiply),
            COMPLEX_DIVIDE => self.run_complex(ComplexOperation::Divide),
            COMPLEX_CONJUGATE => self.run_complex(ComplexOperation::Conjugate),
            CHARACTER_CONVERT => self.run_character_code(),
            FACTOR => self.run_factor(),
            TWOS_COMPLEMENT => self.run_twos_complement(),
            SHORTCUTS => self.dialog = Some(DialogKind::Shortcuts),
            ABOUT => self.dialog = Some(DialogKind::About),
            DIALOG_CLOSE => self.dialog = None,
            _ => {
                if let Some(value) = input_for(target) {
                    self.calculator.input(value)?;
                    let _ = self.calculator.preview();
                } else if let Some(index) = history_index(target) {
                    return Ok(self.calculator.use_history(index));
                } else {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    fn update_workbench(&mut self, target: u64, value: &str) -> bool {
        let slot = match target {
            FINANCIAL_PRINCIPAL => Some(&mut self.workbench.financial[0]),
            FINANCIAL_RATE => Some(&mut self.workbench.financial[1]),
            FINANCIAL_PERIODS => Some(&mut self.workbench.financial[2]),
            FINANCIAL_SALVAGE => Some(&mut self.workbench.financial[3]),
            FINANCIAL_FINAL_VALUE => Some(&mut self.workbench.financial[4]),
            UNIT_AMOUNT => Some(&mut self.workbench.unit[0]),
            UNIT_FROM => Some(&mut self.workbench.unit[1]),
            UNIT_TO => Some(&mut self.workbench.unit[2]),
            COMPLEX_A_REAL => Some(&mut self.workbench.complex[0]),
            COMPLEX_A_IMAGINARY => Some(&mut self.workbench.complex[1]),
            COMPLEX_B_REAL => Some(&mut self.workbench.complex[2]),
            COMPLEX_B_IMAGINARY => Some(&mut self.workbench.complex[3]),
            CHARACTER => Some(&mut self.workbench.character),
            _ => None,
        };
        let Some(slot) = slot else {
            return false;
        };
        value.clone_into(slot);
        true
    }

    pub(super) const fn programmer_scroll(&self) -> u32 {
        self.programmer_scroll
    }

    fn run_workbench(&mut self, operation: FinancialOperation) {
        let result = self.workbench.financial(operation);
        self.finish_workbench(result);
    }

    fn run_unit_conversion(&mut self) {
        let result = self.workbench.convert_unit();
        self.finish_workbench(result);
    }

    fn run_complex(&mut self, operation: ComplexOperation) {
        let result = self.workbench.complex(operation);
        self.finish_workbench(result);
    }

    fn run_character_code(&mut self) {
        let result = self.workbench.character_code();
        self.finish_workbench(result);
    }

    fn run_factor(&mut self) {
        let result = self.workbench.factor(self.calculator.expression());
        self.finish_workbench(result);
    }

    fn run_twos_complement(&mut self) {
        if self.calculator.evaluate().is_ok() {
            self.workbench.set_literal(self.calculator.twos_complement_display());
        } else {
            self.workbench.set_error(workbench::WorkbenchError::WholeExpression);
        }
    }

    fn finish_workbench(&mut self, result: Result<(), workbench::WorkbenchError>) {
        if let Err(error) = result {
            self.workbench.set_error(error);
        }
    }

    pub(crate) fn emit(&mut self) -> Result<Message, CalcActorError> {
        let patches = full_patches(self);
        self.tree.apply(&patches)?;
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or(CalcActorError::SequenceExhausted)?;
        message(self.app, self.window, self.sequence, &patches).map_err(Into::into)
    }

    fn copy_message(&self, request: RequestId) -> Message {
        Message::ClipboardRequest(ClipboardRequest::Write {
            request,
            selection: ClipboardSelection::Clipboard,
            mime_type: String::from("text/plain;charset=utf-8"),
            data: self.calculator.display().into_bytes(),
        })
    }

    pub(super) fn calculator(&self) -> &Calculator {
        &self.calculator
    }

    pub(super) fn redmond98(&self) -> bool {
        self.redmond98
    }

    pub(super) const fn history_visible(&self) -> bool {
        self.history_visible
    }

    pub(super) const fn entropy_available(&self) -> bool {
        self.entropy_available
    }

    pub(super) const fn currency_available(&self) -> bool {
        self.currency_available
    }

    pub(super) fn currency_fields(&self) -> (&str, &str, &str, &str) {
        (
            &self.currency_amount,
            &self.currency_base,
            &self.currency_quote,
            self.provider_status.render(),
        )
    }

    pub(in crate::calc) const fn workbench(&self) -> &Workbench {
        &self.workbench
    }

    pub(super) const fn dialog(&self) -> Option<DialogKind> {
        self.dialog
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ProviderIntent {
    Entropy { request: RequestId, expected: usize },
    Currency { request: RequestId },
}

impl ProviderIntent {
    const fn request(&self) -> RequestId {
        match self {
            Self::Entropy { request, .. } | Self::Currency { request } => *request,
        }
    }
}

fn allowed_keyboard_text(text: &str) -> bool {
    !text.is_empty()
        && text.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(
                    character,
                    ' ' | '.' | '+' | '-' | '*' | '/' | '%' | '!' | '^' | '(' | ')' | '&' | '|'
                )
        })
}

fn valid_iso_code(value: &str) -> bool {
    value.len() == 3 && value.bytes().all(|byte| byte.is_ascii_uppercase())
}

const fn alt_pressed(modifiers: u16) -> bool {
    modifiers & 8 != 0
}

fn key_matches(key_code: u32, text: Option<&str>, character: char) -> bool {
    key_code == u32::from(character)
        || text.is_some_and(|value| {
            value.len() == 1 && value.eq_ignore_ascii_case(&character.to_string())
        })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CalcActorError {
    Tree(TreeError),
    Conversion(PatchConversionError),
    Calculation(CalculatorError),
    InvalidProviderData,
    UnexpectedProviderResponse,
    SequenceExhausted,
}

impl From<TreeError> for CalcActorError {
    fn from(value: TreeError) -> Self {
        Self::Tree(value)
    }
}

impl From<PatchConversionError> for CalcActorError {
    fn from(value: PatchConversionError) -> Self {
        Self::Conversion(value)
    }
}

impl From<CalculatorError> for CalcActorError {
    fn from(value: CalculatorError) -> Self {
        Self::Calculation(value)
    }
}

#[cfg(test)]
#[path = "calc/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "calc/workbench_i18n_tests.rs"]
mod workbench_i18n_tests;
