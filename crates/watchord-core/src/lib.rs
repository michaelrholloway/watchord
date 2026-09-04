//! watchord-core: the types and seams every other crate speaks.
//!
//! Ported from note-view's `NoteViewCore` target. File for file: `pitch_class`,
//! `sounding_set`, `chord_key`, `chord_note`, `chord_reading`, `tuning`, `seams`.
//! `display` carries the pure display formatters from note-view's app target
//! (`ReadingDisplay`, `NoteName`), because both the engine fixture test and the
//! model need them and they are functions on core types.

mod chord_key;
mod chord_note;
mod chord_reading;
mod display;
mod drill;
mod pitch_class;
mod seams;
mod sounding_set;
pub mod tuning;

pub use chord_key::ChordKey;
pub use chord_note::ChordNote;
pub use chord_reading::{ChordAnalysis, ChordFit, ChordReading, DeclineReason, SpellingOrigin};
pub use display::{ChordRoot, NoteName, ReadingDisplay};
pub use drill::{ChordVocabulary, DrillChordStat, DrillStoring, DrillTarget};
pub use pitch_class::PitchClass;
pub use seams::{
    ChordNaming, ControlEvent, NoteStoring, PedalKind, SoundingSetSource, SourceError, StoreError,
};
pub use sounding_set::SoundingSet;
