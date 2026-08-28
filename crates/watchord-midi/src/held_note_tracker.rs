//! The note-on / note-off / sustain state machine, as a pure value type.
//! Ported from note-view's `HeldNoteTracker.swift`.
//!
//! **Every real bug in this crate lives here**, which is why it is a plain
//! struct with no clock, no device and no threads: feed it events, read the set,
//! assert. A stuck note is reproducible without a keyboard plugged in.
//!
//! ## The model
//!
//! Two sets, not one: `pressed` — keys physically down — and `sustained` — keys
//! released while the pedal was down, still sounding. What is sounding is their
//! union. Keeping them apart is what makes the pedal correct: releasing a key
//! under the pedal moves it between the sets rather than out of the world, and
//! lifting the pedal empties `sustained` in one go while leaving anything still
//! under a finger exactly where it is.
//!
//! ## Channel-blindness
//!
//! A note is identified by its MIDI note number alone; the channel is ignored.
//! The app listens to every source at once and asks one question — what is
//! audibly sounding — and a chord does not become two chords because a
//! controller split it across channels. Sustain is likewise global.

use std::collections::BTreeSet;

use watchord_core::SoundingSet;
use watchord_core::tuning;

use crate::MidiEvent;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HeldNoteTracker {
    /// Keys physically down.
    pressed: BTreeSet<u8>,
    /// Keys released under the pedal, still sounding until it lifts.
    sustained: BTreeSet<u8>,
    /// Whether CC64 last read at or above `tuning::SUSTAIN_ON_THRESHOLD`.
    sustain_down: bool,
}

impl HeldNoteTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Everything audibly sounding, as the engine's input type.
    pub fn sounding(&self) -> SoundingSet {
        SoundingSet::new(self.held_notes())
    }

    pub fn is_sustain_down(&self) -> bool {
        self.sustain_down
    }

    /// Applies one event.
    ///
    /// Returns `true` when the sounding set is now different from what it was.
    /// Callers debounce on this rather than on "an event arrived", so a repeated
    /// note-on for a key already down, a note-off for a key that was never down,
    /// and a pedal press that holds nothing all correctly emit nothing.
    pub fn apply(&mut self, event: MidiEvent) -> bool {
        let before = self.held_notes();
        match event {
            MidiEvent::NoteOn { note, velocity, .. } => {
                // The velocity-0 convention, in the one place that owns it.
                if velocity == 0 {
                    self.release(note);
                } else {
                    self.press(note);
                }
            }
            MidiEvent::NoteOff { note, .. } => self.release(note),
            MidiEvent::ControlChange {
                controller, value, ..
            } => {
                if controller == tuning::SUSTAIN_CONTROLLER {
                    self.set_sustain(value >= tuning::SUSTAIN_ON_THRESHOLD);
                }
            }
        }
        self.held_notes() != before
    }

    /// Applies a batch, reporting whether the set ended up different.
    ///
    /// Not a loop over `apply` at the call site, because a batch that presses
    /// and releases the same key nets out to no change and must not trigger a
    /// re-name.
    pub fn apply_all(&mut self, events: impl IntoIterator<Item = MidiEvent>) -> bool {
        let before = self.held_notes();
        for event in events {
            self.apply(event);
        }
        self.held_notes() != before
    }

    /// Drops all state, pedal included. Used when the input is torn down: a
    /// restart must not inherit a note whose note-off went out with the
    /// disconnected device.
    pub fn reset(&mut self) {
        self.pressed.clear();
        self.sustained.clear();
        self.sustain_down = false;
    }

    fn held_notes(&self) -> BTreeSet<u8> {
        self.pressed.union(&self.sustained).copied().collect()
    }

    fn press(&mut self, note: u8) {
        // No `sustained.remove(note)` here — deliberate. Re-striking a
        // sustaining key survives the next pedal lift because the key is in
        // `pressed`, and what is sounding is the union, so being in both sets
        // is indistinguishable from being in `pressed` alone.
        self.pressed.insert(note);
    }

    fn release(&mut self, note: u8) {
        // This guard is load-bearing twice over. It ignores a note-off for a
        // key that is not down, so a duplicate or out-of-order one cannot raise
        // a damper the player is still holding. And under the pedal it is the
        // only thing stopping a note-off for a key that was NEVER struck from
        // conjuring that note into the sustained set out of nothing.
        if !self.pressed.remove(&note) {
            return;
        }
        if self.sustain_down {
            self.sustained.insert(note);
        }
    }

    fn set_sustain(&mut self, down: bool) {
        if down == self.sustain_down {
            return;
        }
        self.sustain_down = down;
        // Lifting the pedal releases everything pending at once.
        if !down {
            self.sustained.clear();
        }
    }
}
