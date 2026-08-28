//! How a reading and a sounding set are written on screen.
//!
//! Ported from note-view's `Sources/NoteViewApp/ReadingDisplay.swift` and
//! `NoteName.swift`. They live in core rather than in the model because they are
//! pure functions on core types, and both the engine fixture test and the model
//! need one implementation of them.

use std::collections::BTreeSet;

use crate::{ChordFit, ChordReading, PitchClass, SoundingSet, SpellingOrigin};

/// One `ChordReading` as the screen writes it: the name, the mark that changes
/// what the name **means**, and the detail that merely qualifies it.
///
/// `ChordReading::fit_note` produces one string for every tier — `·no5`, `·+2`,
/// `≈` — and drawing them all in the same place would be wrong:
///
/// - `·no5 ·no11` is a **detail**. It belongs under the headline, in the
///   secondary grey.
/// - `≈` is **not** a detail. A `Nearest` reading is the closest thing the engine
///   knows, not a chord it asserts. It belongs beside the name, at the name's size.
///
/// The slash lives here too: `C/E` and `C` are the same pitch classes and differ
/// only in which note is lowest, so the slash is display work rather than analysis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadingDisplay {
    /// The chord name, in slash form when the bass is not the root: `C/E`.
    pub name: String,

    /// `≈`, or `None`. Drawn **beside** `name`, at the name's size.
    pub approximation: Option<String>,

    /// `·no5 ·no11`, `·+2`, or `None`. Drawn **under** the headline. Always
    /// `None` for `Exact` and for `Nearest`.
    pub fit_detail: Option<String>,

    /// The reading said aloud — "C major 6".
    pub spoken: String,

    /// Which alternate-spelling axis produced it.
    pub origin: SpellingOrigin,
}

impl ReadingDisplay {
    /// `bass` is the lowest sounding pitch class. Pass `None` where there is no
    /// voicing to speak of — the All Notes headings are re-derived from a
    /// `ChordKey`, which is voicing-blind.
    pub fn new(reading: &ChordReading, bass: Option<PitchClass>) -> Self {
        let note = reading.fit_note();
        let note = if note.is_empty() { None } else { Some(note) };
        let (approximation, fit_detail) = match reading.fit {
            ChordFit::Nearest { .. } => (note, None),
            ChordFit::Exact | ChordFit::Missing(_) | ChordFit::Plus(_) => (None, note),
        };
        ReadingDisplay {
            name: Self::name_of(reading, bass),
            approximation,
            fit_detail,
            spoken: reading.spoken.clone(),
            origin: reading.origin,
        }
    }

    pub fn id(&self) -> String {
        format!("{}{}", self.name, self.approximation.as_deref().unwrap_or(""))
    }

    /// True when the engine does not stand behind this name.
    pub fn is_approximate(&self) -> bool {
        self.approximation.is_some()
    }

    /// `C` with E underneath is `C/E`; `C` with C underneath stays `C`.
    ///
    /// A bass that is not a chord tone at all (`C/D`, `F/G`) is deliberately not
    /// special-cased: those keys name as `Plus` or `Nearest`, which is honest.
    pub fn name_of(reading: &ChordReading, bass: Option<PitchClass>) -> String {
        let Some(bass) = bass else {
            return reading.display.clone();
        };
        if bass == reading.root {
            return reading.display.clone();
        }
        let written_flat = ChordRoot::parse(&reading.display, &reading.pitch_classes)
            .map(|r| r.is_flat)
            .unwrap_or(false);
        format!(
            "{}/{}",
            reading.display,
            NoteName::pitch_class(bass, written_flat)
        )
    }
}

/// The root read back off a written chord name.
///
/// Survives for one cosmetic job: whether the root was *written* flat, which
/// decides how the bass of a slash chord is spelled (`Bbm7/Db`, not `Bbm7/C#`).
/// A leading `b` or `#` is ambiguous in this notation — `Bb9` is a B-flat ninth
/// or a B dominant with a flat ninth — so both candidates are tried against the
/// pitch classes the reading claims, and the one that is present wins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChordRoot {
    pub pitch_class: PitchClass,
    /// True when the root was written with a flat.
    pub is_flat: bool,
}

impl ChordRoot {
    pub fn parse(display: &str, claiming: &BTreeSet<PitchClass>) -> Option<Self> {
        let mut chars = display.chars();
        let letter = chars.next()?;
        let natural: i32 = match letter {
            'C' => 0,
            'D' => 2,
            'E' => 4,
            'F' => 5,
            'G' => 7,
            'A' => 9,
            'B' => 11,
            _ => return None,
        };
        let accidental: Option<i32> = match chars.next() {
            Some('#') => Some(1),
            Some('b') => Some(-1),
            _ => None,
        };
        let Some(accidental) = accidental else {
            return Some(ChordRoot {
                pitch_class: PitchClass::new(natural),
                is_flat: false,
            });
        };
        let altered = PitchClass::new(natural + accidental);
        let plain = PitchClass::new(natural);
        if claiming.contains(&altered) || !claiming.contains(&plain) {
            Some(ChordRoot {
                pitch_class: altered,
                is_flat: accidental < 0,
            })
        } else {
            Some(ChordRoot {
                pitch_class: plain,
                is_flat: false,
            })
        }
    }
}

/// How a raw MIDI note number is written for the `keys` row.
///
/// A **display** concern, not naming: it never claims a chordal spelling. Two
/// choices nobody ruled on: sharps only, and middle C (MIDI 60) is `C3`, the
/// Yamaha/Roland convention.
pub struct NoteName;

impl NoteName {
    pub const PITCH_CLASS_NAMES: [&'static str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];

    /// The same twelve written with flats. Used for **one** thing: the bass of a
    /// slash chord whose root is written with a flat.
    pub const FLAT_PITCH_CLASS_NAMES: [&'static str; 12] = [
        "C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B",
    ];

    /// MIDI 60 renders as `C3`.
    pub const MIDDLE_C_OCTAVE: i32 = 3;

    pub fn pitch_class(pitch_class: PitchClass, preferring_flats: bool) -> &'static str {
        let names = if preferring_flats {
            &Self::FLAT_PITCH_CLASS_NAMES
        } else {
            &Self::PITCH_CLASS_NAMES
        };
        names[pitch_class.value() as usize]
    }

    /// `60` -> `"C3"`.
    pub fn note(midi_note: u8) -> String {
        let value = midi_note as i32;
        let octave = value / 12 - (60 / 12 - Self::MIDDLE_C_OCTAVE);
        format!("{}{}", Self::PITCH_CLASS_NAMES[(value % 12) as usize], octave)
    }

    /// The whole sounding set as one readable row: `"C3  E3  G3  A3"`.
    pub fn keys_row(sounding: &SoundingSet) -> String {
        sounding
            .midi_notes()
            .iter()
            .map(|&n| Self::note(n))
            .collect::<Vec<_>>()
            .join("  ")
    }

    /// The pitch classes of a set, ascending: `"C  E  G  A"`. Used when the engine
    /// declined for having too many distinct pitch classes.
    pub fn pitch_classes_row(sounding: &SoundingSet) -> String {
        sounding
            .pitch_classes()
            .iter()
            .map(|&pc| Self::pitch_class(pc, false))
            .collect::<Vec<_>>()
            .join("  ")
    }
}
