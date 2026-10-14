//! Bounded cell presentation shared by the terminal actor and native compositor.
use serde::{Deserialize, Serialize};
pub const CELL_WIDTH: u32 = 8;
pub const CELL_HEIGHT: u32 = 18;
pub const MAX_COLUMNS: u16 = 512;
pub const MAX_ROWS: u16 = 256;
/// Column, width (one or two cells), grapheme, foreground, background, SGR flags.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Cell(pub u16, pub u8, pub String, pub u32, pub u32, pub u8);
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalImage {
    pub source: String,
    pub column: u16,
    pub columns: u16,
    pub rows: u16,
    pub slice: u16,
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalRow {
    #[serde(default)]
    pub readable_colors: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<TerminalImage>,
    pub cells: Vec<Cell>,
    #[serde(default)]
    pub font_size: u8,
    #[serde(default)]
    pub transparency: u8,
    pub cursor: Option<u16>,
    #[serde(default)]
    pub cursor_color: Option<u32>,
    #[serde(default = "enabled")]
    pub icon_glyphs: bool,
    pub selection: Option<(u16, u16)>,
}

const fn enabled() -> bool {
    true
}
impl TerminalRow {
    pub fn parse(text: &str) -> Result<Self, &'static str> {
        let row: Self = serde_json::from_str(text).map_err(|_| "invalid terminal row")?;
        row.validate(MAX_COLUMNS)?;
        Ok(row)
    }
    pub fn validate(&self, columns: u16) -> Result<(), &'static str> {
        if self.cursor_color.is_some_and(|color| color > 0xffffff)
            || self.transparency > 80
            || (self.font_size != 0 && !(10..=24).contains(&self.font_size))
            || self.cells.len() > usize::from(columns)
            || self.cursor.is_some_and(|x| x >= columns)
            || self.selection.is_some_and(|(a, b)| a > b || b > columns)
        {
            return Err("terminal row bounds");
        }
        if let Some(image) = &self.image {
            let data = image.source.strip_prefix("data:image/svg+xml;base64,")
                .ok_or("terminal image format")?;
            if data.len() > 32768 || data.is_empty()
                || !data.bytes().all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b))
                || image.column >= MAX_COLUMNS || !(1..=128).contains(&image.columns)
                || !(1..=64).contains(&image.rows) || image.slice >= image.rows {
                return Err("terminal image bounds");
            }
        }
        let mut end = 0;
        for Cell(x, width, text, fg, bg, flags) in &self.cells {
            if *x < end
                || !(1..=2).contains(width)
                || x.saturating_add(u16::from(*width)) > columns
                || text.len() > 64
                || text.chars().any(char::is_control)
                || *fg > 0xffffff
                || *bg > 0xffffff
                || *flags > 31
            {
                return Err("terminal cell bounds");
            }
            end = *x + u16::from(*width);
        }
        Ok(())
    }
    pub fn encode(&self) -> String {
        serde_json::to_string(self).expect("terminal row serialization")
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalFrame<Line = TerminalRow> {
    pub columns: u16,
    pub rows: u16,
    pub lines: Vec<Line>,
    pub title: String,
    pub offset: u32,
    pub history: u32,
    pub application_cursor: bool,
    pub bracketed_paste: bool,
    pub mouse: bool,
    pub mouse_sgr: bool,
    pub mouse_motion: u8,
    pub mouse_x10: bool,
}
impl TerminalFrame {
    pub fn parse(payload: &str) -> Result<Self, &'static str> {
        if payload.len() > rice_native_app_protocol::MAX_TERMINAL_GRID_BYTES {
            return Err("terminal frame size");
        }
        let frame: Self = serde_json::from_str(payload).map_err(|_| "invalid terminal frame")?;
        frame.validate()?;
        Ok(frame)
    }
    /// Null rows retain a row from the preceding frame, only at identical dimensions.
    pub fn parse_update(payload: &str, previous: Option<&Self>) -> Result<Self, &'static str> {
        if payload.len() > rice_native_app_protocol::MAX_TERMINAL_GRID_BYTES {
            return Err("terminal frame size");
        }
        let incoming: TerminalFrame<Option<TerminalRow>> =
            serde_json::from_str(payload).map_err(|_| "invalid terminal update")?;
        let previous = previous
            .filter(|frame| frame.columns == incoming.columns && frame.rows == incoming.rows);
        let lines = incoming
            .lines
            .into_iter()
            .enumerate()
            .map(|(index, row)| {
                row.or_else(|| previous.and_then(|frame| frame.lines.get(index)).cloned())
                    .ok_or("terminal update missing base row")
            })
            .collect::<Result<Vec<_>, _>>()?;
        let frame = Self {
            columns: incoming.columns,
            rows: incoming.rows,
            lines,
            title: incoming.title,
            offset: incoming.offset,
            history: incoming.history,
            application_cursor: incoming.application_cursor,
            bracketed_paste: incoming.bracketed_paste,
            mouse: incoming.mouse,
            mouse_sgr: incoming.mouse_sgr,
            mouse_motion: incoming.mouse_motion,
            mouse_x10: incoming.mouse_x10,
        };
        frame.validate()?;
        Ok(frame)
    }
    fn validate(&self) -> Result<(), &'static str> {
        let frame = self;
        if !(2..=MAX_COLUMNS).contains(&frame.columns)
            || !(1..=MAX_ROWS).contains(&frame.rows)
            || frame.lines.len() != usize::from(frame.rows)
            || frame.title.len() > 256
            || frame.title.chars().any(char::is_control)
            || frame.mouse_motion > 2
            || frame.history > 5000
            || frame.offset > frame.history
        {
            return Err("terminal frame bounds");
        }
        for row in &frame.lines {
            row.validate(frame.columns)?;
            if row.encode().len() > rice_native_app_protocol::MAX_WIDGET_PATCH_TEXT_BYTES {
                return Err("terminal row size");
            }
        }
        Ok(())
    }
}
