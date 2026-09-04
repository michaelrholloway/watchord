//! Drill's seams (spec #9, ticket #14): the engine's naming vocabulary, and
//! per-chord drill stats.
//!
//! Both are seams for the same reason `ChordNaming` and `NoteStoring` are:
//! `watchord-model` never reaches into `watchord-engine`'s `QualityCatalog`
//! or a concrete store directly, so the model stays testable through fakes
//! and the composition root is the only place that names a concrete type.

use std::time::SystemTime;

use crate::{ChordKey, StoreError};

/// One chord the drill can name as a target: a name from the engine's
/// vocabulary and the pitch classes it claims.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct DrillTarget {
    /// The identity: the pitch-class set the target claims.
    pub key: ChordKey,
    /// The name as the engine would write it — e.g. `Cm7`.
    pub display: String,
}

/// The engine's naming vocabulary, as targets the drill can draw from.
///
/// Implemented by the engine crate's `EngineVocabulary`, which reads
/// `QualityCatalog::shared().complete_spellings` — additive and read-only,
/// touching no ranking, grammar, or fixture. The model sees this trait and
/// nothing else, the same way it sees `ChordNaming` and never `NamingEngine`.
pub trait ChordVocabulary: Send + Sync {
    /// Every chord the drill may name, in no particular order.
    fn targets(&self) -> Vec<DrillTarget>;
}

/// One chord's drill history: how many times it was drawn, how many of
/// those were graded exact, and when it was last tried.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct DrillChordStat {
    /// Which chord this row is about.
    pub chord_key: ChordKey,
    /// Times this chord has been the target and graded.
    pub attempts: u32,
    /// How many of those attempts graded `Exact`.
    pub exact: u32,
    /// When it was last attempted; `None` for a chord that has never come up.
    pub last_at: Option<SystemTime>,
}

impl DrillChordStat {
    /// A fresh row: never attempted.
    pub fn new(chord_key: ChordKey) -> Self {
        DrillChordStat {
            chord_key,
            attempts: 0,
            exact: 0,
            last_at: None,
        }
    }
}

/// Persistence for drill stats.
///
/// Implemented by the store crate's `JsonDrillStore`, writing `drill.json`
/// beside the notes file — never inside it. The notes file schema is a
/// contract with note-view and does not change for this (ADR-0004, spec #9).
pub trait DrillStoring: Send + Sync {
    /// Every chord with drill history. Empty is a normal answer.
    fn stats(&self) -> Result<Vec<DrillChordStat>, StoreError>;

    /// Records one attempt against `key`: increments `attempts`, increments
    /// `exact` when `exact` is true, and stamps `last_at` with `at`. Creates
    /// the row if this is the chord's first attempt.
    fn record(
        &self,
        key: &ChordKey,
        exact: bool,
        at: SystemTime,
    ) -> Result<DrillChordStat, StoreError>;
}
