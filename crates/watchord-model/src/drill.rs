//! Drill: a mode on [`crate::AppModel`], entered with one key, that names a
//! target chord from the engine's vocabulary and grades what is played
//! against it. Spec #9 ("Drill"), ticket #14.
//!
//! Everything here is pure and seam-driven: [`watchord_core::ChordVocabulary`]
//! supplies the targets and [`watchord_core::DrillStoring`] supplies the
//! stats, so this module — like the rest of the model — never reaches into
//! `watchord-engine` or a concrete store.

use std::collections::BTreeSet;

use watchord_core::{DrillChordStat, DrillTarget, NoteName, PitchClass};

// MARK: - Grading

/// How a played sounding set fits a drill target's claimed pitch classes.
///
/// Mirrors `watchord_engine::chord_ranking::ChordFitting::tier`'s four-way
/// subset comparison between what is claimed and what is sounding — the
/// same rule, the same four tiers, the same names. It is a **separate**
/// type from `watchord_core::ChordFit` rather than a reuse of it: `ChordFit`'s
/// `Missing` carries scale-degree numbers (`no5`, `no11`), which come from a
/// `SpelledChord`'s tones — a spelling the model has no way to reach without
/// depending on the engine. `DrillFit` names what was missed and what was
/// extra by pitch class instead, which the tier comparison already has in
/// hand.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DrillFit {
    /// Played exactly the target's pitch classes, no more, no less.
    Exact,
    /// Played a subset of the target: nothing extra, but some claimed pitch
    /// classes never sounded.
    Missing(BTreeSet<PitchClass>),
    /// Played a superset of the target: every claimed pitch class sounded,
    /// plus some that were not claimed.
    Plus(BTreeSet<PitchClass>),
    /// Neither: some claimed pitch classes were missed and some unclaimed
    /// ones sounded.
    Nearest {
        missing: BTreeSet<PitchClass>,
        extra: BTreeSet<PitchClass>,
    },
}

impl DrillFit {
    /// The tier's name, matching `ChordFit::tier_name`: `exact`, `missing`,
    /// `plus`, `nearest`.
    pub fn tier_name(&self) -> &'static str {
        match self {
            DrillFit::Exact => "exact",
            DrillFit::Missing(_) => "missing",
            DrillFit::Plus(_) => "plus",
            DrillFit::Nearest { .. } => "nearest",
        }
    }

    /// True only for `Exact` — what counts toward a chord's `exact` stat.
    pub fn is_exact(&self) -> bool {
        matches!(self, DrillFit::Exact)
    }

    /// Grades `played` against `claimed`, by the same subset comparison
    /// `ChordFitting::tier` uses on the engine's masks, worked here directly
    /// on pitch-class sets.
    pub fn measure(claimed: &BTreeSet<PitchClass>, played: &BTreeSet<PitchClass>) -> Self {
        let missing: BTreeSet<PitchClass> = claimed.difference(played).copied().collect();
        let extra: BTreeSet<PitchClass> = played.difference(claimed).copied().collect();
        match (missing.is_empty(), extra.is_empty()) {
            (true, true) => DrillFit::Exact,
            (false, true) => DrillFit::Missing(missing),
            (true, false) => DrillFit::Plus(extra),
            (false, false) => DrillFit::Nearest { missing, extra },
        }
    }

    /// What was missed and what was extra, named — empty for `Exact`.
    pub fn note(&self) -> String {
        let names = |pcs: &BTreeSet<PitchClass>| -> String {
            pcs.iter()
                .map(|&pc| NoteName::pitch_class(pc, false))
                .collect::<Vec<_>>()
                .join(" ")
        };
        match self {
            DrillFit::Exact => String::new(),
            DrillFit::Missing(pcs) => format!("missing {}", names(pcs)),
            DrillFit::Plus(pcs) => format!("extra {}", names(pcs)),
            DrillFit::Nearest { missing, extra } => {
                format!("missing {} · extra {}", names(missing), names(extra))
            }
        }
    }
}

// MARK: - Weighting

/// A target's weight for the next draw: `1 + attempts - exact`, never below
/// 1 — a chord with no history at all is weight 1, same as a chord that has
/// been nailed every time; a chord missed more than it has been nailed
/// climbs above that floor. `None` (never attempted) is the same as an
/// all-zero row.
pub fn weight_of(stat: Option<&DrillChordStat>) -> u32 {
    let (attempts, exact) = stat.map_or((0, 0), |s| (s.attempts, s.exact));
    let weight = 1i64 + attempts as i64 - exact as i64;
    weight.max(1) as u32
}

/// Draws one target from `targets`, proportional to `weights` (same index,
/// same length). `None` when either is empty or every weight is zero.
pub fn draw_weighted<'a>(
    targets: &'a [DrillTarget],
    weights: &[u32],
    rng: &mut Xorshift64,
) -> Option<&'a DrillTarget> {
    if targets.len() != weights.len() || targets.is_empty() {
        return None;
    }
    let total: u64 = weights.iter().map(|&w| w as u64).sum();
    if total == 0 {
        return None;
    }
    let mut roll = rng.next_u64() % total;
    for (target, &weight) in targets.iter().zip(weights) {
        if roll < weight as u64 {
            return Some(target);
        }
        roll -= weight as u64;
    }
    targets.last()
}

// MARK: - The seedable RNG

/// A tiny xorshift64 generator — deterministic from a seed, so a test can
/// state exactly what a "random" draw does. Not cryptographic; drill target
/// selection has no adversary.
#[derive(Clone, Debug)]
pub struct Xorshift64(u64);

impl Xorshift64 {
    /// A generator seeded with `seed`. `0` is replaced with a fixed non-zero
    /// value — xorshift's state must never be zero, or every draw is zero.
    pub fn new(seed: u64) -> Self {
        Xorshift64(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }

    /// The next value in the sequence, advancing the state.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

impl Default for Xorshift64 {
    /// A fixed default seed, not the wall clock — so a model built with
    /// `AppModel::new` and never explicitly seeded still draws
    /// deterministically, which is what lets `--fake` scripts be exact.
    fn default() -> Self {
        Xorshift64::new(0xD1CE_D1CE_D1CE_D1CE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use watchord_core::ChordKey;

    fn pcs(values: &[i32]) -> BTreeSet<PitchClass> {
        values.iter().map(|&v| PitchClass::new(v)).collect()
    }

    #[test]
    fn measure_is_exact_when_the_sets_are_equal() {
        let claimed = pcs(&[0, 4, 7]);
        assert_eq!(DrillFit::measure(&claimed, &claimed), DrillFit::Exact);
    }

    #[test]
    fn measure_is_missing_when_a_claimed_tone_never_sounded() {
        let claimed = pcs(&[0, 4, 7, 10]);
        let played = pcs(&[0, 4, 10]);
        let fit = DrillFit::measure(&claimed, &played);
        assert_eq!(fit, DrillFit::Missing(pcs(&[7])));
        assert_eq!(fit.tier_name(), "missing");
        assert_eq!(fit.note(), "missing G");
    }

    #[test]
    fn measure_is_plus_when_every_claimed_tone_sounded_and_more() {
        let claimed = pcs(&[0, 4, 7]);
        let played = pcs(&[0, 4, 7, 11]);
        let fit = DrillFit::measure(&claimed, &played);
        assert_eq!(fit, DrillFit::Plus(pcs(&[11])));
        assert_eq!(fit.note(), "extra B");
    }

    #[test]
    fn measure_is_nearest_when_both_sides_disagree() {
        let claimed = pcs(&[0, 4, 7]);
        let played = pcs(&[0, 3, 9]);
        let fit = DrillFit::measure(&claimed, &played);
        assert!(matches!(fit, DrillFit::Nearest { .. }));
        assert_eq!(fit.tier_name(), "nearest");
        assert!(!fit.is_exact());
    }

    #[test]
    fn weight_is_one_for_a_chord_with_no_history() {
        assert_eq!(weight_of(None), 1);
    }

    #[test]
    fn weight_is_one_plus_attempts_minus_exact_never_below_one() {
        let key = ChordKey::new([0, 4, 7].map(PitchClass::new));
        let perfect = DrillChordStat {
            chord_key: key.clone(),
            attempts: 5,
            exact: 5,
            last_at: None,
        };
        assert_eq!(weight_of(Some(&perfect)), 1);

        let weak = DrillChordStat {
            chord_key: key,
            attempts: 5,
            exact: 1,
            last_at: None,
        };
        assert_eq!(weight_of(Some(&weak)), 5);
    }

    #[test]
    fn draw_weighted_is_deterministic_from_its_seed() {
        let targets = vec![
            DrillTarget {
                key: ChordKey::new([0, 4, 7].map(PitchClass::new)),
                display: "C".to_string(),
            },
            DrillTarget {
                key: ChordKey::new([2, 5, 9].map(PitchClass::new)),
                display: "Dm".to_string(),
            },
        ];
        let weights = [1u32, 1u32];
        let mut rng_a = Xorshift64::new(42);
        let mut rng_b = Xorshift64::new(42);
        let a: Vec<&str> = (0..10)
            .map(|_| {
                draw_weighted(&targets, &weights, &mut rng_a)
                    .unwrap()
                    .display
                    .as_str()
            })
            .collect();
        let b: Vec<&str> = (0..10)
            .map(|_| {
                draw_weighted(&targets, &weights, &mut rng_b)
                    .unwrap()
                    .display
                    .as_str()
            })
            .collect();
        assert_eq!(a, b, "the same seed draws the same sequence");
    }

    /// Ticket #14's acceptance criterion: over a seeded run, chords with more
    /// misses draw more often than chords with none.
    #[test]
    fn weak_chords_draw_more_often_over_a_seeded_run() {
        let key_a = ChordKey::new([0, 4, 7].map(PitchClass::new)); // never missed
        let key_b = ChordKey::new([2, 5, 9].map(PitchClass::new)); // missed a lot
        let targets = vec![
            DrillTarget {
                key: key_a.clone(),
                display: "C".to_string(),
            },
            DrillTarget {
                key: key_b.clone(),
                display: "Dm".to_string(),
            },
        ];
        let stats = [
            DrillChordStat {
                chord_key: key_a.clone(),
                attempts: 10,
                exact: 10,
                last_at: None,
            },
            DrillChordStat {
                chord_key: key_b.clone(),
                attempts: 10,
                exact: 0,
                last_at: None,
            },
        ];
        let weights: Vec<u32> = targets
            .iter()
            .map(|t| weight_of(stats.iter().find(|s| s.chord_key == t.key)))
            .collect();
        assert_eq!(weights, [1, 11]);

        let mut rng = Xorshift64::new(7);
        let mut counts = [0u32; 2];
        for _ in 0..200 {
            let drawn = draw_weighted(&targets, &weights, &mut rng).unwrap();
            if drawn.key == key_a {
                counts[0] += 1;
            } else {
                counts[1] += 1;
            }
        }
        assert!(
            counts[1] > counts[0],
            "the weak chord ({}) should draw more than the strong one ({}) over 200 draws",
            counts[1],
            counts[0]
        );
    }
}
