//! Inversion: which chord tone of the headline is in the bass.

use watchord_core::{ChordReading, PitchClass};

/// Root, first, second, or third inversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Inversion {
    Root,
    First,
    Second,
    Third,
}

impl Inversion {
    /// `root`, `first`, `second`, `third` — the plain word for the screen.
    pub fn label(self) -> &'static str {
        match self {
            Inversion::Root => "root",
            Inversion::First => "first",
            Inversion::Second => "second",
            Inversion::Third => "third",
        }
    }
}

/// Which claimed chord tone of `reading` `bass` is, ranking the claimed pitch
/// classes by ascending semitone interval from the root (root itself is always
/// interval 0, hence always [`Inversion::Root`]). `None` when `bass` is not
/// among the claimed tones — an unclaimed bass has no inversion to report, only
/// a `Plus`/`Nearest` fit.
///
/// Known limit: ranking by raw semitone interval mod 12 folds a ninth (2
/// semitones) below a third (4 semitones), so a chord whose bass is a claimed
/// upper extension (a 9th/11th/13th in the bass) is not reliably named beyond
/// [`Inversion::Third`] — real practice does not name a "fourth inversion"
/// either, so this only matters for the small overlap where an extension's mod-12
/// interval is smaller than the third's or fifth's. Triads and plain sevenths
/// (what the spec's four-name vocabulary actually describes) are unaffected.
pub fn inversion_of(reading: &ChordReading, bass: PitchClass) -> Option<Inversion> {
    if !reading.pitch_classes.contains(&bass) {
        return None;
    }
    let mut ordered: Vec<PitchClass> = reading.pitch_classes.iter().copied().collect();
    ordered.sort_by_key(|&pc| reading.root.interval_to(pc));
    let index = ordered.iter().position(|&pc| pc == bass)?;
    Some(match index {
        0 => Inversion::Root,
        1 => Inversion::First,
        2 => Inversion::Second,
        _ => Inversion::Third,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use watchord_core::{ChordFit, SpellingOrigin};

    fn reading(root: u8, claimed: &[u8]) -> ChordReading {
        ChordReading {
            display: "X".to_string(),
            spoken: "x".to_string(),
            origin: SpellingOrigin::Headline,
            score: 0,
            pitch_classes: claimed.iter().map(|&v| PitchClass::new(v as i32)).collect(),
            fit: ChordFit::Exact,
            root: PitchClass::new(root as i32),
        }
    }

    #[test]
    fn root_position_is_root() {
        let r = reading(0, &[0, 4, 7]);
        assert_eq!(inversion_of(&r, PitchClass::new(0)), Some(Inversion::Root));
    }

    #[test]
    fn third_in_bass_is_first_inversion() {
        let r = reading(0, &[0, 4, 7]);
        assert_eq!(inversion_of(&r, PitchClass::new(4)), Some(Inversion::First));
    }

    #[test]
    fn fifth_in_bass_is_second_inversion() {
        let r = reading(0, &[0, 4, 7]);
        assert_eq!(
            inversion_of(&r, PitchClass::new(7)),
            Some(Inversion::Second)
        );
    }

    #[test]
    fn seventh_in_bass_is_third_inversion() {
        let r = reading(0, &[0, 4, 7, 10]);
        assert_eq!(
            inversion_of(&r, PitchClass::new(10)),
            Some(Inversion::Third)
        );
    }

    #[test]
    fn unclaimed_bass_has_no_inversion() {
        let r = reading(0, &[0, 4, 7]);
        assert_eq!(inversion_of(&r, PitchClass::new(2)), None);
    }
}
