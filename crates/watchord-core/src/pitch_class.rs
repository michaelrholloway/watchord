//! Ported from note-view `Sources/NoteViewCore/PitchClass.swift`.

/// One of the twelve pitch classes, `0` = C through `11` = B.
///
/// MIDI is **spelling-blind** — it transmits key numbers, never letter names — so
/// a pitch class is the most the input can tell us. C♯ and D♭ are the same
/// `PitchClass`; distinguishing them is the *output* side's job (see
/// `ChordReading`), and it does so by generating candidate spellings rather than
/// by reading one off the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PitchClass(u8);

impl PitchClass {
    /// Wraps into `0..12`, so arithmetic on pitch classes never needs guarding.
    pub const fn new(raw: i32) -> Self {
        PitchClass(raw.rem_euclid(12) as u8)
    }

    /// The pitch class a MIDI note number sounds. Middle C (60) is `0`.
    pub const fn from_midi_note(note: u8) -> Self {
        PitchClass(note % 12)
    }

    pub const fn value(self) -> u8 {
        self.0
    }

    /// Semitones from `self` up to `other`, in `0..12`.
    pub fn interval_to(self, other: PitchClass) -> u8 {
        ((other.0 as i32 - self.0 as i32).rem_euclid(12)) as u8
    }

    pub fn transposed(self, semitones: i32) -> PitchClass {
        PitchClass::new(self.0 as i32 + semitones)
    }

    /// The twelve, ascending.
    pub fn all_cases() -> [PitchClass; 12] {
        std::array::from_fn(|i| PitchClass(i as u8))
    }
}

impl std::fmt::Display for PitchClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Encoded as a bare integer — `4`, not `{"value":4}`. Same reasoning as
/// `ChordKey`: the synthesized form leaks the struct's shape into the file format
/// and skips the wrapping initialiser on the way back in.
#[cfg(feature = "serde")]
impl serde::Serialize for PitchClass {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u8(self.0)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for PitchClass {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = i32::deserialize(d)?;
        Ok(PitchClass::new(raw))
    }
}
