//! Ported from note-view `Sources/NoteViewCore/ChordNote.swift`.

use std::time::SystemTime;

use crate::ChordKey;

/// A piece of text Michael attached to a chord.
///
/// The JSON shape (`id` as a UUID string, `createdAt` as ISO 8601) is the notes
/// file contract, ADR-0004. The store crate owns that encoding; this type carries
/// the values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChordNote {
    /// The note's identity, as note-view writes it: a UUID string.
    pub id: String,

    /// The chord this belongs to — the pitch-class set.
    pub chord_key: ChordKey,

    /// The headline that was on screen when the note was written.
    ///
    /// One key set has many honest names. Without this, a note written while the
    /// display read `C6` resurfaces one day under `Am7` with no trace of what he
    /// was looking at. Recording it costs a string and preserves the context the
    /// note was actually about.
    pub spelling_when_written: String,

    /// What Michael wrote.
    pub text: String,
    /// When it was written.
    pub created_at: SystemTime,
}

impl ChordNote {
    /// Builds a note from its stored values. The store crate assigns `id` and `created_at`.
    pub fn new(
        id: impl Into<String>,
        chord_key: ChordKey,
        spelling_when_written: impl Into<String>,
        text: impl Into<String>,
        created_at: SystemTime,
    ) -> Self {
        ChordNote {
            id: id.into(),
            chord_key,
            spelling_when_written: spelling_when_written.into(),
            text: text.into(),
            created_at,
        }
    }
}
