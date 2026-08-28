//! watchord-midi: `SoundingSetSource` over MIDI, via `midir`.
//!
//! Ported from note-view's `MIDIInput` target. Everything that can be decided
//! without hardware is kept away from it, exactly as there:
//!
//! - [`MidiEvent`] — the three channel-voice messages that change what sounds.
//! - [`decode`] — raw MIDI 1.0 bytes into events (`midir` hands us bytes, not
//!   CoreMIDI's UMP words, so this is the decoder note-view's `UMPDecoder` becomes).
//! - [`HeldNoteTracker`] — the note-on / note-off / sustain state machine.
//! - [`SettleRelay`] — the settle window, as a pure value driven by instants.
//! - [`MidiSource`] — the one thing that touches a device.

mod decode;
mod held_note_tracker;
mod settle_relay;
mod source;

pub use decode::decode;
pub use held_note_tracker::HeldNoteTracker;
pub use settle_relay::SettleRelay;
pub use source::{HOT_PLUG_INTERVAL, MidiSource};

/// One MIDI channel-voice message, as a plain value.
///
/// Only the three messages that change what is sounding are modelled. Everything
/// else on the wire (pitch bend, aftertouch, program change, clock, SysEx) is
/// dropped by the decoder rather than represented here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MidiEvent {
    /// Note-on. **Velocity 0 is not normalised here.** Many controllers send
    /// note-on/velocity-0 in place of note-off; `HeldNoteTracker` owns that
    /// rule, alone, so there is exactly one thing to test and one thing to break.
    NoteOn {
        /// MIDI note number, 0–127.
        note: u8,
        /// 1–127 for a strike; 0 is the note-off convention, left to the tracker.
        velocity: u8,
        /// MIDI channel, 0–15.
        channel: u8,
    },
    /// Note-off. Release velocity is not carried: nothing in this app reads it.
    NoteOff {
        /// MIDI note number, 0–127.
        note: u8,
        /// MIDI channel, 0–15.
        channel: u8,
    },
    /// Control change, 7-bit value. CC64 is the sustain pedal; see `tuning`.
    ControlChange {
        /// Controller number; 64 is sustain.
        controller: u8,
        /// The controller's 7-bit value.
        value: u8,
        /// MIDI channel, 0–15.
        channel: u8,
    },
}
