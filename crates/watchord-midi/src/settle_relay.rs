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

use watchord_core::tuning;
use watchord_core::{ControlEvent, PedalKind, SoundingSet};

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
    ///
    /// Returns every pedal that changed in this batch, sustain first — the
    /// source seam's third event, alongside the settled `SoundingSet` that
    /// `fire` yields separately. Pedal changes are never debounced: a plate
    /// should read the pedal the instant it moves.
    pub fn receive(
        &mut self,
        events: impl IntoIterator<Item = MidiEvent>,
        now: Instant,
    ) -> Vec<ControlEvent> {
        let before = (
            self.tracker.is_sustain_down(),
            self.tracker.is_sostenuto_down(),
            self.tracker.is_soft_down(),
        );
        if self.tracker.apply_all(events) {
            self.due = Some(now + self.settle);
        }
        let after = (
            self.tracker.is_sustain_down(),
            self.tracker.is_sostenuto_down(),
            self.tracker.is_soft_down(),
        );
        let mut controls = Vec::new();
        if before.0 != after.0 {
            controls.push(ControlEvent {
                pedal: PedalKind::Sustain,
                down: after.0,
            });
        }
        if before.1 != after.1 {
            controls.push(ControlEvent {
                pedal: PedalKind::Sostenuto,
                down: after.1,
            });
        }
        if before.2 != after.2 {
            controls.push(ControlEvent {
                pedal: PedalKind::Soft,
                down: after.2,
            });
        }
        controls
    }

    /// The settle window currently in effect.
    pub fn settle(&self) -> Duration {
        self.settle
    }

    /// Changes the settle window live. Does not disturb a pending emission's
    /// deadline — only the next restart uses the new window.
    pub fn set_settle(&mut self, settle: Duration) {
        self.settle = settle;
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
