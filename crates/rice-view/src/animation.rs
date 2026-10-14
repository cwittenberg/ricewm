use core::fmt;

/// Hard upper bound for one animation/slideshow interval.
const MAX_INTERVAL_MS: u64 = 86_400_000;
/// Hidden content wakes at most once per minute.
const HIDDEN_THROTTLE_MS: u64 = 60_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Visibility {
    Visible,
    Hidden,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaybackEvent {
    NoChange,
    Advanced { steps: u32 },
}

/// Deterministic clock model driven by monotonic adapter timestamps.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaybackClock {
    interval_ms: u64,
    last_tick_ms: Option<u64>,
    paused: bool,
    visibility: Visibility,
}

impl PlaybackClock {
    /// Creates a clock with a bounded non-zero interval.
    ///
    /// # Errors
    ///
    /// Returns [`ClockError::InvalidInterval`] when the interval is invalid.
    pub fn new(interval_ms: u64) -> Result<Self, ClockError> {
        if interval_ms == 0 || interval_ms > MAX_INTERVAL_MS {
            return Err(ClockError::InvalidInterval);
        }
        Ok(Self {
            interval_ms,
            last_tick_ms: None,
            paused: false,
            visibility: Visibility::Visible,
        })
    }

    #[must_use]
    pub const fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self, now_ms: u64) {
        self.paused = false;
        self.last_tick_ms = Some(now_ms);
    }

    pub fn set_visibility(&mut self, visibility: Visibility, now_ms: u64) {
        if self.visibility != visibility {
            self.visibility = visibility;
            self.last_tick_ms = Some(now_ms);
        }
    }

    /// Applies a monotonic timestamp supplied by the host clock adapter.
    ///
    /// # Errors
    ///
    /// Returns [`ClockError::NonMonotonic`] when time moves backwards.
    pub fn tick(&mut self, now_ms: u64) -> Result<PlaybackEvent, ClockError> {
        let Some(last) = self.last_tick_ms else {
            self.last_tick_ms = Some(now_ms);
            return Ok(PlaybackEvent::NoChange);
        };
        if now_ms < last {
            return Err(ClockError::NonMonotonic);
        }
        if self.paused {
            self.last_tick_ms = Some(now_ms);
            return Ok(PlaybackEvent::NoChange);
        }
        let effective_interval = match self.visibility {
            Visibility::Visible => self.interval_ms,
            Visibility::Hidden => self.interval_ms.max(HIDDEN_THROTTLE_MS),
        };
        let elapsed = now_ms - last;
        let steps = elapsed / effective_interval;
        if steps == 0 {
            return Ok(PlaybackEvent::NoChange);
        }
        let bounded_steps = u32::try_from(steps).unwrap_or(u32::MAX);
        self.last_tick_ms =
            Some(last.saturating_add(u64::from(bounded_steps).saturating_mul(effective_interval)));
        Ok(PlaybackEvent::Advanced {
            steps: bounded_steps,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClockError {
    InvalidInterval,
    NonMonotonic,
}

impl fmt::Display for ClockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ClockError {}
