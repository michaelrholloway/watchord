//! Upper structure: every major or minor triad inside the sounding set whose
//! root differs from the headline's, root ascending, not ranked.

use std::collections::BTreeSet;

use watchord_core::PitchClass;

use crate::spelling;

/// A major or a minor triad. No augmented or diminished — the spec names only
/// these two.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TriadQuality {
    Major,
    Minor,
}

/// One upper-structure triad: its root and quality. Pure identity — no display
/// text, so the 4,095-set sweep can assert against it directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpperStructure {
    pub root: PitchClass,
    pub quality: TriadQuality,
}

impl UpperStructure {
    /// The triad's own pitch classes: `{root, root+3or4, root+7}`.
    pub fn pitch_classes(&self) -> [PitchClass; 3] {
        let third = match self.quality {
            TriadQuality::Major => 4,
            TriadQuality::Minor => 3,
        };
        [
            self.root,
            self.root.transposed(third),
            self.root.transposed(7),
        ]
    }
}

/// Every major or minor triad inside `set`, with a root other than
/// `exclude_root` (the headline's root, typically), root ascending, major
/// before minor at the same root. Pure: no `ChordReading` needed, which is what
/// lets the 4,095-pitch-class-set sweep exercise it directly.
pub fn upper_structures_in(
    set: &BTreeSet<PitchClass>,
    exclude_root: Option<PitchClass>,
) -> Vec<UpperStructure> {
    let mut found = Vec::new();
    for root in PitchClass::all_cases() {
        if Some(root) == exclude_root {
            continue;
        }
        for quality in [TriadQuality::Major, TriadQuality::Minor] {
            let candidate = UpperStructure { root, quality };
            if candidate.pitch_classes().iter().all(|pc| set.contains(pc)) {
                found.push(candidate);
            }
        }
    }
    found
}

/// `<triad> over <headline>` — `"Ab triad over C7"` — the line the spec puts
/// beside the headline. `flat` selects the alphabet (mirrors the headline's own
/// spelling, see [`crate::spelling`]); `headline_name` is the headline's own
/// display text (`ChordReading::display`, not the slash form).
pub fn label(structure: &UpperStructure, flat: bool, headline_name: &str) -> String {
    let spelled = spelling::spell(structure.root, flat);
    let suffix = match structure.quality {
        TriadQuality::Major => "",
        TriadQuality::Minor => "m",
    };
    format!(
        "{}{}{} triad over {}",
        spelled.letter,
        spelled.accidental.symbol(),
        suffix,
        headline_name
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(pcs: &[i32]) -> BTreeSet<PitchClass> {
        pcs.iter().map(|&v| PitchClass::new(v)).collect()
    }

    #[test]
    fn finds_a_major_triad_a_third_above() {
        // Ab triad (8,0,3) over a C7 (0,4,7,10).
        let s = set(&[0, 4, 7, 10, 8, 3]);
        let found = upper_structures_in(&s, Some(PitchClass::new(0)));
        assert!(found.contains(&UpperStructure {
            root: PitchClass::new(8),
            quality: TriadQuality::Major
        }));
    }

    #[test]
    fn excludes_the_headline_root() {
        let s = set(&[0, 4, 7]);
        let found = upper_structures_in(&s, Some(PitchClass::new(0)));
        assert!(!found.iter().any(|u| u.root == PitchClass::new(0)));
    }

    #[test]
    fn root_ascending_major_before_minor() {
        // C major (0,4,7) and C minor cannot coexist (needs both E and Eb), so
        // use two different roots: D major and F minor both inside one set.
        let s = set(&[2, 6, 9, 5, 8, 0]); // D F# A, F Ab C
        let found = upper_structures_in(&s, None);
        let roots: Vec<i32> = found.iter().map(|u| u.root.value() as i32).collect();
        let mut sorted = roots.clone();
        sorted.sort_unstable();
        assert_eq!(roots, sorted);
    }

    #[test]
    fn sweep_over_all_4095_sets_never_finds_a_triad_outside_the_set() {
        for mask in 1u16..4096 {
            let set: BTreeSet<PitchClass> = (0..12)
                .filter(|i| mask & (1 << i) != 0)
                .map(PitchClass::new)
                .collect();
            for structure in upper_structures_in(&set, None) {
                for pc in structure.pitch_classes() {
                    assert!(
                        set.contains(&pc),
                        "mask {mask:012b}: {structure:?} claims {pc:?} outside the set"
                    );
                }
            }
        }
    }

    #[test]
    fn control_a_fabricated_triad_outside_the_set_is_rejected() {
        // {C, E, G} does not contain a G triad (G, B, D) — B and D are absent.
        let s = set(&[0, 4, 7]);
        let found = upper_structures_in(&s, None);
        assert!(!found.contains(&UpperStructure {
            root: PitchClass::new(7),
            quality: TriadQuality::Major
        }));
    }
}
