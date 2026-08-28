//! Ported from note-view `Sources/NoteViewCore/Tuning.swift`.
//!
//! Every number in this app that was guessed rather than measured.
//!
//! **None of these are Michael's numbers.** They are engineering guesses made by
//! an agent standing in for him, and they are gathered here — rather than
//! scattered across the modules that use them — so that when he plays it and
//! disagrees, the fix is editing one value in one file.
//!
//! The Swift engine is the source of truth (ADR-0002): change a number there,
//! regenerate the fixture, then change it here.

use std::time::Duration;

/// How long the held-note set must stop changing before it is named.
pub const SETTLE_INTERVAL: Duration = Duration::from_millis(60);

/// Fewest distinct pitch classes worth attempting a chord name for.
pub const MINIMUM_PITCH_CLASSES: usize = 2;

/// Most distinct pitch classes worth attempting a chord name for. Past this,
/// candidate generation explodes and every answer is noise, so the engine
/// declines and reports the pitch classes plainly instead.
pub const MAXIMUM_PITCH_CLASSES: usize = 8;

/// MIDI CC number for the sustain pedal.
pub const SUSTAIN_CONTROLLER: u8 = 64;

/// CC value at or above which sustain counts as down.
pub const SUSTAIN_ON_THRESHOLD: u8 = 64;

/// Ranking weights.
pub mod weight {
    /// The dominant term. If C is the lowest sounding note, `C6` should beat
    /// `Am7` for the same four keys almost regardless of what else is true.
    pub const ROOT_IS_BASS: i32 = 100;
    /// Rootless voicings are real, but a reading that accounts for a root
    /// present in the set is likelier to be what was meant.
    pub const ROOT_IS_PRESENT: i32 = 20;
    /// Every sounding key is explained by the reading.
    pub const ALL_TONES_ACCOUNTED_FOR: i32 = 15;
    /// Charged per extension or alteration, so `C6` beats a contorted reading
    /// of the same keys carrying a 13th and a missing 5th.
    pub const PER_ALTERATION: i32 = -6;
    /// Faint tiebreak — `C` reads more easily than `B#`.
    pub const NATURAL_ROOT: i32 = 3;
    /// Fainter still. Only ever separates two otherwise equal readings.
    pub const SIMPLER_ACCIDENTAL: i32 = 2;
}

/// What each kind of mismatch costs when ranking a reading against the keys that
/// are actually sounding. Summed into a **fit cost**, compared **before any score
/// term**.
///
/// **Zero is load-bearing at the top and non-zero is load-bearing at the
/// bottom.** A sanctioned omission costing nothing is what keeps 311 measured
/// headlines on the right names; `UNEXPLAINED_KEY` being non-zero is what stops a
/// reading that ignores a sounding key from ever outranking one that explains
/// them all.
pub mod fit {
    /// A note the spelling itself declared droppable. **Free**: the grammar has
    /// already ruled the chord is still that chord without it.
    pub const SANCTIONED_OMISSION: i32 = 0;
    /// A perfect fifth this particular spelling did *not* declare droppable.
    pub const MISSING_PERFECT_FIFTH: i32 = 1;
    /// Any other added tone — a `6`, an `add9`, a `b9`.
    pub const MISSING_ADDED_TONE: i32 = 2;
    /// A sounding key the spelling cannot account for.
    pub const UNEXPLAINED_KEY: i32 = 3;
    /// Root, third, seventh — and a fifth that has been *moved*.
    pub const MISSING_IDENTITY_TONE: i32 = 6;
}

/// How common a chord quality is, as a bonus in `0..=18`. Ordinary triads and
/// dominant sevenths sit at the top; the exotica at the bottom must earn its
/// place on the other terms. `power` sits at the `sus4` tier;
/// `diminishedMajor7` at the bottom, under `augmentedMajor7`.
pub const QUALITY_COMMONNESS: [(&str, i32); 18] = [
    ("major", 18),
    ("minor", 17),
    ("dominant7", 16),
    ("major7", 15),
    ("minor7", 15),
    ("sus4", 12),
    ("sus2", 11),
    ("major6", 12),
    ("minor6", 10),
    ("diminished", 10),
    ("augmented", 8),
    ("halfDiminished", 9),
    ("fullyDiminished", 9),
    ("minorMajor7", 6),
    ("augmented7", 5),
    ("augmentedMajor7", 4),
    ("power", 12),
    ("diminishedMajor7", 3),
];

/// The commonness bonus for a quality key, or `None` if nothing weights it.
pub fn quality_commonness(key: &str) -> Option<i32> {
    QUALITY_COMMONNESS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| *v)
}
