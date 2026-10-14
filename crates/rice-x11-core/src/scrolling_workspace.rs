//! One independent horizontal strip per output. Monitor geometry is presentation
//! state; window membership and preferred column widths survive output changes.
use crate::scrolling::ScrollingStrip;
use crate::tiling::{Axis, Rect};

#[derive(Clone, Debug)]
struct OutputStrip<T> {
    id: u32,
    area: Rect,
    strip: ScrollingStrip<T>,
}
#[derive(Clone, Debug)]
pub struct ScrollingLayout<T> {
    outputs: Vec<OutputStrip<T>>,
    assignments: Vec<(T, u32)>,
}
impl<T> Default for ScrollingLayout<T> {
    fn default() -> Self {
        Self {
            outputs: Vec::new(),
            assignments: Vec::new(),
        }
    }
}
impl<T: Copy + Eq> ScrollingLayout<T> {
    fn fallback(area: Rect) -> OutputStrip<T> {
        OutputStrip {
            id: 0,
            area,
            strip: ScrollingStrip::default(),
        }
    }
    fn output_index(&self, id: T) -> Option<usize> {
        self.outputs.iter().position(|o| o.strip.contains(id))
    }
    pub fn contains(&self, id: T) -> bool {
        self.output_index(id).is_some()
    }
    pub fn clip(&self, id: T) -> Option<Rect> {
        self.output_index(id).map(|i| self.outputs[i].area)
    }
    pub fn configure(&mut self, areas: &[Rect], assignments: &[(T, u32)]) {
        if areas.is_empty() {
            return;
        }
        for (index, &area) in areas.iter().enumerate() {
            if let Some(output) = self.outputs.iter_mut().find(|o| o.id == index as u32) {
                output.area = area;
            } else {
                self.outputs.push(OutputStrip {
                    id: index as u32,
                    area,
                    strip: ScrollingStrip::default(),
                });
            }
        }
        self.assignments = assignments.to_vec();
        // Retain disconnected monitor strips until the next synchronization.
        // Their members are migrated explicitly, never drawn across a neighbor.
        for output in &mut self.outputs {
            if output.id as usize >= areas.len() {
                output.area = areas[0];
            }
        }
    }
    // The caller validates a complete permutation before changing any topology.
    pub fn remap_output_indices(&mut self, old_to_new: &[u32]) {
        let remap = |id: u32| old_to_new.get(id as usize).copied().unwrap_or(id);
        for output in &mut self.outputs { output.id = remap(output.id); }
        for (_, output) in &mut self.assignments { *output = remap(*output); }
    }
    pub fn set_fullscreens(&mut self, windows: &[(T, u16)]) {
        for output in &mut self.outputs {
            let selected: Vec<_> = windows
                .iter()
                .copied()
                .filter(|(id, _)| {
                    self.assignments
                        .iter()
                        .find(|(w, _)| w == id)
                        .map_or(0, |(_, o)| *o)
                        == output.id
                })
                .collect();
            output.strip.set_fullscreens(&selected);
        }
    }
    pub fn retain(&mut self, live: &[T]) {
        for o in &mut self.outputs {
            o.strip.retain(live);
        }
        self.assignments.retain(|(id, _)| live.contains(id));
    }
    pub fn sync(&mut self, members: &[T], area: Rect, minimums: &[(T, u16, u16)]) {
        if self.outputs.is_empty() {
            self.outputs.push(Self::fallback(area));
        }
        if self.outputs.len() == 1 && self.assignments.is_empty() {
            self.outputs[0].area = area;
        }
        let previous = self.outputs.clone();
        for output in &mut self.outputs {
            let selected: Vec<_> = members
                .iter()
                .copied()
                .filter(|id| {
                    self.assignments
                        .iter()
                        .find(|(w, _)| w == id)
                        .map_or(0, |(_, o)| *o)
                        == output.id
                })
                .collect();
            for source in &previous {
                if source.id != output.id {
                    output.strip.adopt(&source.strip, &selected);
                }
            }
            output.strip.sync(&selected, output.area, minimums);
        }
    }
    pub fn focus(&mut self, active: Option<T>, _area: Rect) {
        if let Some(index) = active.and_then(|id| self.output_index(id)) {
            let output = &mut self.outputs[index];
            output.strip.focus(active, output.area);
        }
    }
    pub fn pan_at(&mut self, delta: i32, x: i32, y: i32) -> bool {
        let Some(output) = self.outputs.iter_mut().find(|o| {
            x >= o.area.x
                && x < o.area.x + i32::from(o.area.width)
                && y >= o.area.y
                && y < o.area.y + i32::from(o.area.height)
        }) else {
            return false;
        };
        output.strip.pan(delta, output.area.width)
    }
    pub fn rectangles(&self, _area: Rect, minimums: &[(T, u16, u16)]) -> Vec<(T, Rect)> {
        self.outputs
            .iter()
            .flat_map(|o| o.strip.rectangles(o.area, minimums))
            .collect()
    }
    pub fn neighbor(&self, id: T, direction: u32) -> Option<T> {
        self.outputs
            .get(self.output_index(id)?)?
            .strip
            .neighbor(id, direction)
    }
    pub fn swap(&mut self, a: T, b: T) -> bool {
        let Some(index) = self.output_index(a) else {
            return false;
        };
        self.output_index(b) == Some(index) && self.outputs[index].strip.swap(a, b)
    }
    pub fn insert(
        &mut self,
        source: T,
        target: T,
        direction: u32,
        _area: Rect,
        minimums: &[(T, u16, u16)],
    ) -> bool {
        let Some(index) = self.output_index(source) else {
            return false;
        };
        if self.output_index(target) != Some(index) {
            return false;
        }
        let output = &mut self.outputs[index];
        output
            .strip
            .insert(source, target, direction, output.area, minimums)
    }
    pub fn resize(
        &mut self,
        id: T,
        axis: Axis,
        delta: i32,
        _area: Rect,
        minimums: &[(T, u16, u16)],
    ) -> bool {
        let Some(index) = self.output_index(id) else {
            return false;
        };
        let output = &mut self.outputs[index];
        output.strip.resize(id, axis, delta, output.area, minimums)
    }
    pub fn preset(
        &mut self,
        id: T,
        operation: u32,
        _area: Rect,
        minimums: &[(T, u16, u16)],
    ) -> bool {
        let Some(index) = self.output_index(id) else {
            return false;
        };
        let output = &mut self.outputs[index];
        output.strip.preset(id, operation, output.area, minimums)
    }
    pub fn dividers(&self, _area: Rect, minimums: &[(T, u16, u16)]) -> Vec<(Axis, Rect, i32)> {
        self.outputs
            .iter()
            .flat_map(|o| o.strip.dividers(o.area, minimums))
            .collect()
    }
    pub fn move_divider(
        &mut self,
        token: u32,
        position: i32,
        _area: Rect,
        minimums: &[(T, u16, u16)],
    ) -> bool {
        let Some(mut index) = (token >> 1).checked_sub(1) else {
            return false;
        };
        for output in &mut self.outputs {
            let count = output.strip.dividers(output.area, minimums).len() as u32;
            if index < count {
                return output.strip.move_divider(
                    ((index + 1) << 1) | (token & 1),
                    position,
                    output.area,
                    minimums,
                );
            }
            index -= count;
        }
        false
    }
    pub fn encode(&self, token: &mut impl FnMut(T) -> u32, words: &mut Vec<u32>) {
        words.push(self.outputs.len() as u32);
        for output in &self.outputs {
            words.push(output.id);
            output.strip.encode(token, words);
        }
    }
    pub fn decode(
        words: &[u32],
        cursor: &mut usize,
        resolve: &impl Fn(u32) -> Option<T>,
        version: u32,
    ) -> Option<Self> {
        let mut layout = Self::default();
        if version == 2 {
            let strip = ScrollingStrip::decode(words, cursor, resolve)?;
            layout.outputs.push(OutputStrip {
                id: 0,
                area: Rect {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                },
                strip,
            });
            return Some(layout);
        }
        let count = *words.get(*cursor)?;
        *cursor += 1;
        if count > 16 {
            return None;
        }
        let mut seen = Vec::new();
        for _ in 0..count {
            let fields = words.get(*cursor..*cursor + 1)?;
            *cursor += 1;
            if fields[0] >= 16 || layout.outputs.iter().any(|o| o.id == fields[0]) {
                return None;
            }
            let area = Rect {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            };
            if area.width == 0 || area.height == 0 {
                return None;
            }
            let strip = ScrollingStrip::decode(words, cursor, resolve)?;
            for id in strip.members() {
                if seen.contains(&id) {
                    return None;
                }
                seen.push(id);
                layout.assignments.push((id, fields[0]));
            }
            layout.outputs.push(OutputStrip {
                id: fields[0],
                area,
                strip,
            });
        }
        Some(layout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const LEFT: Rect = Rect {
        x: 0,
        y: 30,
        width: 1000,
        height: 650,
    };
    const RIGHT: Rect = Rect {
        x: 1000,
        y: 30,
        width: 800,
        height: 650,
    };
    #[test]
    fn monitors_scroll_independently_and_roundtrip_membership() {
        let mut layout = ScrollingLayout::default();
        let members = [1u32, 2, 3, 4, 5, 6];
        let assignments = [(1, 0), (2, 0), (3, 0), (4, 1), (5, 1), (6, 1)];
        layout.configure(&[LEFT, RIGHT], &assignments);
        layout.sync(&members, LEFT, &[]);
        // Explicit half-width columns exercise overflow independently of defaults.
        for (id, rect) in layout.rectangles(LEFT, &[]) {
            layout.resize(id, Axis::Horizontal, 500 - i32::from(rect.width), LEFT, &[]);
        }
        let before = layout.rectangles(LEFT, &[]);
        assert!(layout.pan_at(200, 1100, 300));
        let after = layout.rectangles(LEFT, &[]);
        for (id, r) in &before {
            let next = after.iter().find(|(w, _)| w == id).unwrap().1;
            assert_eq!(next.x, if *id <= 3 { r.x } else { r.x - 200 });
            assert_eq!(layout.clip(*id), Some(if *id <= 3 { LEFT } else { RIGHT }));
        }
        layout.focus(Some(3), LEFT);
        let focused = layout.rectangles(LEFT, &[]);
        assert_eq!(focused[2].1.x + i32::from(focused[2].1.width), 1000);
        assert_eq!(focused[3..], after[3..]);
        let mut words = Vec::new();
        layout.encode(&mut |id| id, &mut words);
        let mut cursor = 0;
        let mut restored = ScrollingLayout::decode(&words, &mut cursor, &Some, 3).unwrap();
        assert_eq!(cursor, words.len());
        restored.configure(&[LEFT, RIGHT], &assignments);
        restored.sync(&members, LEFT, &[]);
        assert_eq!(restored.rectangles(LEFT, &[]), before);
    }
}

#[cfg(test)]
mod migration_tests {
    use super::*;
    #[test]
    fn disconnect_and_reconnect_keep_column_widths_and_stacks() {
        let a = Rect {
            x: 0,
            y: 0,
            width: 1000,
            height: 700,
        };
        let b = Rect { x: 1000, ..a };
        let mut s = ScrollingLayout::default();
        s.configure(&[a, b], &[(1, 0), (2, 1), (3, 1)]);
        s.sync(&[1, 2, 3], a, &[]);
        s.resize(2, Axis::Horizontal, 83, a, &[]);
        s.insert(3, 2, 3, a, &[]);
        let initial = s.rectangles(a, &[]);
        s.configure(&[a], &[(1, 0), (2, 0), (3, 0)]);
        s.sync(&[1, 2, 3], a, &[]);
        let migrated = s.rectangles(a, &[]);
        assert_eq!(migrated[1].1.width, initial[1].1.width);
        assert_eq!(migrated[1].1.x, migrated[2].1.x);
        s.configure(&[a, b], &[(1, 0), (2, 1), (3, 1)]);
        s.sync(&[1, 2, 3], a, &[]);
        assert_eq!(s.rectangles(a, &[]), initial);
    }
}
