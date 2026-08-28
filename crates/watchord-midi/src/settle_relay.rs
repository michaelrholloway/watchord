//! The settle window, as a pure value. Ported from note-view's
//! `SoundingSetRelay.swift`, minus the actor and the clock: instead of sleeping
//! it reports when it is due, and the thread that owns it does the waiting.
//!
//! That split is what makes the debounce testable without waiting. A 60 ms real
//! sleep in a test can only ever prove "something was emitted eventually", never
//! "nothing was emitted before the interval elapsed", which is the half of the
//! spec that actually prevents a four-note chord flickering through three wrong
//! names.

use std::time::{Duration, Instant};

use watchord_core::SoundingSet;
use watchord_core::tuning;

use crate::{HeldNoteTracker, MidiEvent};

/// A tracker plus the settle window over it: events go in, and a sounding set
/// comes out only once it has been stable for the whole interval.
#[derive(Clone, Debug)]
pub struct SettleRelay {
    tracker: HeldNoteTracker,
    settle: Duration,
    /// When the current set has been stable long enough to emit, or `None`
    /// when nothing is pending.
    due: Option<Instant>,
}

impl Default for SettleRelay {
    fn default() -> Self {
        Self::new(tuning::SETTLE_INTERVAL)
    }
}

impl SettleRelay {
    /// A relay with the given settle window and nothing held.
    pub fn new(settle: Duration) -> Self {
        SettleRelay {
            tracker: HeldNoteTracker::new(),
            settle,
            due: None,
        }
    }

    /// Feeds a batch of events through the tracker, restarting the settle
    /// window if — and only if — the sounding set actually changed.
    pub fn receive(&mut self, events: impl IntoIterator<Item = MidiEvent>, now: Instant) {
        if self.tracker.apply_all(events) {
            self.due = Some(now + self.settle);
        }
    }

    /// When the owner should next call [`SettleRelay::fire`], or `None` when
    /// nothing is pending.
    pub fn due_at(&self) -> Option<Instant> {
        self.due
    }

    /// The set to emit, if the window has elapsed by `now`. Reads the set now
    /// rather than at schedule time: this is the freshest truth, and it is the
    /// set that has been stable for the whole interval.
    pub fn fire(&mut self, now: Instant) -> Option<SoundingSet> {
        let due = self.due?;
        if now < due {
            return None;
        }
        self.due = None;
        Some(self.tracker.sounding())
    }

    /// What is held right now, ignoring the settle window.
    pub fn currently_sounding(&self) -> SoundingSet {
        self.tracker.sounding()
    }

    /// Cancels any pending emission and forgets all held notes. Deliberately
    /// silent: the last chord stays on screen after release, so tearing the
    /// input down should not blank it.
    pub fn shut_down(&mut self) {
        self.due = None;
        self.tracker.reset();
    }
}
