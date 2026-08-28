//! watchord-engine: names a sounding set in Michael's notation.
//!
//! Ported file for file from note-view's `ChordEngine` target: `note_spelling`,
//! `chord_grammar`, `quality_catalog`, `chord_ranking`, `chord_engine`. The Swift
//! engine is the source of truth (ADR-0002); the fixture test proves parity.

pub mod chord_engine;
pub mod chord_grammar;
pub mod chord_ranking;
pub mod note_spelling;
pub mod quality_catalog;

pub use chord_engine::NamingEngine;
pub use chord_grammar::{
    ChordDegree, ChordExtension, ChordSpelling, Effect, SeventhQuality, SpelledChord, SpelledTone,
    TriadQuality,
};
pub use note_spelling::{NoteLetter, NoteSpelling};
