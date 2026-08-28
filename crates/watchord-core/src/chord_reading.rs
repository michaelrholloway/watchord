//! Ported from note-view `Sources/NoteViewCore/ChordReading.swift`.

use std::collections::BTreeSet;

use crate::{ChordKey, PitchClass, SoundingSet};

/// Which of the two "alternate spelling" axes produced a reading.
///
/// Both axes are real and they are different operations, so the UI labels them
/// separately rather than pooling them into one undifferentiated list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpellingOrigin {
    /// Rank 1 — the headline.
    Headline,
    /// Same keys read from a different root: `C6` vs `Am7`.
    ReRooted,
    /// Same keys, same root, different letters: `C#Δ7` vs `DbΔ7`.
    Enharmonic,
}

impl SpellingOrigin {
    /// The Swift raw value: `headline`, `reRooted`, `enharmonic`.
    pub fn raw_value(self) -> &'static str {
        match self {
            SpellingOrigin::Headline => "headline",
            SpellingOrigin::ReRooted => "reRooted",
            SpellingOrigin::Enharmonic => "enharmonic",
        }
    }
}

/// How well a reading actually fits the keys that are sounding.
///
/// Michael, 2026-08-10: *"i would like for program to always try to suggest a
/// chord, or a close one."* Honesty is kept, but moved: instead of a **gate** that
/// refuses to speak, it is a **property** that is always shown. The engine may
/// name a chord it does not exactly match, and the screen says so in the same
/// breath. What must never weaken is the other direction — every sounding key
/// stays accounted for, or the reading is `Nearest` and admits it.
///
/// Ranked strictly by tier before anything else, so an exact name always beats an
/// approximate one and `Nearest` appears only when nothing fits.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ChordFit {
    /// The reading's pitch classes equal the sounding keys exactly.
    Exact,
    /// Every sounding key is explained, but the chord has tones you did not play.
    /// `C13` from `C E Bb D A` — no 5th, no 11th. Carries which degrees are absent
    /// so the screen can say `·no5`.
    Missing(Vec<u8>),
    /// The whole chord is sounding, plus keys it cannot explain.
    /// Carries the unexplained pitch classes.
    Plus(Vec<PitchClass>),
    /// Neither — the closest thing the engine knows, by weighted distance. The
    /// only tier that is allowed to leave a sounding key unexplained *and* claim
    /// tones you did not play, and it is marked so it can never be mistaken for
    /// a name the engine stands behind.
    Nearest { distance: i32 },
}

impl ChordFit {
    /// Rank order. Lower sorts first, and this dominates every score term —
    /// a worse-fitting reading never outranks a better-fitting one.
    pub fn tier(&self) -> u8 {
        match self {
            ChordFit::Exact => 0,
            ChordFit::Missing(_) => 1,
            ChordFit::Plus(_) => 2,
            ChordFit::Nearest { .. } => 3,
        }
    }

    /// The tier's name as the fixture writes it: `exact`, `missing`, `plus`, `nearest`.
    pub fn tier_name(&self) -> &'static str {
        match self {
            ChordFit::Exact => "exact",
            ChordFit::Missing(_) => "missing",
            ChordFit::Plus(_) => "plus",
            ChordFit::Nearest { .. } => "nearest",
        }
    }

    /// True only for `Exact`.
    pub fn is_exact(&self) -> bool {
        *self == ChordFit::Exact
    }
}

impl PartialOrd for ChordFit {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.tier().cmp(&other.tier()))
    }
}

const SHARP_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// One name for a set of sounding keys, carrying how well it fits.
///
/// A reading is produced by writing a name and computing forward from the grammar
/// what that name would sound like; the engine never invents intervals. A
/// non-exact match is *reported with its fit* rather than suppressed.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChordReading {
    /// The chord in Michael's notation, one line of plain text — `C#-dim7`.
    pub display: String,

    /// The same chord spoken aloud — "C sharp dim, dim 7".
    pub spoken: String,

    /// Which alternate-spelling axis produced this reading.
    pub origin: SpellingOrigin,

    /// Ranking score. Higher wins.
    pub score: i32,

    /// The pitch classes this reading claims. Carried so the claim is checkable
    /// rather than trusted.
    pub pitch_classes: BTreeSet<PitchClass>,

    /// How well this reading fits what is actually sounding.
    pub fit: ChordFit,

    /// The pitch class this reading is rooted on. Carried rather than derived;
    /// the app needs it to decide whether to write a slash bass.
    pub root: PitchClass,
}

impl ChordReading {
    /// A stable identity for the screen: the display string, unique across the catalog.
    pub fn id(&self) -> &str {
        &self.display
    }

    /// A short suffix naming the fit, for the screen — `·no5`, `·+F#`, `≈`.
    /// Empty for an exact reading, which is the common case and needs no mark.
    pub fn fit_note(&self) -> String {
        match &self.fit {
            ChordFit::Exact => String::new(),
            ChordFit::Missing(degrees) => {
                let mut degrees = degrees.clone();
                degrees.sort_unstable();
                degrees
                    .iter()
                    .map(|d| format!("·no{d}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            }
            ChordFit::Plus(extras) => {
                // Name the keys, don't count them. Sharps throughout — MIDI is
                // spelling-blind, so any letter here is a rendering choice rather
                // than a claim about what was meant.
                let mut extras = extras.clone();
                extras.sort_unstable();
                extras
                    .iter()
                    .map(|pc| format!("·+{}", SHARP_NAMES[pc.value() as usize]))
                    .collect::<Vec<_>>()
                    .join(" ")
            }
            ChordFit::Nearest { .. } => "≈".to_string(),
        }
    }
}

/// Set when the engine declined to name rather than failed to. The UI shows this
/// instead of a name so "I won't guess" never looks like "I crashed".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DeclineReason {
    /// Nothing is held.
    Silent,
    /// Fewer distinct pitch classes than `tuning::MINIMUM_PITCH_CLASSES`.
    SingleNote,
    /// More distinct pitch classes than `tuning::MAXIMUM_PITCH_CLASSES`.
    TooManyPitchClasses,
    /// Inside the window, but nothing in the vocabulary comes close enough to offer.
    NoHonestReading,
}

impl DeclineReason {
    /// The Swift raw value: `silent`, `singleNote`, `tooManyPitchClasses`, `noHonestReading`.
    pub fn raw_value(self) -> &'static str {
        match self {
            DeclineReason::Silent => "silent",
            DeclineReason::SingleNote => "singleNote",
            DeclineReason::TooManyPitchClasses => "tooManyPitchClasses",
            DeclineReason::NoHonestReading => "noHonestReading",
        }
    }
}

/// Everything the display needs about one sounding set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChordAnalysis {
    /// What was analysed.
    pub sounding: SoundingSet,
    /// The identity of `sounding`, for notes lookup.
    pub key: ChordKey,

    /// Rank 1, or `None` when nothing nameable is sounding — silence, a single
    /// note, or a set past the naming ceiling.
    pub headline: Option<ChordReading>,

    /// Every other honest reading, best first.
    pub alternates: Vec<ChordReading>,

    /// Why there is no headline, when there is none.
    pub declined_reason: Option<DeclineReason>,
}

impl ChordAnalysis {
    /// Builds an analysis; `key` is derived from `sounding`.
    pub fn new(
        sounding: SoundingSet,
        headline: Option<ChordReading>,
        alternates: Vec<ChordReading>,
        declined_reason: Option<DeclineReason>,
    ) -> Self {
        let key = sounding.key();
        ChordAnalysis {
            sounding,
            key,
            headline,
            alternates,
            declined_reason,
        }
    }

    /// The analysis of nothing sounding: no headline, declined as `Silent`.
    pub fn silence() -> Self {
        ChordAnalysis::new(
            SoundingSet::silent(),
            None,
            vec![],
            Some(DeclineReason::Silent),
        )
    }
}
