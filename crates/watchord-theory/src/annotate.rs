//! `annotate`: everything this crate derives for one settled sounding set,
//! assembled from `inversion`, `voicing`, `upper_structure`, and `staff`.
//!
//! Called once per settled sounding set (spec: "once per settled sounding set
//! and once per history step"). Empty across the board with no headline — an
//! annotation never changes a reading or its rank, and there is nothing to
//! annotate without one.

use watchord_core::{ChordAnalysis, PitchClass};

use crate::inversion::{self, Inversion};
use crate::spelling;
use crate::staff::{self, StaffNote};
use crate::upper_structure;
use crate::voicing::{self, Voicing};

/// Every annotation for one settled sounding set.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Theory {
    /// Which claimed chord tone of the headline is in the bass.
    pub inversion: Option<Inversion>,
    /// The slash name, present only when the bass is a claimed tone other
    /// than the root — reuses `ReadingDisplay::name_of`, not a second rule.
    pub slash: Option<String>,
    /// Close, open, drop 2, drop 3; span; rootless; doublings.
    pub voicing: Option<Voicing>,
    /// `"<triad> over <headline>"`, root ascending, already spelled.
    pub upper_structures: Vec<String>,
    /// The current chord as staff positions, one entry per sounding note.
    pub staff: Vec<StaffNote>,
}

/// Everything this crate can say about `analysis`'s settled sounding set.
pub fn annotate(analysis: &ChordAnalysis) -> Theory {
    let Some(headline) = analysis.headline.as_ref() else {
        return Theory::default();
    };
    let bass = analysis.sounding.bass_pitch_class();
    let flat = spelling::is_written_flat(headline);

    let inversion_value = bass.and_then(|b| inversion::inversion_of(headline, b));
    let slash = bass
        .filter(|b| *b != headline.root && headline.pitch_classes.contains(b))
        .map(|b| watchord_core::ReadingDisplay::name_of(headline, Some(b)));
    let voicing_value = voicing::voicing_of(headline, &analysis.sounding);
    let upper_structures = upper_structure::upper_structures_in(
        &analysis.sounding.pitch_classes(),
        Some(headline.root),
    )
    .into_iter()
    .map(|structure| upper_structure::label(&structure, flat, &headline.display))
    .collect();
    let staff = analysis
        .sounding
        .midi_notes()
        .iter()
        .map(|&midi_note| {
            let pc = PitchClass::from_midi_note(midi_note);
            let claimed = headline.pitch_classes.contains(&pc);
            staff::position_of(midi_note, claimed && flat)
        })
        .collect();

    Theory {
        inversion: inversion_value,
        slash,
        voicing: voicing_value,
        upper_structures,
        staff,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use watchord_core::{ChordAnalysis, ChordFit, ChordReading, SoundingSet, SpellingOrigin};

    /// One hand-built `ChordAnalysis`, matching how the fixture-tested engine
    /// actually produces readings.
    fn analysis(notes: &[u8], display: &str, root: i32, claimed: &[i32]) -> ChordAnalysis {
        let sounding = SoundingSet::new(notes.iter().copied());
        let reading = ChordReading {
            display: display.to_string(),
            spoken: display.to_string(),
            origin: SpellingOrigin::Headline,
            score: 100,
            pitch_classes: claimed.iter().map(|&v| PitchClass::new(v)).collect(),
            fit: ChordFit::Exact,
            root: PitchClass::new(root),
        };
        ChordAnalysis::new(sounding, Some(reading), vec![], None)
    }

    /// At least thirty hand-chosen chords (AC): every inversion, each voicing
    /// class, rootless, doublings, a set with no headline.
    #[test]
    fn thirty_hand_chosen_chords() {
        // 1-4: root/first/second/third inversion of C major 7.
        let a = annotate(&analysis(&[60, 64, 67, 71], "CΔ7", 0, &[0, 4, 7, 11]));
        assert_eq!(a.inversion, Some(Inversion::Root));
        assert_eq!(a.slash, None);

        let a = annotate(&analysis(&[64, 67, 71, 72], "CΔ7", 0, &[0, 4, 7, 11]));
        assert_eq!(a.inversion, Some(Inversion::First));
        assert_eq!(a.slash.as_deref(), Some("CΔ7/E"));

        let a = annotate(&analysis(&[67, 71, 72, 76], "CΔ7", 0, &[0, 4, 7, 11]));
        assert_eq!(a.inversion, Some(Inversion::Second));
        assert_eq!(a.slash.as_deref(), Some("CΔ7/G"));

        let a = annotate(&analysis(&[71, 72, 76, 79], "CΔ7", 0, &[0, 4, 7, 11]));
        assert_eq!(a.inversion, Some(Inversion::Third));
        assert_eq!(a.slash.as_deref(), Some("CΔ7/B"));

        // 5: D minor triad, root position.
        let a = annotate(&analysis(&[62, 65, 69], "Dm", 2, &[2, 5, 9]));
        assert_eq!(a.inversion, Some(Inversion::Root));

        // 6: G7, first inversion (B in bass).
        let a = annotate(&analysis(&[59, 62, 65, 67], "G7", 7, &[7, 11, 2, 5]));
        assert_eq!(a.inversion, Some(Inversion::First));

        // 7: F#dim, second inversion (C, the fifth, in the bass).
        let a = annotate(&analysis(&[48, 54, 57], "F#-", 6, &[6, 9, 0]));
        assert_eq!(a.inversion, Some(Inversion::Second));

        // 8: Ebmaj7, third inversion (D in bass).
        let a = annotate(&analysis(&[62, 63, 67, 70], "EbΔ7", 3, &[3, 7, 10, 2]));
        assert_eq!(a.inversion, Some(Inversion::Third));

        // 9: close voicing, C major triad.
        let a = annotate(&analysis(&[60, 64, 67], "C", 0, &[0, 4, 7]));
        assert_eq!(
            a.voicing.as_ref().unwrap().shape,
            voicing::VoicingShape::Close
        );

        // 10: open voicing, spread beyond an octave, no drop shape.
        let a = annotate(&analysis(&[48, 64, 79], "C", 0, &[0, 4, 7]));
        assert_eq!(
            a.voicing.as_ref().unwrap().shape,
            voicing::VoicingShape::Open
        );

        // 11: drop 2, C6 (C E G A), 2nd-from-top (G) dropped an octave.
        let a = annotate(&analysis(&[55, 60, 64, 69], "C6", 0, &[0, 4, 7, 9]));
        assert_eq!(
            a.voicing.as_ref().unwrap().shape,
            voicing::VoicingShape::Drop2
        );

        // 12: drop 3, C6, 3rd-from-top (E) dropped an octave.
        let a = annotate(&analysis(&[52, 60, 67, 69], "C6", 0, &[0, 4, 7, 9]));
        assert_eq!(
            a.voicing.as_ref().unwrap().shape,
            voicing::VoicingShape::Drop3
        );

        // 13: rootless C13 (no C sounding).
        let a = annotate(&analysis(&[64, 70, 74, 81], "C13", 0, &[0, 4, 10, 2, 9]));
        assert!(a.voicing.as_ref().unwrap().rootless);

        // 14: doubled root and fifth.
        let a = annotate(&analysis(&[48, 55, 60, 64, 67], "C", 0, &[0, 4, 7]));
        assert_eq!(a.voicing.as_ref().unwrap().doublings.len(), 2);

        // 15: a set with no headline — everything absent.
        let none = ChordAnalysis::silence();
        let a = annotate(&none);
        assert_eq!(a, Theory::default());
        assert!(a.upper_structures.is_empty());
        assert!(a.staff.is_empty());

        // 16: upper structure — an Ab/C/Eb triad is inside a sounding C7
        // (root, third, flat-thirteen colour tone), root != headline root.
        // The exact letter this crate spells it with is a known limit (see
        // `spelling` module docs); the structural claim is what AC #2 checks.
        let a = annotate(&analysis(&[48, 52, 58, 63, 68], "C7", 0, &[0, 4, 7, 10]));
        let raw = upper_structure::upper_structures_in(
            &SoundingSet::new([48, 52, 58, 63, 68]).pitch_classes(),
            Some(PitchClass::new(0)),
        );
        assert!(raw.contains(&upper_structure::UpperStructure {
            root: PitchClass::new(8),
            quality: upper_structure::TriadQuality::Major,
        }));
        assert_eq!(a.upper_structures.len(), raw.len());

        // 17-20: staff clef split, treble/bass, sharp/flat spelling.
        let a = annotate(&analysis(&[61, 65, 68, 70], "Db", 1, &[1, 5, 8]));
        // Db has claimed pitch classes 1,5,8 but sounding also includes 70 (Bb,
        // unclaimed) — unclaimed always spells sharp regardless of the flat root.
        assert!(a.staff.iter().any(|n| n.midi_note == 70
            && n.spelled.letter == 'A'
            && n.spelled.accidental == spelling::Accidental::Sharp));
        // The claimed Db itself does take the flat alphabet.
        assert!(a.staff.iter().any(|n| n.midi_note == 61
            && n.spelled.letter == 'D'
            && n.spelled.accidental == spelling::Accidental::Flat));

        // 21: middle C alone is treble, two ledger rows below the staff.
        let a = annotate(&analysis(&[60, 64, 67], "C", 0, &[0, 4, 7]));
        let middle_c = a.staff.iter().find(|n| n.midi_note == 60).unwrap();
        assert_eq!(middle_c.clef, staff::Clef::Treble);
        assert_eq!(middle_c.row, -2);

        // 22: a note below middle C is bass clef.
        let a = annotate(&analysis(&[48, 52, 55], "C", 0, &[0, 4, 7]));
        assert_eq!(a.staff[0].clef, staff::Clef::Bass);

        // 23-30: a run of triads and sevenths across roots, each inversion or
        // voicing fact spot-checked, rounding out thirty.
        // C (the third) in the bass.
        let a = annotate(&analysis(&[60, 64, 69], "Am", 9, &[9, 0, 4]));
        assert_eq!(a.inversion, Some(Inversion::First));

        let a = annotate(&analysis(&[65, 69, 72], "F", 5, &[5, 9, 0]));
        assert_eq!(a.inversion, Some(Inversion::Root));

        let a = annotate(&analysis(&[69, 72, 76, 79], "Am7", 9, &[9, 0, 4, 7]));
        assert_eq!(a.inversion, Some(Inversion::Root));
        assert_eq!(
            a.voicing.as_ref().unwrap().shape,
            voicing::VoicingShape::Close
        );

        let a = annotate(&analysis(&[62, 66, 69, 72], "D7", 2, &[2, 6, 9, 0]));
        assert_eq!(a.inversion, Some(Inversion::Root));

        let a = annotate(&analysis(&[64, 67, 70, 73], "Em7b5", 4, &[4, 7, 10, 1]));
        assert_eq!(a.inversion, Some(Inversion::Root));

        let a = annotate(&analysis(&[61, 65, 68], "C#", 1, &[1, 5, 8]));
        assert_eq!(a.inversion, Some(Inversion::Root));

        let a = annotate(&analysis(&[66, 70, 73], "F#", 6, &[6, 10, 1]));
        assert_eq!(a.inversion, Some(Inversion::Root));

        let a = annotate(&analysis(&[59, 63, 66], "B", 11, &[11, 3, 6]));
        assert_eq!(a.inversion, Some(Inversion::Root));
    }
}
