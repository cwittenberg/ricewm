//! Bounded native presentation damage. This owns no workbook or calculation.
use crate::grid_frame::{
    GridCell, GridFrame, GridReference, GridSelection, GridStyle, MAX_FRAME_BYTES, MAX_FRAME_CELLS, MAX_REVISION,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GridDamage {
    pub version: u8,
    pub sheet: u32,
    pub base_revision: u64,
    pub revision: u64,
    /// Fingerprint of geometry, not values. Envelope ordering provides delivery identity.
    pub base_geometry: u32,
    pub cells: Vec<(u16, u16, Option<String>, Option<GridStyle>)>,
    pub selection: Option<GridSelection>,
    pub active: Option<(u32, u32)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub references: Option<Vec<GridReference>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GridDamageError {
    Invalid,
    BaseMismatch,
}

/// Stable FNV-1a over explicitly ordered geometry values, shared with the SDK.
pub fn geometry_key(frame: &GridFrame) -> u32 {
    let tracks = |list: &[crate::grid_frame::GridTrack]| {
        list.iter()
            .map(|t| {
                (
                    t.index,
                    t.start,
                    t.extent,
                    t.clip_start,
                    t.clip_end,
                    u8::from(t.frozen),
                )
            })
            .collect::<Vec<_>>()
    };
    let filters = frame
        .filter_columns
        .iter()
        .map(|f| (f.column, u8::from(f.active)))
        .collect::<Vec<_>>();
    let mut data = serde_json::to_vec(&(
        frame.sheet,
        frame.row_count,
        frame.column_count,
        frame.width,
        frame.height,
        frame.row_header,
        frame.column_header,
        tracks(&frame.rows),
        tracks(&frame.columns),
        filters,
    ))
    .expect("bounded geometry");
    // Preserve the original geometry signature exactly when the optional
    // negotiated field is absent (including mixed GRID_DAMAGE v1 hosts).
    if let Some((row, column)) = frame.fill_endpoint {
        data.pop();
        data.extend_from_slice(format!(",[{row},{column}]]").as_bytes());
    }
    if let Some(row) = frame.filter_header_row {
        data.pop();
        data.extend_from_slice(format!(",{row}]").as_bytes());
    }
    if let Some(zoom) = frame.zoom_percent {
        data.pop(); data.extend_from_slice(format!(",{zoom}]").as_bytes());
    }
    if frame.canvas.is_some() {
        data.pop(); data.extend_from_slice(b",\"document\"]");
    }
    if !frame.merges.is_empty(){
        data.pop();data.push(b',');
        data.extend_from_slice(&serde_json::to_vec(&frame.merges.iter().map(|m|m.values()).collect::<Vec<_>>()).expect("bounded merge geometry"));data.push(b']');
    }
    data.iter().fold(2_166_136_261_u32, |hash, byte| {
        (hash ^ u32::from(*byte)).wrapping_mul(16_777_619)
    })
}

impl GridDamage {
    pub fn parse(text: &str) -> Result<Self, GridDamageError> {
        if text.len() > MAX_FRAME_BYTES {
            return Err(GridDamageError::Invalid);
        }
        let value: Self = serde_json::from_str(text).map_err(|_| GridDamageError::Invalid)?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<(), GridDamageError> {
        let invalid = GridDamageError::Invalid;
        if self.version != 1
            || self.base_revision > MAX_REVISION
            || self.revision > MAX_REVISION
            || self.revision < self.base_revision
            || self.cells.len() > MAX_FRAME_CELLS
        {
            return Err(invalid);
        }
        let mut previous = None;
        if self.references.as_ref().is_some_and(|list| list.len() > 16 || list.iter().any(|r| !r.is_valid(u32::MAX, u32::MAX))) {
            return Err(invalid);
        }
        for (row, col, text, style) in &self.cells {
            if *row >= 512 || *col >= 512 || previous.is_some_and(|key| key >= (*row, *col)) {
                return Err(invalid);
            }
            previous = Some((*row, *col));
            match (text, style) {
                (None, None) => {}
                (Some(text), Some(style))
                    if text.len() <= 4096
                        && !text.chars().any(|c| c.is_control() && (c != '\n' || !style.paint.as_ref().is_some_and(|p| p.wrap == Some(true))))
                        && style.is_valid() => {}
                _ => return Err(invalid),
            }
        }
        Ok(())
    }
    pub fn apply(&self, frame: &GridFrame) -> Result<GridFrame, GridDamageError> {
        self.validate()?;
        if frame.revision != self.base_revision
            || frame.sheet != self.sheet
            || geometry_key(frame) != self.base_geometry
        {
            return Err(GridDamageError::BaseMismatch);
        }
        let mut cells: BTreeMap<_, _> = frame
            .cells
            .iter()
            .map(|GridCell(row, col, text, style)| {
                (
                    (*row, *col),
                    (
                        frame.strings[usize::from(*text)].clone(),
                        frame.styles[usize::from(*style)].clone(),
                    ),
                )
            })
            .collect();
        for (row, col, text, style) in &self.cells {
            if usize::from(*row) >= frame.rows.len() || usize::from(*col) >= frame.columns.len() {
                return Err(GridDamageError::Invalid);
            }
            if let (Some(text), Some(style)) = (text, style) {
                cells.insert((*row, *col), (text.clone(), style.clone()));
            } else {
                cells.remove(&(*row, *col));
            }
        }
        let mut next = frame.clone();
        next.revision = self.revision;
        next.selection = self.selection.clone();
        next.active = self.active;
        if let Some(references) = &self.references { next.references = references.clone(); }
        next.cells.clear();
        next.strings.clear();
        next.styles = vec![frame.styles[0].clone()];
        let mut string_ids = BTreeMap::new();
        let mut style_ids =
            BTreeMap::from([(frame.styles[0].clone(), 0)]);
        for ((row, col), (text, style)) in cells {
            let text_id = *string_ids.entry(text.clone()).or_insert_with(|| {
                next.strings.push(text);
                (next.strings.len() - 1) as u16
            });
            let style_id = *style_ids
                .entry(style.clone())
                .or_insert_with(|| {
                    next.styles.push(style);
                    (next.styles.len() - 1) as u16
                });
            next.cells.push(GridCell(row, col, text_id, style_id));
        }
        next.encode().map_err(|_| GridDamageError::Invalid)?;
        Ok(next)
    }
}
