//! Which alphabet — sharp or flat — writes a pitch class.
//!
//! `ChordReading` carries a final `display` string and the claimed pitch-class
//! *set*, not a per-degree letter (that lives only in the engine's internal
//! `SpelledChord`/`NoteSpelling`, which this crate must not depend on —
//! ADR-0002 keeps annotation work out of the engine). So the letter a claimed
//! pitch class takes here is the one its own alphabet gives it: flat when the
//! headline's root is written flat (mirroring `ReadingDisplay`'s
//! `ChordRoot::parse(...).is_flat` bass-spelling rule, not a second rule), sharp
//! otherwise. A chord that genuinely mixes flats and sharps in one written name
//! (rare, and always an extension/alteration token) spells a shade more
//! uniformly here than the engine's own per-degree grammar would. That is a
//! known limit of a pure theory crate with no engine access, not an oversight.

use watchord_core::{ChordReading, ChordRoot, PitchClass};

/// `#`, `b`, or nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Accidental {
    Sharp,
    Flat,
    Natural,
}

impl Accidental {
    /// `#`, `b`, or empty.
    pub fn symbol(self) -> &'static str {
        match self {
            Accidental::Sharp => "#",
            Accidental::Flat => "b",
            Accidental::Natural => "",
        }
    }
}

/// One spelled note: a letter `A`-`G` and its accidental. No octave — that is
/// carried separately by whatever raw MIDI note this spelling is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Spelled {
    pub letter: char,
    pub accidental: Accidental,
}

/// `(letter, accidental)` for pitch classes `0..12`, spelled sharp. Chromatic
/// pitch classes borrow the natural letter directly below them in the same
/// octave — no letter ever wraps into a neighbouring octave here, which is what
/// lets [`letter_index`] treat "same physical octave as the sounding note" as a
/// safe assumption.
const SHARP: [(char, Accidental); 12] = [
    ('C', Accidental::Natural),
    ('C', Accidental::Sharp),
    ('D', Accidental::Natural),
    ('D', Accidental::Sharp),
    ('E', Accidental::Natural),
    ('F', Accidental::Natural),
    ('F', Accidental::Sharp),
    ('G', Accidental::Natural),
    ('G', Accidental::Sharp),
    ('A', Accidental::Natural),
    ('A', Accidental::Sharp),
    ('B', Accidental::Natural),
];

/// Flat counterpart of [`SHARP`]: chromatic pitch classes borrow the natural
/// letter directly above them, same octave.
const FLAT: [(char, Accidental); 12] = [
    ('C', Accidental::Natural),
    ('D', Accidental::Flat),
    ('D', Accidental::Natural),
    ('E', Accidental::Flat),
    ('E', Accidental::Natural),
    ('F', Accidental::Natural),
    ('G', Accidental::Flat),
    ('G', Accidental::Natural),
    ('A', Accidental::Flat),
    ('A', Accidental::Natural),
    ('B', Accidental::Flat),
    ('B', Accidental::Natural),
];

/// The letter and accidental a pitch class takes, sharp unless `flat`.
pub fn spell(pc: PitchClass, flat: bool) -> Spelled {
    let (letter, accidental) = (if flat { FLAT } else { SHARP })[pc.value() as usize];
    Spelled { letter, accidental }
}

/// `letter`'s position in the scale, `C` = 0 through `B` = 6.
pub fn letter_to_index(letter: char) -> i32 {
    match letter {
        'C' => 0,
        'D' => 1,
        'E' => 2,
        'F' => 3,
        'G' => 4,
        'A' => 5,
        'B' => 6,
        _ => unreachable!("spell() only ever writes A-G"),
    }
}

/// The letter index (`C` = 0 .. `B` = 6) [`spell`] gives a pitch class — the
/// piece [`crate::staff`] needs to place a note on a staff row.
pub fn letter_index(pc: PitchClass, flat: bool) -> i32 {
    letter_to_index(spell(pc, flat).letter)
}

/// True when `reading`'s root is written with a flat. Mirrors
/// `watchord_core::ReadingDisplay::name_of`'s bass-spelling rule
/// (`ChordRoot::parse(...).is_flat`) rather than inventing a second one.
pub fn is_written_flat(reading: &ChordReading) -> bool {
    ChordRoot::parse(&reading.display, &reading.pitch_classes).is_some_and(|r| r.is_flat)
}
