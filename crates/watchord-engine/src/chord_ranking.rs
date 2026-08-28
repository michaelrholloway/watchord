//! Ported from note-view `Sources/ChordEngine/ChordRanking.swift`.

use std::collections::BTreeSet;

use watchord_core::tuning;
use watchord_core::{ChordFit, PitchClass};

use crate::quality_catalog::{CatalogEntry, Mask, QualityCatalog};
use crate::{SpelledChord, SpelledTone};

/// A candidate with the fit it has and the score it earned.
///
/// **Fit is a fact, score is an opinion.** Fit is measured against the keys that
/// are down and cannot be argued with; the score is a sum of guessed weights
/// saying which of several true readings a player probably meant. So the fact is
/// read first and the opinion only ever separates readings that fit alike.
#[derive(Clone, Debug)]
pub struct ScoredReading {
    /// The candidate.
    pub entry: CatalogEntry,
    /// Which tier it landed in.
    pub fit: ChordFit,
    /// The weighted distance from the sounding keys — 0 for `Exact`, and rising
    /// with how dear the differences are. `ChordFit` compares on its tier alone,
    /// and two readings in the same tier are still not equally far away.
    pub fit_cost: i32,
    /// The ranking opinion; higher wins inside a tier.
    pub score: i32,
}

impl ScoredReading {
    /// The ordinary construction: measure the fit, don't be told it.
    pub fn measured(entry: CatalogEntry, sounding: &BTreeSet<PitchClass>, score: i32) -> Self {
        let (fit, fit_cost) = ChordFitting::measure(&entry.chord, sounding);
        ScoredReading {
            entry,
            fit,
            fit_cost,
            score,
        }
    }

    /// For constructing a candidate whose fit is *stated* rather than measured —
    /// the only caller is a test that has to put a high-scoring bad fit next to a
    /// low-scoring good one and watch the good one win.
    pub fn stated(entry: CatalogEntry, fit: ChordFit, fit_cost: i32, score: i32) -> Self {
        ScoredReading {
            entry,
            fit,
            fit_cost,
            score,
        }
    }
}

/// How well a spelling fits the keys that are down.
///
/// The measurement is taken in **both** directions at once. `ChordFit` names
/// which of the four shapes came out; the **cost** says how dear the difference
/// was, and the cost is what the ranking reads. A difference the grammar already
/// sanctioned costs nothing; everything it did *not* sanction costs something and
/// therefore loses to everything that fits, ahead of any score term. `Plus` and
/// `Nearest` start at `UNEXPLAINED_KEY` and can never reach zero, which is the
/// honesty invariant expressed as an ordering.
pub struct ChordFitting;

impl ChordFitting {
    /// Which of the four tiers a spelling lands in against these keys, on the masks.
    pub fn tier(spelling: Mask, sounding: Mask) -> u8 {
        match (spelling & !sounding == 0, sounding & !spelling == 0) {
            (true, true) => 0,
            (false, true) => 1,
            (true, false) => 2,
            (false, false) => 3,
        }
    }

    /// The weighted distance between a spelling and the keys, from the fingerprints.
    pub fn cost(tones: &[SpelledTone], spelling: Mask, sounding: Mask) -> i32 {
        let mut total = (sounding & !spelling).count_ones() as i32 * tuning::fit::UNEXPLAINED_KEY;
        let absent = spelling & !sounding;
        if absent == 0 {
            return total;
        }
        for tone in tones {
            if absent & (1 << tone.note.pitch_class().value()) != 0 {
                total += Self::cost_of_absent(tone);
            }
        }
        total
    }

    /// Which tier this spelling lands in against these keys, and how far away it is.
    pub fn measure(chord: &SpelledChord, sounding: &BTreeSet<PitchClass>) -> (ChordFit, i32) {
        let spelling_mask = QualityCatalog::mask(chord.pitch_classes.iter().copied());
        let sounding_mask = QualityCatalog::mask(sounding.iter().copied());
        let cost = Self::cost(&chord.tones, spelling_mask, sounding_mask);

        let fit = match Self::tier(spelling_mask, sounding_mask) {
            0 => ChordFit::Exact,
            1 => {
                let mut degrees: Vec<u8> = chord
                    .tones
                    .iter()
                    .filter(|t| !sounding.contains(&t.note.pitch_class()))
                    .map(|t| t.degree.number())
                    .collect();
                degrees.sort_unstable();
                ChordFit::Missing(degrees)
            }
            2 => ChordFit::Plus(sounding.difference(&chord.pitch_classes).copied().collect()),
            _ => ChordFit::Nearest { distance: cost },
        };
        (fit, cost)
    }

    /// What leaving this one tone out costs, from `tuning::fit`.
    pub fn cost_of_absent(tone: &SpelledTone) -> i32 {
        if tone.omissible() {
            return tuning::fit::SANCTIONED_OMISSION;
        }
        match tone.degree.number() {
            1 | 3 | 7 => tuning::fit::MISSING_IDENTITY_TONE,
            // Read "perfect" as the arithmetic, not as the triad: a fifth that has
            // been moved is the point of the spelling that moved it.
            5 => {
                if tone.degree.semitones % 12 == 7 {
                    tuning::fit::MISSING_PERFECT_FIFTH
                } else {
                    tuning::fit::MISSING_IDENTITY_TONE
                }
            }
            _ => tuning::fit::MISSING_ADDED_TONE,
        }
    }
}

/// Orders the honest readings of one chord.
///
/// Every weight lives in `tuning::weight`, none of them are Michael's, and the
/// whole point of gathering them there is that disagreeing with a headline is a
/// one-line edit rather than a rewrite. Nothing in this file hard-codes a number.
pub struct ChordRanking;

impl ChordRanking {
    /// Sums the independent terms.
    ///
    /// `ROOT_IS_PRESENT` and `ALL_TONES_ACCOUNTED_FOR` are constants under the
    /// exact path — awarded to every reading of a given chord — and are still
    /// written out because they say what the ranking *means*.
    pub fn score(
        entry: &CatalogEntry,
        bass: Option<PitchClass>,
        sounding: &BTreeSet<PitchClass>,
    ) -> i32 {
        let mut total = 0;
        let root = entry.root().pitch_class();

        if Some(root) == bass {
            total += tuning::weight::ROOT_IS_BASS;
        }
        if sounding.contains(&root) {
            total += tuning::weight::ROOT_IS_PRESENT;
        }
        if entry.pitch_classes == *sounding {
            total += tuning::weight::ALL_TONES_ACCOUNTED_FOR;
        }

        total += tuning::quality_commonness(entry.commonness_key).unwrap_or(0);

        // Charged per extension **and per note the spelling names that is not
        // being played**.
        total += tuning::weight::PER_ALTERATION
            * (entry.extension_count() + entry.omitted_tone_count()) as i32;

        if entry.root().alteration == 0 {
            total += tuning::weight::NATURAL_ROOT;
        }
        if !entry.chord.uses_double_accidental() {
            total += tuning::weight::SIMPLER_ACCIDENTAL;
        }

        total
    }

    /// Best first. Fit cost ahead of every score term; then score; then fewest
    /// notes left out; then fewest extensions; then the root spelling; then the
    /// display, which is unique across the catalog, so the order is total.
    pub fn rank(mut scored: Vec<ScoredReading>) -> Vec<ScoredReading> {
        scored.sort_by(|lhs, rhs| {
            lhs.fit_cost
                .cmp(&rhs.fit_cost)
                .then_with(|| rhs.score.cmp(&lhs.score))
                .then_with(|| {
                    lhs.entry
                        .omitted_tone_count()
                        .cmp(&rhs.entry.omitted_tone_count())
                })
                .then_with(|| {
                    lhs.entry
                        .extension_count()
                        .cmp(&rhs.entry.extension_count())
                })
                .then_with(|| lhs.entry.root().display().cmp(&rhs.entry.root().display()))
                .then_with(|| lhs.entry.display().cmp(&rhs.entry.display()))
        });
        scored
    }
}
