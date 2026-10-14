//! Bounded, value-only presentation for a single virtual grid widget.
//!
//! The application owns the workbook. Frames carry only visible geometry and
//! display strings; they are never an alternative formula or cell model.
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

use crate::virtual_grid::VisibleTrack;

pub const FRAME_VERSION: u8 = 1;
pub const MAX_FRAME_BYTES: usize = rice_native_app_protocol::MAX_GRID_PATCH_TEXT_BYTES;
pub const MAX_FRAME_CELLS: usize = 4096;
pub const MAX_FRAME_TRACKS: usize = 512;
pub const MAX_FRAME_STRINGS: usize = 4096;
pub const MAX_FRAME_STYLES: usize = 4096;
pub const MAX_FRAME_EXTENT: u32 = 65_536;
pub const MAX_REVISION: u64 = (1_u64 << 53) - 1;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GridTrack {
    /// Zero-based logical row/column identity, never a retained widget ID.
    pub index: u32,
    pub start: i64,
    pub extent: u32,
    pub clip_start: u32,
    pub clip_end: u32,
    pub frozen: bool,
}

impl From<VisibleTrack> for GridTrack {
    fn from(track: VisibleTrack) -> Self {
        Self {
            index: track.index,
            start: track.start,
            extent: track.extent,
            clip_start: track.clip_start,
            clip_end: track.clip_end,
            frozen: track.frozen,
        }
    }
}

/// A lossless finite document font size in points. Keeping the field private
/// makes equality, hashing and total ordering safe without quantizing imported
/// OOXML doubles. The 1..=409 bound is the supported document profile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridFontSize(f64);

impl GridFontSize {
    pub fn new(points: f64) -> Result<Self, &'static str> {
        if points.is_finite() && (1.0..=409.0).contains(&points) {
            Ok(Self(points))
        } else {
            Err("document font size must be finite and within 1..=409 points")
        }
    }

    #[must_use]
    pub const fn points(self) -> f64 { self.0 }

    /// Integer fonts retain their existing binary and JSON representation.
    #[must_use]
    pub fn as_integer(self) -> Option<u16> {
        (self.0.fract() == 0.0).then_some(self.0 as u16)
    }
}

impl Eq for GridFontSize {}
impl Hash for GridFontSize {
    fn hash<H: Hasher>(&self, state: &mut H) { self.0.to_bits().hash(state); }
}
impl Ord for GridFontSize {
    fn cmp(&self, other: &Self) -> Ordering { self.0.total_cmp(&other.0) }
}
impl PartialOrd for GridFontSize {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}
impl Serialize for GridFontSize {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.as_integer() {
            Some(points) => serializer.serialize_u16(points),
            None => serializer.serialize_f64(self.0),
        }
    }
}
impl<'de> Deserialize<'de> for GridFontSize {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(f64::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// Optional document typography; family names are hints resolved by the host,
/// never CSS, URLs or an instruction to download fonts. UI chrome stays themed.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GridFont {
    pub family: String,
    pub size_pt: GridFontSize,
    pub weight: u16,
    pub italic: bool,
    pub underline: bool,
}
impl GridFont {
    pub fn encode(&self)->Option<String>{serde_json::to_string(self).ok()}
    pub fn is_valid(&self) -> bool {
        !self.family.is_empty() && self.family.len() <= 128
            && !self.family.chars().any(char::is_control)
            && (100..=900).contains(&self.weight) && self.weight % 100 == 0
    }
}

/// Effective document colors for a native single-line editor. Background is
/// required so an explicit document ink can never inherit unrelated UI chrome.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentPaint {
    pub background: String,
    #[serde(default,skip_serializing_if="Option::is_none",deserialize_with="non_null_optional")]
    pub foreground: Option<String>,
    /// Omission preserves centered generic document fields. Spreadsheet editors
    /// explicitly supply their cell's effective alignment.
    #[serde(default,skip_serializing_if="Option::is_none",deserialize_with="non_null_optional")]
    pub vertical: Option<String>,
}
impl DocumentPaint { pub fn is_valid(&self)->bool {
    valid_paint_color(&self.background) && self.foreground.as_deref().is_none_or(valid_paint_color)
        && self.vertical.as_deref().is_none_or(|value| matches!(value,"top"|"center"|"bottom"))
} }

/// Optional document paint. Omitted colors retain the current desktop theme.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GridBorder { pub style: String, pub color: String }
impl GridBorder { pub fn is_valid(&self) -> bool { valid_paint_color(&self.color) && matches!(self.style.as_str(),"hair"|"thin"|"medium"|"thick"|"double"|"dotted"|"dashed"|"dashDot"|"dashDotDot"|"mediumDashed"|"mediumDashDot"|"mediumDashDotDot"|"slantDashDot") } }
#[derive(Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GridBorders {
    #[serde(default,skip_serializing_if="Option::is_none",deserialize_with="non_null_object")] pub top: Option<GridBorder>,
    #[serde(default,skip_serializing_if="Option::is_none",deserialize_with="non_null_object")] pub right: Option<GridBorder>,
    #[serde(default,skip_serializing_if="Option::is_none",deserialize_with="non_null_object")] pub bottom: Option<GridBorder>,
    #[serde(default,skip_serializing_if="Option::is_none",deserialize_with="non_null_object")] pub left: Option<GridBorder>,
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GridPaint {
    #[serde(default,skip_serializing_if="Option::is_none",deserialize_with="non_null_optional")] pub foreground: Option<String>,
    #[serde(default,skip_serializing_if="Option::is_none",deserialize_with="non_null_optional")] pub background: Option<String>,
    #[serde(default,skip_serializing_if="Option::is_none",deserialize_with="non_null_object")] pub borders: Option<GridBorders>,
    #[serde(default,skip_serializing_if="Option::is_none",deserialize_with="non_null_optional")] pub vertical: Option<String>,
    #[serde(default,skip_serializing_if="Option::is_none",deserialize_with="non_null_optional")] pub wrap: Option<bool>,
    #[serde(default,skip_serializing_if="Option::is_none",deserialize_with="non_null_optional")] pub strike: Option<bool>,
}
fn non_null_object<'de,D:serde::Deserializer<'de>,T:serde::de::DeserializeOwned>(deserializer:D)->Result<Option<T>,D::Error>{
    let object=serde_json::Map::<String,serde_json::Value>::deserialize(deserializer)?;
    serde_json::from_value(serde_json::Value::Object(object)).map(Some).map_err(serde::de::Error::custom)
}
fn non_null_optional<'de,D:serde::Deserializer<'de>,T:Deserialize<'de>>(deserializer:D)->Result<Option<T>,D::Error>{T::deserialize(deserializer).map(Some)}
fn valid_paint_color(value:&str)->bool { value.len()==7 && value.starts_with('#') && value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit) }
impl GridPaint { pub fn is_valid(&self)->bool {
    self.foreground.as_deref().is_none_or(valid_paint_color) && self.background.as_deref().is_none_or(valid_paint_color)
    && self.vertical.as_deref().is_none_or(|v|matches!(v,"top"|"center"|"bottom"))
    && self.borders.as_ref().is_none_or(|b|[&b.top,&b.right,&b.bottom,&b.left].into_iter().all(|v|v.as_ref().is_none_or(GridBorder::is_valid)))
} }

/// Semantic colors belong to the theme; optional fonts describe document text.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GridStyle {
    /// 0 = start, 1 = center, 2 = end.
    pub alignment: u8,
    /// 0 = ordinary value, 1 = heading, 2 = error, 3 = muted.
    pub role: u8,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_grid_font")]
    pub font: Option<GridFont>,
    #[serde(default,skip_serializing_if="Option::is_none",deserialize_with="non_null_object")]
    pub paint: Option<GridPaint>,
}
fn deserialize_grid_font<'de,D:serde::Deserializer<'de>>(deserializer:D)->Result<Option<GridFont>,D::Error>{
    GridFont::deserialize(deserializer).map(Some)
}
impl GridStyle {
    pub fn is_valid(&self) -> bool { self.alignment <= 2 && self.role <= 3 && self.font.as_ref().is_none_or(GridFont::is_valid) && self.paint.as_ref().is_none_or(GridPaint::is_valid) }
}

/// Visible row slot, visible column slot, interned string index, style index.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GridCell(pub u16, pub u16, pub u16, pub u16);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Inclusive visible-track slots, independent of filtered/sorted logical IDs.
/// The engine projects its selection into each new viewport frame.
pub struct GridSelection {
    pub first_row: u32,
    pub last_row: u32,
    pub first_column: u32,
    pub last_column: u32,
}

impl GridSelection {
    #[must_use]
    pub fn contains(&self, row: u32, column: u32) -> bool {
        (self.first_row..=self.last_row).contains(&row)
            && (self.first_column..=self.last_column).contains(&column)
    }
}

/// Optional native header filter affordance; logical column identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GridFilterColumn {
    pub column: u32,
    pub active: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GridReference {
    pub first_row: u32,
    pub last_row: u32,
    pub first_column: u32,
    pub last_column: u32,
    /// Theme-owned reference ink slot; never a caller-supplied CSS color.
    pub color: u8,
}

impl GridReference {
    pub fn is_valid(&self, rows: u32, columns: u32) -> bool {
        self.first_row <= self.last_row && self.last_row < rows
            && self.first_column <= self.last_column && self.last_column < columns
            && self.color < 8
    }
}

/// One logical merged cell represented by its first visible recorder slot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GridMerge {
    pub first_row:u32,pub last_row:u32,pub first_column:u32,pub last_column:u32,
    pub row_slot:u32,pub column_slot:u32,
    pub x:i64,pub y:i64,pub width:u32,pub height:u32,
}
impl GridMerge {
    pub fn contains(&self,row:u32,column:u32)->bool{(self.first_row..=self.last_row).contains(&row)&&(self.first_column..=self.last_column).contains(&column)}
    pub fn values(&self)->[i64;10]{[i64::from(self.first_row),i64::from(self.last_row),i64::from(self.first_column),i64::from(self.last_column),i64::from(self.row_slot),i64::from(self.column_slot),self.x,self.y,i64::from(self.width),i64::from(self.height)]}
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GridFrame {
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub merges:Vec<GridMerge>,
    pub version: u8,
    pub revision: u64,
    pub sheet: u32,
    pub row_count: u32,
    pub column_count: u32,
    /// Frame viewport including headers. Tracks use body-relative coordinates.
    pub width: u32,
    pub height: u32,
    pub row_header: u32,
    pub column_header: u32,
    /// View-only scale; track coordinates have already been scaled by the engine.
    #[serde(default,skip_serializing_if="Option::is_none",deserialize_with="non_null_optional")]
    pub zoom_percent: Option<u16>,
    /// Display-only document canvas; absence retains themed grid behavior.
    #[serde(default,skip_serializing_if="Option::is_none",deserialize_with="non_null_optional")]
    pub canvas: Option<String>,
    pub rows: Vec<GridTrack>,
    pub columns: Vec<GridTrack>,
    pub strings: Vec<String>,
    pub styles: Vec<GridStyle>,
    /// Sparse visible display values, strictly sorted by (row slot, column slot).
    pub cells: Vec<GridCell>,
    pub selection: Option<GridSelection>,
    /// Logical (row, column), including offscreen cells.
    pub active: Option<(u32, u32)>,
    /// Transient formula-editing references. They do not change workbook state.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<GridReference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filter_columns: Vec<GridFilterColumn>,
    /// Logical table header row; absent retains the legacy column-letter header.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_filter_header")]
    pub filter_header_row: Option<u32>,
    /// Authoritative logical selection endpoint, distinct from clipped slots.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_fill_endpoint")]
    pub fill_endpoint: Option<(u32, u32)>,
}

fn deserialize_filter_header<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<u32>, D::Error> {
    u32::deserialize(deserializer).map(Some)
}

fn deserialize_fill_endpoint<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<(u32, u32)>, D::Error> {
    <(u32, u32)>::deserialize(deserializer).map(Some)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GridHit {
    /// True only inside a visible native filter button, never the whole header.
    pub filter: bool,
    pub fill: bool,
    pub revision: u64,
    pub sheet: u32,
    /// None denotes a column header; both None denote the corner.
    pub row: Option<u32>,
    /// None denotes a row header.
    pub column: Option<u32>,
}

impl GridFrame {
    pub fn parse(text: &str) -> Result<Self, &'static str> {
        if text.len() > MAX_FRAME_BYTES {
            return Err("grid frame bytes");
        }
        let frame: Self = serde_json::from_str(text).map_err(|_| "grid frame syntax")?;
        frame.validate()?;
        Ok(frame)
    }

    pub fn encode(&self) -> Result<String, &'static str> {
        self.validate()?;
        let text = serde_json::to_string(self).map_err(|_| "grid frame encoding")?;
        if text.len() > MAX_FRAME_BYTES {
            return Err("grid frame bytes");
        }
        Ok(text)
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.canvas.as_deref().is_some_and(|canvas|canvas != "document") { return Err("grid canvas"); }
        if self.zoom_percent.is_some_and(|zoom| !(25..=400).contains(&zoom)) { return Err("grid zoom"); }
        if self.version != FRAME_VERSION || self.revision > MAX_REVISION {
            return Err("grid frame version or revision");
        }
        if self.row_count == 0
            || self.column_count == 0
            || !(1..=MAX_FRAME_EXTENT).contains(&self.width)
            || !(1..=MAX_FRAME_EXTENT).contains(&self.height)
            || self.row_header > self.width
            || self.column_header > self.height
            || self.row_header > 256
            || self.column_header > 256
        {
            return Err("grid frame dimensions");
        }
        validate_tracks(&self.rows, self.row_count, self.height - self.column_header)?;
        validate_tracks(
            &self.columns,
            self.column_count,
            self.width - self.row_header,
        )?;
        if self.rows.len().saturating_mul(self.columns.len()) > MAX_FRAME_CELLS
            || self.cells.len() > MAX_FRAME_CELLS
            || self.strings.len() > MAX_FRAME_STRINGS
            || self.styles.is_empty()
            || self.styles.len() > MAX_FRAME_STYLES
        {
            return Err("grid frame counts");
        }
        if self
            .styles
            .iter()
            .any(|style| !style.is_valid())
        {
            return Err("grid style");
        }
        if self.filter_header_row.is_some_and(|row| row >= self.row_count) { return Err("grid filter header row"); }
        if self.merges.len()>512{return Err("grid merge count");}
        for (i,m) in self.merges.iter().enumerate(){
            if m.first_row>m.last_row||m.last_row>=self.row_count||m.first_column>m.last_column||m.last_column>=self.column_count
                ||(m.first_row==m.last_row&&m.first_column==m.last_column)||m.row_slot as usize>=self.rows.len()||m.column_slot as usize>=self.columns.len()
                ||!(-1_000_000_000..=65_536).contains(&m.x)||!(-1_000_000_000..=65_536).contains(&m.y)
                ||!(1..=1_000_000_000).contains(&m.width)||!(1..=1_000_000_000).contains(&m.height){return Err("grid merge geometry");}
            let rows:Vec<_>=self.rows.iter().filter(|t|(m.first_row..=m.last_row).contains(&t.index)).collect();
            let columns:Vec<_>=self.columns.iter().filter(|t|(m.first_column..=m.last_column).contains(&t.index)).collect();
            if rows.is_empty()||columns.is_empty()||self.rows[m.row_slot as usize]!=*rows[0]||self.columns[m.column_slot as usize]!=*columns[0]
                ||m.x>i64::from(columns[0].clip_start)||m.y>i64::from(rows[0].clip_start)
                ||m.x+i64::from(m.width)<i64::from(columns.last().unwrap().clip_end)||m.y+i64::from(m.height)<i64::from(rows.last().unwrap().clip_end){return Err("grid merge visible slot");}
            if self.merges[..i].iter().any(|n|n.first_row<=m.last_row&&n.last_row>=m.first_row&&n.first_column<=m.last_column&&n.last_column>=m.first_column){return Err("overlapping grid merges");}
        }
        if self.references.len() > 16 || self.references.iter().any(|reference| !reference.is_valid(self.row_count, self.column_count)) {
            return Err("grid references");
        }
        let mut filter_columns = std::collections::BTreeSet::new();
        if self.filter_columns.len() > MAX_FRAME_TRACKS
            || self.filter_columns.iter().any(|entry| {
                entry.column >= self.column_count || !filter_columns.insert(entry.column)
            })
        {
            return Err("grid filter columns");
        }
        let mut bytes = 0_usize;
        let mut multiline = std::collections::BTreeSet::new();
        for text in &self.strings {
            bytes += text.len();
            if text.len() > 4096 || bytes > MAX_FRAME_BYTES || text.chars().any(|c| c.is_control() && c != '\n') {
                return Err("grid string bounds");
            }
        }
        for (index,text) in self.strings.iter().enumerate() { if text.contains('\n') { multiline.insert(index); } }
        let mut referenced_multiline = std::collections::BTreeSet::new();
        let mut previous = None;
        for GridCell(row, column, text, style) in &self.cells {
            let key = (*row, *column);
            if usize::from(*row) >= self.rows.len()
                || usize::from(*column) >= self.columns.len()
                || usize::from(*text) >= self.strings.len()
                || usize::from(*style) >= self.styles.len()
                || previous.is_some_and(|previous| previous >= key)
            {
                return Err("grid cell indexes or ordering");
            }
            if multiline.contains(&usize::from(*text)) {
                if !self.styles[usize::from(*style)].paint.as_ref().is_some_and(|p| p.wrap == Some(true)) {
                    return Err("grid multiline text requires wrapping");
                }
                referenced_multiline.insert(usize::from(*text));
            }
            previous = Some(key);
        }
        if referenced_multiline != multiline { return Err("grid unreferenced multiline text"); }
        if self.fill_endpoint.is_some_and(|(row, column)| row >= self.row_count || column >= self.column_count) {
            return Err("grid fill endpoint");
        }
        if self.selection.as_ref().is_some_and(|s| {
            s.first_row > s.last_row
                || s.first_column > s.last_column
                || s.last_row as usize >= self.rows.len()
                || s.last_column as usize >= self.columns.len()
        }) || self
            .active
            .is_some_and(|(row, column)| row >= self.row_count || column >= self.column_count)
        {
            return Err("grid selection");
        }
        Ok(())
    }

    /// Small native fill handle on the complete visible selection endpoint.
    /// A partly clipped endpoint does not move its handle to a false boundary.
    #[must_use]
    pub fn fill_handle(&self) -> Option<(crate::Rect, (u32, u32))> {
        let selection = self.selection.as_ref()?;
        let first_row=self.rows.get(selection.first_row as usize)?.index;let last_row=self.rows.get(selection.last_row as usize)?.index;
        let first_column=self.columns.get(selection.first_column as usize)?.index;let last_column=self.columns.get(selection.last_column as usize)?.index;
        if self.merges.iter().any(|m|m.first_row<=last_row&&m.last_row>=first_row&&m.first_column<=last_column&&m.last_column>=first_column){return None;}
        let row = self.rows.get(selection.last_row as usize)?;
        let column = self.columns.get(selection.last_column as usize)?;
        if self.fill_endpoint.is_some_and(|endpoint| endpoint != (row.index, column.index))
            || row.clip_end - row.clip_start < 8 || column.clip_end - column.clip_start < 8
            || i64::from(row.clip_end) != row.start + i64::from(row.extent)
            || i64::from(column.clip_end) != column.start + i64::from(column.extent) { return None; }
        Some((crate::Rect { origin: crate::Point {
            x: (self.row_header + column.clip_end - 7) as i32,
            y: (self.column_header + row.clip_end - 7) as i32,
        }, size: crate::Size { width: 6, height: 6 } }, (row.index, column.index)))
    }

    /// Exact widget-local filter button bounds shared with the SDK hit test.
    /// Tiny/clipped headers suppress the affordance rather than steal all hits.
    #[must_use]
    pub fn filter_button(&self, column: u32) -> Option<(crate::Rect, bool)> {
        let entry = self
            .filter_columns
            .iter()
            .find(|entry| entry.column == column)?;
        let track = self.columns.iter().find(|track| track.index == column)?;
        let (top, span) = if let Some(header) = self.filter_header_row {
            let row = self.rows.iter().find(|row| row.index == header)?;
            (self.column_header + row.clip_start, row.clip_end - row.clip_start)
        } else { (0, self.column_header) };
        if span < 12 || track.clip_end - track.clip_start < 24 {
            return None;
        }
        let height = span.min(18);
        Some((
            crate::Rect {
                origin: crate::Point {
                    x: (self.row_header + track.clip_end - 18) as i32,
                    y: (top + (span - height) / 2) as i32,
                },
                size: crate::Size { width: 16, height },
            },
            entry.active,
        ))
    }

    /// Route widget-local pointer coordinates against the same clipped tracks
    /// used for rendering. Consumers reject stale revisions before editing.
    #[must_use]
    pub fn hit_test(&self, x: i32, y: i32) -> Option<GridHit> {
        let x = u32::try_from(x).ok()?;
        let y = u32::try_from(y).ok()?;
        if x >= self.width || y >= self.height {
            return None;
        }
        let row = if y < self.column_header {
            None
        } else {
            Some(track_hit(&self.rows, y - self.column_header)?)
        };
        let column = if x < self.row_header {
            None
        } else {
            Some(track_hit(&self.columns, x - self.row_header)?)
        };
        let filter = column.is_some_and(|column| {
                self.filter_button(column).is_some_and(|(rect, _)| {
                    x >= rect.origin.x as u32
                        && x < rect.origin.x as u32 + rect.size.width
                        && y >= rect.origin.y as u32
                        && y < rect.origin.y as u32 + rect.size.height
                })
            });
        let fill = self.fill_handle().is_some_and(|(rect, _)| x >= rect.origin.x as u32 && x < rect.origin.x as u32 + rect.size.width
            && y >= rect.origin.y as u32 && y < rect.origin.y as u32 + rect.size.height);
        let merged=if !filter&&!fill {row.zip(column).and_then(|(r,c)|self.merges.iter().find(|m|m.contains(r,c)))}else{None};
        Some(GridHit {
            filter,
            fill,
            revision: self.revision,
            sheet: self.sheet,
            row:merged.map_or(row,|m|Some(m.first_row)),
            column:merged.map_or(column,|m|Some(m.first_column)),
        })
    }
}

fn track_hit(tracks: &[GridTrack], position: u32) -> Option<u32> {
    let slot = tracks.partition_point(|track| track.clip_end <= position);
    tracks
        .get(slot)
        .filter(|track| position >= track.clip_start)
        .map(|track| track.index)
}

fn validate_tracks(tracks: &[GridTrack], count: u32, viewport: u32) -> Result<(), &'static str> {
    if tracks.len() > MAX_FRAME_TRACKS {
        return Err("grid track count");
    }
    let mut last_end = 0;
    let mut identities = std::collections::BTreeSet::new();
    let mut scrolling_seen = false;
    for track in tracks {
        if track.index >= count
            || track.extent == 0
            || track.extent > crate::virtual_grid::MAX_TRACK_EXTENT
            || track.start < -i64::from(crate::virtual_grid::MAX_TRACK_EXTENT)
            || track.start > i64::from(MAX_FRAME_EXTENT)
            || track.clip_start < last_end
            || track.clip_start >= track.clip_end
            || track.clip_end > viewport
            || i64::from(track.clip_start) < track.start
            || i64::from(track.clip_end) > track.start + i64::from(track.extent)
            || !identities.insert(track.index)
            || (scrolling_seen && track.frozen)
        {
            return Err("grid track geometry");
        }
        scrolling_seen |= !track.frozen;
        last_end = track.clip_end;
    }
    Ok(())
}

/// Excel-style column identity generated by the host, independent of contents.
#[must_use]
pub fn column_label(index: u32) -> String {
    let mut value = u64::from(index) + 1;
    let mut bytes = Vec::new();
    while value > 0 {
        bytes.push(b'A' + ((value - 1) % 26) as u8);
        value = (value - 1) / 26;
    }
    bytes.reverse();
    String::from_utf8(bytes).expect("column letters are ASCII")
}
