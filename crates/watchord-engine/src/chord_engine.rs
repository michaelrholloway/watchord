//! Ported from note-view `Sources/ChordEngine/ChordEngine.swift`.

use watchord_core::tuning;
use watchord_core::{
    ChordAnalysis, ChordNaming, ChordReading, DeclineReason, PitchClass, SoundingSet,
    SpellingOrigin,
};

use crate::chord_ranking::{ChordRanking, ScoredReading};
use crate::quality_catalog::{CatalogEntry, QualityCatalog};

/// Names a sounding set in Michael's notation, with every other honest reading of
/// the same keys beneath it.
///
/// Pure: no I/O, no clock, no hardware, no stored state. The same keys always give
/// byte-identical output.
///
/// **The engine never asserts a spelling it has not verified.** Every reading it
/// emits was produced by writing a name and computing forward from the grammar
/// what that name would sound like, then measuring that against the keys.
/// `QualityCatalog` is built in the forward direction and `analyze` only ever
/// looks a chord up in it.
#[derive(Clone, Copy, Debug, Default)]
pub struct NamingEngine;

impl NamingEngine {
    /// The engine. Stateless, so this is the same as `Default`.
    pub fn new() -> Self {
        NamingEngine
    }

    fn scoring(candidates: &[CatalogEntry], sounding: &SoundingSet) -> Vec<ScoredReading> {
        let pitch_classes = sounding.pitch_classes();
        candidates
            .iter()
            .map(|entry| {
                let score = ChordRanking::score(entry, sounding.bass_pitch_class(), &pitch_classes);
                ScoredReading::measured(entry.clone(), &pitch_classes, score)
            })
            .collect()
    }

    /// Why the engine will not attempt a name, or `None` to go ahead. The floor
    /// and ceiling are read against **distinct pitch classes**.
    fn decline_reason(pitch_class_count: usize) -> Option<DeclineReason> {
        if pitch_class_count == 0 {
            Some(DeclineReason::Silent)
        } else if pitch_class_count < tuning::MINIMUM_PITCH_CLASSES {
            Some(DeclineReason::SingleNote)
        } else if pitch_class_count > tuning::MAXIMUM_PITCH_CLASSES {
            Some(DeclineReason::TooManyPitchClasses)
        } else {
            None
        }
    }

    /// Declining is a normal output, not an error.
    fn declined(sounding: &SoundingSet, reason: DeclineReason) -> ChordAnalysis {
        ChordAnalysis::new(sounding.clone(), None, vec![], Some(reason))
    }

    /// Which of the two alternate-spelling axes a reading came off. Rank 1 is the
    /// headline; a different root is a re-rooting, the same root written
    /// differently is an enharmonic respelling.
    fn origin(rank: usize, root: PitchClass, headline_root: PitchClass) -> SpellingOrigin {
        if rank == 0 {
            SpellingOrigin::Headline
        } else if root == headline_root {
            SpellingOrigin::Enharmonic
        } else {
            SpellingOrigin::ReRooted
        }
    }
}

impl ChordNaming for NamingEngine {
    fn analyze(&self, sounding: &SoundingSet) -> ChordAnalysis {
        let pitch_classes = sounding.pitch_classes();

        if let Some(reason) = Self::decline_reason(pitch_classes.len()) {
            return Self::declined(sounding, reason);
        }

        let catalog = QualityCatalog::shared();
        let candidates = catalog.readings(QualityCatalog::mask(pitch_classes.iter().copied()));
        let ranked = ChordRanking::rank(Self::scoring(candidates, sounding));
        let Some(best) = ranked.first() else {
            return Self::declined(sounding, DeclineReason::NoHonestReading);
        };

        let headline_root = best.entry.root().pitch_class();
        let mut readings: Vec<ChordReading> = ranked
            .into_iter()
            .enumerate()
            .map(|(rank, scored)| ChordReading {
                display: scored.entry.display(),
                spoken: scored.entry.chord.spelling.spoken(),
                origin: Self::origin(rank, scored.entry.root().pitch_class(), headline_root),
                score: scored.score,
                pitch_classes: scored.entry.chord.pitch_classes.clone(),
                fit: scored.fit,
                root: scored.entry.root().pitch_class(),
            })
            .collect();

        let headline = readings.remove(0);
        ChordAnalysis::new(sounding.clone(), Some(headline), readings, None)
    }
}
