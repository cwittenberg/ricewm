//! Scrollable columns: fit up to three automatic columns before overflowing.
use crate::tiling::{Axis, Rect};

#[derive(Clone, Debug)]
struct Column<T> {
    windows: Vec<T>,
    width: u16,
    automatic: bool,
    restore_width: Option<(u16, bool)>,
}
#[derive(Clone, Debug)]
pub struct ScrollingStrip<T> {
    columns: Vec<Column<T>>,
    weights: Vec<(T, u32)>,
    offset: i32,
    focused: Option<T>,
    viewport: u16,
    centered: bool,
    fullscreens: Vec<(T, u16)>,
    fullscreen_home: Vec<(T, T, usize)>,
}
impl<T> Default for ScrollingStrip<T> {
    fn default() -> Self {
        Self {
            columns: Vec::new(),
            weights: Vec::new(),
            offset: 0,
            focused: None,
            viewport: 0,
            centered: false,
            fullscreens: Vec::new(),
            fullscreen_home: Vec::new(),
        }
    }
}
impl<T: Copy + Eq> ScrollingStrip<T> {
    pub fn set_fullscreens(&mut self, windows: &[(T, u16)]) {
        if self.fullscreens == windows {
            return;
        }
        // A fullscreen row temporarily gets its own column. Keep its original
        // neighbor and row index so leaving fullscreen restores the stack.
        for (id, anchor, row) in self.fullscreen_home.clone() {
            if windows.iter().any(|(w, _)| *w == id) {
                continue;
            }
            if let (Some((source, r)), Some((target, _))) = (self.locate(id), self.locate(anchor)) {
                if source != target {
                    self.columns[source].windows.remove(r);
                    let at = row.min(self.columns[target].windows.len());
                    self.columns[target].windows.insert(at, id);
                    self.columns.retain(|c| !c.windows.is_empty());
                }
            }
        }
        self.fullscreen_home
            .retain(|(id, _, _)| windows.iter().any(|(w, _)| w == id));
        for &(id, _) in windows {
            if self.fullscreens.iter().any(|(w, _)| *w == id) {
                continue;
            }
            if let Some((column, row)) = self.locate(id) {
                if self.columns[column].windows.len() > 1 {
                    let width = self.columns[column].width;
                    let automatic = self.columns[column].automatic;
                    self.columns[column].windows.remove(row);
                    let anchor = self.columns[column].windows[0];
                    self.fullscreen_home.push((id, anchor, row));
                    self.columns.insert(
                        column + 1,
                        Column {
                            windows: vec![id],
                            width,
                            automatic,
                            restore_width: None,
                        },
                    );
                }
            }
        }
        self.fullscreens = windows.to_vec();
        self.focused = None;
    }
    fn column_width(&self, column: &Column<T>) -> u16 {
        self.fullscreens
            .iter()
            .find(|(id, _)| column.windows.contains(id))
            .map_or(column.width, |(_, width)| *width)
    }
    pub fn adopt(&mut self, source: &Self, members: &[T]) {
        for column in &source.columns {
            let windows: Vec<_> = column
                .windows
                .iter()
                .copied()
                .filter(|id| members.contains(id) && !self.contains(*id))
                .collect();
            if windows.is_empty() {
                continue;
            }
            for &id in &windows {
                self.weights.push((id, source.weight(id)));
            }
            self.columns.push(Column {
                windows,
                width: column.width,
                automatic: column.automatic,
                restore_width: column.restore_width,
            });
        }
    }
    pub fn retain(&mut self, live: &[T]) {
        for c in &mut self.columns {
            c.windows.retain(|id| live.contains(id));
        }
        self.columns.retain(|c| !c.windows.is_empty());
        self.weights.retain(|(id, _)| live.contains(id));
    }
    pub fn encode(&self, token: &mut impl FnMut(T) -> u32, words: &mut Vec<u32>) {
        words.push(self.columns.len() as u32);
        for c in &self.columns {
            // Bit 16 preserves automatic sizing; legacy widths remain explicit.
            words.extend([
                u32::from(c.width) | (u32::from(c.automatic) << 16),
                c.windows.len() as u32,
            ]);
            for &id in &c.windows {
                words.extend([token(id), self.weight(id)]);
            }
        }
    }
    pub fn decode(
        words: &[u32],
        cursor: &mut usize,
        resolve: &impl Fn(u32) -> Option<T>,
    ) -> Option<Self> {
        let count = *words.get(*cursor)?;
        *cursor += 1;
        if count > 128 {
            return None;
        }
        let mut layout = Self::default();
        let mut seen = Vec::new();
        for _ in 0..count {
            let encoded_width = *words.get(*cursor)?;
            if encoded_width > 0x1ffff {
                return None;
            }
            let automatic = encoded_width & 0x10000 != 0;
            let width = (encoded_width & 0xffff) as u16;
            *cursor += 1;
            let rows = *words.get(*cursor)?;
            *cursor += 1;
            if width == 0 || rows == 0 || rows > 128 || seen.len() + rows as usize > 128 {
                return None;
            }
            let mut windows = Vec::new();
            for _ in 0..rows {
                let id = resolve(*words.get(*cursor)?)?;
                *cursor += 1;
                if seen.contains(&id) {
                    return None;
                }
                seen.push(id);
                windows.push(id);
                let weight = *words.get(*cursor)?;
                *cursor += 1;
                if weight > 65535 {
                    return None;
                }
                layout.weights.push((id, weight));
            }
            layout.columns.push(Column {
                windows,
                width,
                automatic,
                restore_width: None,
            });
        }
        Some(layout)
    }
    fn weight(&self, id: T) -> u32 {
        self.weights
            .iter()
            .find(|(w, _)| *w == id)
            .map_or(1000, |(_, v)| *v)
    }
    fn minimum_height(id: T, minimums: &[(T, u16, u16)]) -> u32 {
        minimums
            .iter()
            .find(|(w, _, _)| *w == id)
            .map_or(100, |(_, _, h)| u32::from(*h).max(100))
    }
    pub fn members(&self) -> Vec<T> {
        self.columns
            .iter()
            .flat_map(|c| c.windows.iter().copied())
            .collect()
    }
    pub fn contains(&self, id: T) -> bool {
        self.locate(id).is_some()
    }
    fn locate(&self, id: T) -> Option<(usize, usize)> {
        self.columns
            .iter()
            .enumerate()
            .find_map(|(c, column)| column.windows.iter().position(|w| *w == id).map(|r| (c, r)))
    }
    fn start(&self, index: usize) -> i32 {
        self.columns[..index]
            .iter()
            .map(|c| i32::from(self.column_width(c)))
            .sum()
    }
    fn total(&self) -> i32 {
        self.start(self.columns.len())
    }
    pub fn sync(&mut self, members: &[T], area: Rect, minimums: &[(T, u16, u16)]) {
        // Preserve the screen position of the active column when windows to its
        // left disappear. A new window enters immediately after the active column.
        let anchor = self.focused.and_then(|id| {
            self.locate(id)
                .map(|(c, _)| (id, self.start(c) - self.offset))
        });
        for c in &mut self.columns {
            c.windows.retain(|id| members.contains(id));
        }
        self.columns.retain(|c| !c.windows.is_empty());
        self.weights.retain(|(id, _)| members.contains(id));
        let mut insertion = self
            .focused
            .and_then(|id| self.locate(id))
            .map_or(self.columns.len(), |(c, _)| c + 1);
        for &id in members {
            if self.contains(id) {
                continue;
            }
            self.columns.insert(
                insertion,
                Column {
                    windows: vec![id],
                    width: area.width.max(1),
                    automatic: true,
                    restore_width: None,
                },
            );
            insertion += 1;
        }
        let slots = self.columns.len().clamp(1, 3) as u16;
        for (index, c) in self.columns.iter_mut().enumerate() {
            if c.restore_width.is_some() {
                c.width = area.width.max(1);
            } else if c.automatic {
                // Distribute the remainder so the first three columns fill the workarea.
                c.width = area.width / slots
                    + u16::from((index % usize::from(slots)) < usize::from(area.width % slots));
            }
            let minimum = c
                .windows
                .iter()
                .filter_map(|id| {
                    minimums
                        .iter()
                        .find(|(w, _, _)| w == id)
                        .map(|(_, w, _)| *w)
                })
                .max()
                .unwrap_or(160);
            c.width = c.width.max(minimum).min(area.width.max(1));
        }
        if let Some((id, x)) = anchor {
            if let Some((c, _)) = self.locate(id) {
                self.offset = self.start(c) - x;
            }
        }
        if self.columns.is_empty() {
            self.offset = 0;
            self.focused = None;
        }
        self.clamp_offset(area.width);
    }
    pub fn focus(&mut self, active: Option<T>, area: Rect) {
        let resized = self.viewport != area.width;
        self.viewport = area.width;
        let active = active.filter(|id| self.contains(*id));
        if active == self.focused && !resized {
            return;
        }
        self.focused = active;
        if let Some((c, _)) = active.and_then(|id| self.locate(id)) {
            let start = self.start(c);
            let end = start + i32::from(self.column_width(&self.columns[c]));
            if end - start > i32::from(area.width) {
                self.offset = start;
            } else if start < self.offset {
                self.offset = start;
            } else if end > self.offset + i32::from(area.width) {
                self.offset = end - i32::from(area.width);
            }
        }
        self.clamp_offset(area.width);
    }
    fn clamp_offset(&mut self, width: u16) {
        let first = self.columns.first().map_or(width, |c| c.width);
        let last = self.columns.last().map_or(width, |c| c.width);
        let left = if self.centered {
            -(i32::from(width) - i32::from(first)).max(0) / 2
        } else {
            0
        };
        let right = if self.centered {
            (i32::from(width) - i32::from(last)).max(0) / 2
        } else {
            0
        };
        self.offset = self
            .offset
            .clamp(left, (self.total() - i32::from(width) + right).max(left));
    }
    pub fn preset(
        &mut self,
        id: T,
        operation: u32,
        area: Rect,
        minimums: &[(T, u16, u16)],
    ) -> bool {
        let Some((c, _)) = self.locate(id) else {
            return false;
        };
        if operation == 6 {
            let offset =
                self.start(c) - (i32::from(area.width) - i32::from(self.columns[c].width)) / 2;
            let changed = self.offset != offset;
            self.offset = offset;
            self.centered = true;
            self.focused = Some(id);
            return changed;
        }
        let width = self.columns[c].width;
        let presets = [
            area.width / 3,
            area.width / 2,
            (u32::from(area.width) * 2 / 3) as u16,
        ];
        if operation == 5 {
            if let Some((previous, automatic)) = self.columns[c].restore_width.take() {
                self.resize(
                    id,
                    Axis::Horizontal,
                    i32::from(previous) - i32::from(width),
                    area,
                    minimums,
                );
                self.columns[c].automatic = automatic;
                self.focused = None;
                return true;
            }
            let automatic = self.columns[c].automatic;
            if !self.resize(
                id,
                Axis::Horizontal,
                i32::from(area.width) - i32::from(width),
                area,
                minimums,
            ) {
                return false;
            }
            self.columns[c].restore_width = Some((width, automatic));
            return true;
        }
        let next = presets
            .into_iter()
            .find(|preset| *preset > width.saturating_add(1))
            .unwrap_or(presets[0]);
        self.resize(
            id,
            Axis::Horizontal,
            i32::from(next) - i32::from(width),
            area,
            minimums,
        )
    }
    pub fn pan(&mut self, delta: i32, width: u16) -> bool {
        self.centered = false;
        let next = self
            .offset
            .saturating_add(delta)
            .clamp(0, (self.total() - i32::from(width)).max(0));
        let changed = next != self.offset;
        self.offset = next;
        changed
    }
    pub fn rectangles(&self, area: Rect, minimums: &[(T, u16, u16)]) -> Vec<(T, Rect)> {
        let mut x = area.x - self.offset;
        let mut output = Vec::new();
        for c in &self.columns {
            let count = c.windows.len() as u32;
            let minima: Vec<_> = c
                .windows
                .iter()
                .map(|id| Self::minimum_height(*id, minimums))
                .collect();
            let minimum: u32 = minima.iter().sum();
            let extra = u32::from(area.height).saturating_sub(minimum);
            let weights: u32 = c.windows.iter().map(|id| self.weight(*id)).sum();
            let mut y = 0;
            let mut cumulative = 0;
            for (r, &id) in c.windows.iter().enumerate() {
                let before = cumulative;
                cumulative += self.weight(id);
                let height = if minimum > u32::from(area.height) {
                    u32::from(area.height) * (r as u32 + 1) / count
                        - u32::from(area.height) * r as u32 / count
                } else if weights == 0 {
                    minima[r] + extra * (r as u32 + 1) / count - extra * r as u32 / count
                } else {
                    minima[r]
                        + (u64::from(extra) * u64::from(cumulative) / u64::from(weights)) as u32
                        - (u64::from(extra) * u64::from(before) / u64::from(weights)) as u32
                };
                output.push((
                    id,
                    Rect {
                        x,
                        y: area.y + y as i32,
                        width: self.column_width(c),
                        height: height as u16,
                    },
                ));
                y += height;
            }
            x += i32::from(self.column_width(c));
        }
        output
    }
    pub fn neighbor(&self, id: T, direction: u32) -> Option<T> {
        let (c, r) = self.locate(id)?;
        match direction {
            0 => self
                .columns
                .get(c.checked_sub(1)?)
                .map(|c| c.windows[r.min(c.windows.len() - 1)]),
            1 => self
                .columns
                .get(c + 1)
                .map(|c| c.windows[r.min(c.windows.len() - 1)]),
            2 => self.columns[c].windows.get(r.checked_sub(1)?).copied(),
            3 => self.columns[c].windows.get(r + 1).copied(),
            _ => None,
        }
    }
    pub fn swap(&mut self, a: T, b: T) -> bool {
        let (Some((ac, ar)), Some((bc, br))) = (self.locate(a), self.locate(b)) else {
            return false;
        };
        if a == b {
            return false;
        }
        self.columns[ac].windows[ar] = b;
        self.columns[bc].windows[br] = a;
        self.focused = None;
        true
    }
    pub fn insert(
        &mut self,
        source: T,
        target: T,
        direction: u32,
        area: Rect,
        minimums: &[(T, u16, u16)],
    ) -> bool {
        if source == target || direction > 3 {
            return false;
        }
        let (Some((sc, sr)), Some((tc, _))) = (self.locate(source), self.locate(target)) else {
            return false;
        };
        if direction >= 2 {
            let members: Vec<_> = self.columns[tc]
                .windows
                .iter()
                .copied()
                .filter(|id| *id != source)
                .chain([source])
                .collect();
            let needed: u32 = members
                .iter()
                .map(|id| {
                    minimums
                        .iter()
                        .find(|(w, _, _)| w == id)
                        .map_or(100, |(_, _, h)| u32::from(*h).max(100))
                })
                .sum();
            // Equal-height rows must each accommodate their client's minimum.
            let max = members
                .iter()
                .map(|id| {
                    minimums
                        .iter()
                        .find(|(w, _, _)| w == id)
                        .map_or(100, |(_, _, h)| u32::from(*h).max(100))
                })
                .max()
                .unwrap_or(0);
            if needed > u32::from(area.height)
                || max * members.len() as u32 > u32::from(area.height)
            {
                return false;
            }
        }
        let width = self.columns[sc].width;
        let automatic = self.columns[sc].automatic;
        self.columns[sc].windows.remove(sr);
        self.columns.retain(|c| !c.windows.is_empty());
        let (tc, tr) = self.locate(target).unwrap();
        if direction < 2 {
            self.columns.insert(
                tc + usize::from(direction == 1),
                Column {
                    windows: vec![source],
                    width,
                    automatic,
                    restore_width: None,
                },
            );
        } else {
            self.columns[tc]
                .windows
                .insert(tr + usize::from(direction == 3), source);
        }
        self.focused = None;
        true
    }
    pub fn resize(
        &mut self,
        id: T,
        axis: Axis,
        delta: i32,
        area: Rect,
        minimums: &[(T, u16, u16)],
    ) -> bool {
        let Some((c, row)) = self.locate(id) else {
            return false;
        };
        if axis == Axis::Vertical {
            let column = &self.columns[c];
            if column.windows.len() < 2 {
                return false;
            }
            let other = column.windows[if row + 1 < column.windows.len() {
                row + 1
            } else {
                row - 1
            }];
            let cells = self.rectangles(area, minimums);
            let current = i32::from(cells.iter().find(|(w, _)| *w == id).unwrap().1.height);
            let adjacent = i32::from(cells.iter().find(|(w, _)| *w == other).unwrap().1.height);
            let low = Self::minimum_height(id, minimums) as i32;
            let high = current + adjacent - Self::minimum_height(other, minimums) as i32;
            if high < low {
                return false;
            }
            let next = (current + delta).clamp(low, high);
            if next == current {
                return false;
            }
            for &member in &column.windows {
                let height = i32::from(cells.iter().find(|(w, _)| *w == member).unwrap().1.height)
                    + if member == id {
                        next - current
                    } else if member == other {
                        current - next
                    } else {
                        0
                    };
                let weight = (height - Self::minimum_height(member, minimums) as i32).max(0) as u32;
                self.weights.retain(|(w, _)| *w != member);
                self.weights.push((member, weight));
            }
            return true;
        }
        let min = self.columns[c]
            .windows
            .iter()
            .filter_map(|id| {
                minimums
                    .iter()
                    .find(|(w, _, _)| w == id)
                    .map(|(_, w, _)| *w)
            })
            .max()
            .unwrap_or(160)
            .max(160)
            .min(area.width.max(1));
        let width = (i32::from(self.columns[c].width) + delta)
            .clamp(i32::from(min), i32::from(area.width.max(1))) as u16;
        let changed = width != self.columns[c].width;
        self.columns[c].width = width;
        if changed {
            self.columns[c].automatic = false;
            self.columns[c].restore_width = None;
            self.focused = None;
        }
        changed
    }
    pub fn dividers(&self, area: Rect, minimums: &[(T, u16, u16)]) -> Vec<(Axis, Rect, i32)> {
        let mut output: Vec<_> = self
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let x = area.x + self.start(i) - self.offset;
                (
                    Axis::Horizontal,
                    Rect {
                        x,
                        width: self.column_width(c),
                        ..area
                    },
                    x + i32::from(c.width),
                )
            })
            .collect();
        let cells = self.rectangles(area, minimums);
        for c in &self.columns {
            for &id in c.windows.iter().take(c.windows.len().saturating_sub(1)) {
                let rect = cells.iter().find(|(w, _)| *w == id).unwrap().1;
                output.push((Axis::Vertical, rect, rect.y + i32::from(rect.height)));
            }
        }
        output
    }
    pub fn move_divider(
        &mut self,
        token: u32,
        position: i32,
        area: Rect,
        minimums: &[(T, u16, u16)],
    ) -> bool {
        let Some(index) = (token >> 1).checked_sub(1).map(|i| i as usize) else {
            return false;
        };
        let dividers = self.dividers(area, minimums);
        let Some(&(axis, _, old)) = dividers.get(index) else {
            return false;
        };
        if (token & 1) != u32::from(axis == Axis::Vertical) {
            return false;
        }
        let id = if axis == Axis::Horizontal {
            self.columns[index].windows[0]
        } else {
            let rows: Vec<_> = self
                .columns
                .iter()
                .flat_map(|c| {
                    c.windows
                        .iter()
                        .copied()
                        .take(c.windows.len().saturating_sub(1))
                })
                .collect();
            rows[index - self.columns.len()]
        };
        self.resize(id, axis, position.saturating_sub(old), area, minimums)
    }
}

#[cfg(test)]
#[path = "../tests/scrolling.rs"]
mod tests;
