//! Ported case for case from note-view `Tests/ChordEngineTests/GrammarTests.swift`.
//! The notation grammar, exercised forward: a spelling in, the notes it implies out.

mod common;

use std::collections::BTreeSet;

use common::{analyze, c, pcs, worked_table};
use watchord_core::{ChordFit, PitchClass};
use watchord_engine::quality_catalog::QualityCatalog;
use watchord_engine::{
    ChordExtension, ChordSpelling, Effect, NoteLetter, NoteSpelling, SeventhQuality, TriadQuality,
};

fn tone_displays(spelling: &ChordSpelling) -> Vec<String> {
    spelling
        .spelled()
        .unwrap_or_else(|| panic!("{} must be writable under the grammar", spelling.display()))
        .tones
        .iter()
        .map(|t| t.note.display())
        .collect()
}

#[test]
fn the_worked_table_writes_sounds_and_speaks_as_michael_wrote_it() {
    for row in worked_table() {
        assert_eq!(row.spelling.display(), row.display);
        assert_eq!(row.spelling.spoken(), row.spoken);
        assert_eq!(tone_displays(&row.spelling), row.tones, "{}", row.display);
    }
}

/// Cross-check of the letter arithmetic against plain semitone arithmetic.
#[test]
fn letter_derived_pitch_classes_equal_semitone_derived_ones() {
    for row in worked_table() {
        let spelled = row.spelling.spelled().expect("writable");
        let root = row.spelling.root.pitch_class();
        let mut expected: BTreeSet<PitchClass> = [root].into_iter().collect();
        expected.insert(root.transposed(row.spelling.triad.third_semitones()));

        let mut fifth = row.spelling.triad.fifth_semitones();
        for ext in &row.spelling.extensions {
            if let Effect::AltersFifth(by) = ext.effect() {
                fifth += by;
            }
        }
        expected.insert(root.transposed(fifth));
        if let Some(seventh) = row.spelling.seventh.semitones() {
            expected.insert(root.transposed(seventh));
        }
        for ext in &row.spelling.extensions {
            match ext.effect() {
                Effect::AddsDegrees(added) => {
                    for degree in added {
                        expected.insert(root.transposed(degree.semitones));
                    }
                }
                Effect::ReplacesThird(degree) => {
                    expected.remove(&root.transposed(row.spelling.triad.third_semitones()));
                    expected.insert(root.transposed(degree.semitones));
                }
                Effect::OmitsThird => {
                    expected.remove(&root.transposed(row.spelling.triad.third_semitones()));
                }
                Effect::AltersFifth(_) => {}
            }
        }
        assert_eq!(spelled.pitch_classes, expected, "{}", row.display);
    }
}

#[test]
fn the_root_vocabulary_is_the_naturals_plus_the_real_enharmonic_pairs() {
    let roots: Vec<String> = NoteSpelling::root_spellings()
        .iter()
        .map(|r| r.display())
        .collect();
    assert_eq!(
        roots,
        [
            "A", "A#", "Ab", "B", "Bb", "C", "C#", "D", "D#", "Db", "E", "Eb", "F", "F#", "G",
            "G#", "Gb"
        ]
    );
    assert!(
        NoteSpelling::root_spellings()
            .iter()
            .all(|r| r.alteration.abs() <= 1)
    );
    for (sharp, flat) in [
        ("C#", "Db"),
        ("D#", "Eb"),
        ("F#", "Gb"),
        ("G#", "Ab"),
        ("A#", "Bb"),
    ] {
        assert!(roots.iter().any(|r| r == sharp));
        assert!(roots.iter().any(|r| r == flat));
    }
    for renamed in ["B#", "Cb", "E#", "Fb"] {
        assert!(!roots.iter().any(|r| r == renamed));
    }
}

#[test]
fn a_tone_needing_a_triple_accidental_is_not_writable() {
    assert_eq!(NoteSpelling::new(NoteLetter::C, 3), None);
    assert_eq!(NoteSpelling::new(NoteLetter::C, -3), None);
    assert_eq!(
        NoteSpelling::new(NoteLetter::C, 2).map(|n| n.display()),
        Some("C##".into())
    );
    assert_eq!(
        NoteSpelling::new(NoteLetter::C, -2).map(|n| n.display()),
        Some("Cbb".into())
    );
}

#[test]
fn an_augmented_seventh_collapses_onto_the_root_and_is_refused() {
    assert_eq!(SeventhQuality::Augmented.semitones(), Some(12));
    let spelling = ChordSpelling::new(c(), TriadQuality::Major, SeventhQuality::Augmented, &[]);
    assert_eq!(spelling.display(), "Caug7");
    assert_eq!(spelling.spelled(), None);
}

#[test]
fn the_stack_implies_the_seventh_and_writes_only_its_own_number() {
    let cases: [(SeventhQuality, ChordExtension, &str, &[&str]); 4] = [
        (
            SeventhQuality::Minor,
            ChordExtension::Nine,
            "C9",
            &["C", "E", "G", "Bb", "D"],
        ),
        (
            SeventhQuality::Major,
            ChordExtension::Nine,
            "CΔ9",
            &["C", "E", "G", "B", "D"],
        ),
        (
            SeventhQuality::Minor,
            ChordExtension::Eleven,
            "C11",
            &["C", "E", "G", "Bb", "D", "F"],
        ),
        (
            SeventhQuality::Minor,
            ChordExtension::Thirteen,
            "C13",
            &["C", "E", "G", "Bb", "D", "F", "A"],
        ),
    ];
    for (seventh, token, display, tones) in cases {
        let spelling = ChordSpelling::new(c(), TriadQuality::Major, seventh, &[token]);
        assert_eq!(spelling.display(), display);
        assert_eq!(tone_displays(&spelling), tones);
    }
}

#[test]
fn add9_is_still_a_ninth_with_no_seventh() {
    let add_nine = ChordSpelling::new(
        c(),
        TriadQuality::Major,
        SeventhQuality::None,
        &[ChordExtension::AddNine],
    );
    assert_eq!(add_nine.display(), "Cadd9");
    assert_eq!(tone_displays(&add_nine), ["C", "E", "G", "D"]);
}

#[test]
fn a_suspension_replaces_the_third_rather_than_adding_a_note() {
    let cases: [(ChordExtension, SeventhQuality, &str, &[&str]); 3] = [
        (
            ChordExtension::Sus4,
            SeventhQuality::None,
            "Csus4",
            &["C", "F", "G"],
        ),
        (
            ChordExtension::Sus2,
            SeventhQuality::None,
            "Csus2",
            &["C", "D", "G"],
        ),
        (
            ChordExtension::Sus4,
            SeventhQuality::Minor,
            "C7sus4",
            &["C", "F", "G", "Bb"],
        ),
    ];
    for (token, seventh, display, tones) in cases {
        let spelling = ChordSpelling::new(c(), TriadQuality::Major, seventh, &[token]);
        assert_eq!(spelling.display(), display);
        assert_eq!(tone_displays(&spelling), tones);
        assert!(!spelling.spoken().contains("major 3"));
    }
}

/// The omissibility ruling, asserted as rules rather than as examples.
#[test]
fn only_the_notes_a_chord_can_spare_are_marked_omissible() {
    for entry in &QualityCatalog::shared().all {
        let chord = &entry.chord;
        let spelling = &chord.spelling;
        let tones = &chord.tones;
        let display = entry.display();

        assert!(!tones[0].omissible(), "{display} would drop its root");

        if spelling.omits_the_third() {
            assert_eq!(tones.len(), 2, "{display} is not two notes");
            assert!(
                tones.iter().all(|t| !t.omissible()),
                "{display} would drop a note"
            );
            continue;
        }

        assert!(!tones[1].omissible(), "{display} would drop its third");

        let fifth = &tones[2];
        let fifth_is_perfect = spelling
            .root
            .pitch_class()
            .interval_to(fifth.note.pitch_class())
            == 7;
        if fifth.omissible() {
            assert!(fifth_is_perfect, "{display} would drop an altered fifth");
            assert!(
                spelling.seventh != SeventhQuality::None,
                "{display} has no seventh to carry it"
            );
        }

        if spelling.seventh != SeventhQuality::None {
            assert!(!tones[3].omissible(), "{display} would drop its seventh");
        }
        if let (Some(last), true) = (tones.last(), spelling.seventh_is_implied()) {
            assert!(
                !last.omissible(),
                "{display} would drop the top of its own stack"
            );
        }
    }
}

#[test]
fn a_thirteenth_is_named_from_the_notes_a_player_actually_holds() {
    let as_played = analyze(&[0, 4, 10, 2, 9], 0);
    let headline = as_played.headline.as_ref().expect("named");
    assert_eq!(headline.display, "C13");
    assert_eq!(headline.pitch_classes, pcs(&[0, 4, 7, 10, 2, 5, 9]));
    assert_eq!(headline.fit, ChordFit::Missing(vec![5, 11]));

    let complete = analyze(&[0, 4, 7, 10, 2, 5, 9], 0);
    let complete_headline = complete.headline.as_ref().expect("named");
    assert_eq!(complete_headline.display, "C13");
    assert!(complete_headline.score > headline.score);

    let without_the_thirteenth = analyze(&[0, 4, 10, 2], 0);
    assert_eq!(
        without_the_thirteenth.headline.map(|h| h.display),
        Some("C9".into())
    );
}

#[test]
fn extensions_are_written_in_one_canonical_order_however_they_are_supplied() {
    let scrambled = ChordSpelling::new(
        c(),
        TriadQuality::Major,
        SeventhQuality::None,
        &[ChordExtension::AddNine, ChordExtension::Six],
    );
    assert_eq!(scrambled.display(), "C6add9");
    assert_eq!(
        scrambled.extensions,
        [ChordExtension::Six, ChordExtension::AddNine]
    );
}
