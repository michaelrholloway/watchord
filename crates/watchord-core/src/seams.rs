//! Ported from note-view `Sources/NoteViewCore/Seams.swift`, plus the terminal
//! skin's own seam, `Frame`.

use std::sync::mpsc::Receiver;

use crate::{ChordAnalysis, ChordKey, ChordNote, SoundingSet};

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

    /// Start listening. Succeeding says nothing about whether anything is plugged
    /// in; `connected_inputs` is what reports that.
    fn start(&mut self) -> Result<(), SourceError>;

    /// Stop listening. Idempotent.
    fn stop(&mut self);
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
    /// Remove one note by id.
    fn delete(&self, id: &str) -> Result<(), StoreError>;
}

/// The terminal skin's seam: draws a model snapshot into a buffer.
///
/// The model is the terminal-free state machine; a `Frame` is the one thing that
/// turns it into cells. Minimal on purpose — the tui ticket grows it. The snapshot
/// and buffer types are the implementor's, so this crate stays free of terminal
/// code.
pub trait Frame {
    /// What one frame is drawn from.
    type Snapshot;
    /// Where it is drawn to.
    type Buffer;

    /// Draw `snapshot` into `buffer`.
    fn draw(&self, snapshot: &Self::Snapshot, buffer: &mut Self::Buffer);
}
