//! Sparse geometry for large native grids. Cell values and formulas are owned
//! by the application engine; the renderer requests only visible coordinates.
//!
//! Uniform tracks share runs, including hidden ranges. A million default rows
//! therefore require one run, and scrolling across hidden rows never scans them.

use std::ops::Range;

pub const MAX_TRACK_EXTENT: u32 = 1_000_000;
pub const MAX_VISIBLE_TRACKS: usize = 8_192;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GridError {
    InvalidRange,
    InvalidExtent,
    ViewportTooDense,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Run {
    first: u32,
    end: u32,
    extent: u32,
    offset: u64,
}

impl Run {
    fn end_offset(&self) -> u64 {
        self.offset + u64::from(self.end - self.first) * u64::from(self.extent)
    }
}

/// Run-length encoded row or column geometry in logical device-independent
/// units. Zero extent hides a track. Zoom is applied by the native renderer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GridAxis {
    count: u32,
    runs: Vec<Run>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VisibleTrack {
    pub index: u32,
    /// Unclipped position; a partially scrolled track may start before its pane.
    pub start: i64,
    pub extent: u32,
    pub clip_start: u32,
    pub clip_end: u32,
    pub frozen: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AxisViewport {
    pub tracks: Vec<VisibleTrack>,
    pub frozen_extent: u32,
    /// The actual clamped scroll position within the unfrozen region.
    pub scroll: u64,
}

impl AxisViewport {
    #[must_use]
    pub fn hit_test(&self, position: u32) -> Option<u32> {
        let index = self.tracks.partition_point(|track| track.clip_end <= position);
        self.tracks.get(index).filter(|track| position >= track.clip_start).map(|track| track.index)
    }
}

impl GridAxis {
    pub fn new(count: u32, extent: u32) -> Result<Self, GridError> {
        if extent > MAX_TRACK_EXTENT {
            return Err(GridError::InvalidExtent);
        }
        let runs = if count == 0 { Vec::new() } else { vec![Run { first: 0, end: count, extent, offset: 0 }] };
        Ok(Self { count, runs })
    }

    #[must_use]
    pub const fn count(&self) -> u32 { self.count }

    #[must_use]
    pub fn run_count(&self) -> usize { self.runs.len() }

    #[must_use]
    pub fn total_extent(&self) -> u64 { self.runs.last().map_or(0, Run::end_offset) }

    /// Return a boundary offset, including the trailing boundary at `count`.
    #[must_use]
    pub fn offset(&self, index: u32) -> Option<u64> {
        if index == self.count { return Some(self.total_extent()); }
        if index > self.count { return None; }
        let run = self.runs.get(self.runs.partition_point(|run| run.end <= index))?;
        Some(run.offset + u64::from(index - run.first) * u64::from(run.extent))
    }

    #[must_use]
    pub fn track_at(&self, offset: u64) -> Option<u32> {
        let run = self.runs.get(self.runs.partition_point(|run| run.end_offset() <= offset))?;
        // A zero-size run cannot have end_offset > offset after its predecessors.
        let displacement = u32::try_from((offset - run.offset) / u64::from(run.extent)).ok()?;
        Some(run.first + displacement)
    }

    /// Resize or hide an entire range without visiting its individual tracks.
    pub fn set_extent(&mut self, range: Range<u32>, extent: u32) -> Result<(), GridError> {
        if range.start > range.end || range.end > self.count { return Err(GridError::InvalidRange); }
        if extent > MAX_TRACK_EXTENT { return Err(GridError::InvalidExtent); }
        if range.is_empty() { return Ok(()); }
        let mut runs = Vec::with_capacity(self.runs.len() + 2);
        for run in &self.runs {
            if run.end <= range.start || run.first >= range.end {
                Self::append_run(&mut runs, run.first, run.end, run.extent);
                continue;
            }
            Self::append_run(&mut runs, run.first, run.end.min(range.start), run.extent);
            Self::append_run(&mut runs, run.first.max(range.start), run.end.min(range.end), extent);
            Self::append_run(&mut runs, run.first.max(range.end), run.end, run.extent);
        }
        self.runs = runs;
        Ok(())
    }

    fn append_run(runs: &mut Vec<Run>, first: u32, end: u32, extent: u32) {
        if first >= end { return; }
        if let Some(previous) = runs.last_mut() {
            if previous.end == first && previous.extent == extent {
                previous.end = end;
                return;
            }
        }
        let offset = runs.last().map_or(0, Run::end_offset);
        runs.push(Run { first, end, extent, offset });
    }

    /// Compute visible geometry for frozen and scrolling panes. The returned
    /// list is ordered by screen position and can be used for drawing/hit tests.
    pub fn viewport(&self, frozen: u32, scroll: u64, extent: u32) -> Result<AxisViewport, GridError> {
        let frozen_total = self.offset(frozen).ok_or(GridError::InvalidRange)?;
        let frozen_extent = u32::try_from(frozen_total.min(u64::from(extent))).unwrap_or(extent);
        let body_extent = extent - frozen_extent;
        let body_total = self.total_extent() - frozen_total;
        let scroll = scroll.min(body_total.saturating_sub(u64::from(body_extent)));
        let mut tracks = Vec::new();
        self.append_view(&mut tracks, 0..frozen, 0, 0..frozen_extent, true)?;
        self.append_view(&mut tracks, frozen..self.count, frozen_total + scroll,
                         frozen_extent..extent, false)?;
        Ok(AxisViewport { tracks, frozen_extent, scroll })
    }

    fn append_view(&self, output: &mut Vec<VisibleTrack>, indexes: Range<u32>, origin: u64,
                   clip: Range<u32>, frozen: bool) -> Result<(), GridError> {
        if indexes.is_empty() || clip.is_empty() { return Ok(()); }
        let limit = origin + u64::from(clip.end - clip.start);
        let first_run = self.runs.partition_point(|run| run.end_offset() <= origin);
        for run in &self.runs[first_run..] {
            if run.first >= indexes.end || run.offset >= limit { break; }
            if run.extent == 0 || run.end <= indexes.start { continue; }
            let skip = u32::try_from(origin.saturating_sub(run.offset) / u64::from(run.extent))
                .map_err(|_| GridError::InvalidRange)?;
            let first = (run.first + skip).max(indexes.start);
            for index in first..run.end.min(indexes.end) {
                let offset = run.offset + u64::from(index - run.first) * u64::from(run.extent);
                if offset >= limit { break; }
                if output.len() >= MAX_VISIBLE_TRACKS { return Err(GridError::ViewportTooDense); }
                // u32 track counts * bounded extents are always below i64::MAX.
                let start = i64::from(clip.start) + i64::try_from(offset).map_err(|_| GridError::InvalidRange)?
                    - i64::try_from(origin).map_err(|_| GridError::InvalidRange)?;
                let end = start + i64::from(run.extent);
                output.push(VisibleTrack {
                    index, start, extent: run.extent,
                    clip_start: u32::try_from(start.max(i64::from(clip.start))).map_err(|_| GridError::InvalidRange)?,
                    clip_end: u32::try_from(end.min(i64::from(clip.end))).map_err(|_| GridError::InvalidRange)?,
                    frozen,
                });
            }
        }
        Ok(())
    }
}
