//! Ported from note-view `Sources/NoteViewCore/Seams.swift`.

use std::sync::mpsc::Receiver;
use std::time::Duration;

use crate::{ChordAnalysis, ChordKey, ChordNote, SoundingSet};

/// Which pedal a [`ControlEvent`] names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PedalKind {
    /// CC64. Sustains keys released while it is down.
    Sustain,
    /// CC66. Ticket 13 repurposes it to toggle arpeggio mode.
    Sostenuto,
    /// CC67. Shown as a plate; nothing else reads it yet.
    Soft,
}

/// One pedal moving up or down, as the source reports it. The third event on
/// the [`SoundingSetSource`] seam, alongside a settled [`SoundingSet`] and the
/// connected-inputs set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ControlEvent {
    /// Which pedal.
    pub pedal: PedalKind,
    /// True when the pedal is now down.
    pub down: bool,
}

/// Names a sounding set. Implemented by the engine's `NamingEngine`.
///
/// Pure and synchronous by contract — no I/O, no clock, no hidden state. That is
/// what lets the whole naming surface be tested exhaustively without a keyboard
/// plugged in, and it is the reason this trait has exactly one method.
pub trait ChordNaming: Send + Sync {
    /// Every honest reading of `sounding`, best first, or why there is none.
    fn analyze(&self, sounding: &SoundingSet) -> ChordAnalysis;
}

/// What can go wrong opening or closing a sounding set source.
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    /// The platform's MIDI layer refused to open. The message is its own words.
    #[error("MIDI input could not start: {0}")]
    CouldNotStart(String),
    /// A named device was asked for and is not attached.
    #[error("no MIDI input named {0:?}")]
    NoSuchInput(String),
}

/// A source of sounding sets. Implemented by the MIDI crate, and by fakes in tests.
///
/// The receiver yields an already-settled `SoundingSet` — debouncing and sustain
/// bookkeeping happen behind this seam, so the UI never sees a half-struck chord.
///
/// Swift's `AsyncStream` becomes a `std::sync::mpsc::Receiver`: no async runtime
/// is in the workspace, and a channel is what every consumer can drain.
pub trait SoundingSetSource: Send {
    /// Yields each settled sounding set, including the empty one on full release.
    fn sounding_sets(&mut self) -> Receiver<SoundingSet>;

    /// The input devices currently attached, by the display name a person would
    /// recognise. Yields the whole current set on start and again on every change.
    ///
    /// A whole set rather than add/remove events, because the UI wants the current
    /// answer and never a history, and a set is idempotent.
    ///
    /// Default: a source that has no devices to report — every fake and every
    /// scripted source. The channel is closed, so a consumer's loop completes.
    fn connected_inputs(&mut self) -> Receiver<Vec<String>> {
        let (_tx, rx) = std::sync::mpsc::channel();
        rx
    }

    /// Yields a [`ControlEvent`] each time the sustain, sostenuto, or soft
    /// pedal changes.
    ///
    /// Default: a source that has no pedals to report — every fake and every
    /// scripted source unless it opts in. The channel is closed, so a
    /// consumer's loop completes.
    fn controls(&mut self) -> Receiver<ControlEvent> {
        let (_tx, rx) = std::sync::mpsc::channel();
        rx
    }

    /// Start listening. Succeeding says nothing about whether anything is plugged
    /// in; `connected_inputs` is what reports that.
    fn start(&mut self) -> Result<(), SourceError>;

    /// Stop listening. Idempotent.
    fn stop(&mut self);

    /// Adjusts the settle window live — no restart. Default: a no-op, for a
    /// source with no window to adjust.
    fn set_settle(&mut self, settle: Duration) {
        let _ = settle;
    }

    /// Restricts the source to one named input, or clears the restriction to
    /// listen to everything. Live — no restart.
    ///
    /// Default: a no-op, for a fake or a scripted source with no inputs to
    /// choose among.
    fn select_input(&mut self, name: Option<String>) {
        let _ = name;
    }
}

/// What can go wrong reading or writing the notes file.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// The file could not be read or written. The message is the OS's own words.
    #[error("notes file I/O failed: {0}")]
    Io(String),
    /// The file was read but is not the notes-file schema (ADR-0004).
    #[error("notes file is not readable: {0}")]
    Corrupt(String),
    /// A delete named an id that is not in the store.
    #[error("no note with id {0}")]
    NoSuchNote(String),
}

/// Persistence for text notes. Implemented by the store crate.
pub trait NoteStoring: Send + Sync {
    /// Notes for exactly this chord, newest first. Empty is a normal answer.
    fn notes(&self, key: &ChordKey) -> Result<Vec<ChordNote>, StoreError>;
    /// Every note, newest first.
    fn all_notes(&self) -> Result<Vec<ChordNote>, StoreError>;
    /// Attach `text` to `key`, recording `spelling` as the headline on screen.
    fn add(&self, text: &str, key: &ChordKey, spelling: &str) -> Result<ChordNote, StoreError>;
    /// Replace an existing note's text by id, keeping its key, spelling, and
    /// creation time. The file schema does not change (spec #9). `NoSuchNote`
    /// when `id` is not there.
    fn update(&self, id: &str, text: &str) -> Result<ChordNote, StoreError>;
    /// Remove one note by id.
    fn delete(&self, id: &str) -> Result<(), StoreError>;
}
