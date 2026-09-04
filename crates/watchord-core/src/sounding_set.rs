//! Ported from note-view `Sources/NoteViewCore/SoundingSet.swift`.

use std::collections::BTreeSet;

use crate::{ChordKey, PitchClass};

/// What is audibly sounding at one instant: the held MIDI notes, as gathered by
/// the input layer.
///
/// This is the engine's input. It carries the raw notes rather than only the
/// pitch-class set because **the bass note is the strongest single term in the
/// naming heuristic** — it is the one piece of voicing information that survives
/// into the name. Identity (`ChordKey`) still ignores it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct SoundingSet {
    /// Distinct sounding MIDI note numbers, ascending.
    midi_notes: Vec<u8>,
}

impl SoundingSet {
    /// Builds a set from any notes; duplicates collapse and the order is ascending.
    pub fn new(midi_notes: impl IntoIterator<Item = u8>) -> Self {
        let set: BTreeSet<u8> = midi_notes.into_iter().collect();
        SoundingSet {
            midi_notes: set.into_iter().collect(),
        }
    }

    /// Nothing held.
    pub fn silent() -> Self {
        SoundingSet::default()
    }

    /// Distinct sounding MIDI note numbers, ascending.
    pub fn midi_notes(&self) -> &[u8] {
        &self.midi_notes
    }

    /// The distinct pitch classes sounding, ascending.
    pub fn pitch_classes(&self) -> BTreeSet<PitchClass> {
        self.midi_notes
            .iter()
            .map(|&n| PitchClass::from_midi_note(n))
            .collect()
    }

    /// The lowest sounding note, or `None` when nothing is held.
    pub fn bass(&self) -> Option<u8> {
        self.midi_notes.first().copied()
    }

    /// The pitch class of the lowest sounding note, or `None` when nothing is held.
    pub fn bass_pitch_class(&self) -> Option<PitchClass> {
        self.bass().map(PitchClass::from_midi_note)
    }

    /// The identity of what is sounding: its pitch-class set as a `ChordKey`.
    pub fn key(&self) -> ChordKey {
        ChordKey::new(self.pitch_classes())
    }

    /// True when nothing is held.
    pub fn is_empty(&self) -> bool {
        self.midi_notes.is_empty()
    }

    /// Distinct pitch classes — the count the naming floor and ceiling are read
    /// against, *not* `midi_notes.len()`. An octave-doubled triad is three, not six.
    pub fn distinct_pitch_class_count(&self) -> usize {
        self.pitch_classes().len()
    }
}

/// Encoded as a bare array of MIDI note numbers — `[60,64,67,69]`, not
/// `{"midi_notes":[...]}`. Same reasoning as `PitchClass`: the wire shape is the
/// value, and the way back in goes through `new`, so a stray duplicate or a
/// wrong order cannot arrive.
#[cfg(feature = "serde")]
impl serde::Serialize for SoundingSet {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.midi_notes.serialize(s)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for SoundingSet {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let notes = Vec::<u8>::deserialize(d)?;
        Ok(SoundingSet::new(notes))
    }
}
