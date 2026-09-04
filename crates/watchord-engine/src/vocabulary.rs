//! Exposes the engine's naming vocabulary as a `ChordVocabulary`, for the
//! drill (spec #9, ticket #14).
//!
//! Additive and read-only: it reads `QualityCatalog::shared().complete_spellings`,
//! which was already `pub`, and touches no ranking, grammar, or fixture.

use watchord_core::{ChordKey, ChordVocabulary, DrillTarget};

use crate::quality_catalog::QualityCatalog;

/// The engine's vocabulary of nameable chords, one target per unique
/// writable spelling — the same list `readings()` draws its exact matches
/// from, at full strength (no notes omitted).
#[derive(Clone, Copy, Debug, Default)]
pub struct EngineVocabulary;

impl EngineVocabulary {
    /// The vocabulary. Stateless, so this is the same as `Default`.
    pub fn new() -> Self {
        EngineVocabulary
    }
}

impl ChordVocabulary for EngineVocabulary {
    fn targets(&self) -> Vec<DrillTarget> {
        QualityCatalog::shared()
            .complete_spellings
            .iter()
            .map(|entry| DrillTarget {
                key: ChordKey::new(entry.pitch_classes.iter().copied()),
                display: entry.display(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_vocabulary_is_the_catalogs_complete_spellings_and_nothing_hand_typed() {
        let targets = EngineVocabulary::new().targets();
        assert_eq!(
            targets.len(),
            QualityCatalog::shared().complete_spellings.len()
        );
        assert!(targets.iter().any(|t| t.display == "C"));
        assert!(targets.iter().any(|t| t.display == "Cm7"));
        // Every target's key is exactly its claimed pitch classes.
        let c_major = targets.iter().find(|t| t.display == "C").unwrap();
        assert_eq!(
            c_major.key,
            ChordKey::new([0, 4, 7].map(watchord_core::PitchClass::new))
        );
    }
}
