//! Ported case for case from note-view `Tests/ChordEngineTests/VocabularyTests.swift`.
//! The chords ticket 21 added, and the names that must not have moved.

mod common;

use std::collections::BTreeSet;

use common::{analyze, displays, mask_of, pcs_of_mask};
use watchord_core::tuning;
use watchord_core::{ChordNaming, DeclineReason, SoundingSet};
use watchord_engine::quality_catalog::QualityCatalog;
use watchord_engine::{
    ChordExtension, ChordSpelling, NamingEngine, NoteLetter, NoteSpelling, SeventhQuality,
    TriadQuality,
};

fn c() -> NoteSpelling {
    NoteSpelling::new(NoteLetter::C, 0).expect("C")
}

fn keys(spelling: &ChordSpelling) -> Option<Vec<i32>> {
    spelling
        .spelled()
        .map(|s| s.pitch_classes.iter().map(|pc| pc.value() as i32).collect())
}

fn names(display: &str, pitch_classes: &[i32], bass: i32) -> bool {
    displays(&analyze(pitch_classes, bass))
        .iter()
        .any(|d| d == display)
}

fn tone_displays(spelling: &ChordSpelling) -> Vec<String> {
    spelling
        .spelled()
        .expect("writable")
        .tones
        .iter()
        .map(|t| t.note.display())
        .collect()
}

fn catalog_has(pred: impl Fn(&str) -> bool) -> bool {
    QualityCatalog::shared()
        .all
        .iter()
        .any(|e| pred(&e.display()))
}

#[test]
fn a_root_and_a_fifth_name_a_power_chord() {
    let spelling = ChordSpelling::new(
        c(),
        TriadQuality::Major,
        SeventhQuality::None,
        &[ChordExtension::Five],
    );
    assert_eq!(spelling.display(), "C5");
    assert_eq!(keys(&spelling), Some(vec![0, 7]));
    let spelled = spelling.spelled().expect("writable");
    assert_eq!(tone_displays(&spelling), ["C", "G"]);
    assert!(spelled.tones.iter().all(|t| !t.omissible()));
    assert!(spelled.omissible_pitch_classes().is_empty());

    let analysis = analyze(&[0, 7], 0);
    assert_eq!(
        analysis.headline.as_ref().map(|h| h.display.as_str()),
        Some("C5")
    );
    assert_eq!(analysis.declined_reason, None);
}

#[test]
fn a_power_chord_is_never_spoken_as_major_or_minor() {
    let spoken = ChordSpelling::new(
        c(),
        TriadQuality::Major,
        SeventhQuality::None,
        &[ChordExtension::Five],
    )
    .spoken();
    assert_eq!(spoken, "C 5, no 3");
    assert!(!spoken.contains("major"));
    assert!(!spoken.contains("minor"));
}

#[test]
fn the_naming_floor_is_now_load_bearing_rather_than_dead() {
    assert_eq!(tuning::MINIMUM_PITCH_CLASSES, 2);
    assert!(analyze(&[0, 7], 0).headline.is_some());
    assert_eq!(
        NamingEngine::new()
            .analyze(&SoundingSet::new([60, 72]))
            .declined_reason,
        Some(DeclineReason::SingleNote)
    );
}

#[test]
fn exactly_the_perfect_fifths_name_and_no_other_dyad_does() {
    let catalog = QualityCatalog::shared();
    let mut named = 0;
    for mask in 0..4096u16 {
        let set = pcs_of_mask(mask);
        if set.len() != 2 {
            continue;
        }
        let candidates = catalog.candidates(mask);
        let is_a_fifth = (set[1] - set[0]) % 12 == 7 || (set[0] - set[1] + 12) % 12 == 7;
        if is_a_fifth {
            named += 1;
            assert!(!candidates.is_empty(), "the fifth {set:?} has no name");
            assert!(
                candidates.iter().all(|e| e.display().ends_with('5')),
                "{set:?} offers something other than a power chord"
            );
        } else {
            assert!(candidates.is_empty(), "{set:?} is an interval, not a chord");
        }
    }
    assert_eq!(named, 12);
}

#[test]
fn a_suspension_now_carries_the_stack_above_it() {
    let cases: [(ChordExtension, &str, &[i32], &[&str]); 2] = [
        (
            ChordExtension::Nine,
            "C9sus4",
            &[0, 2, 5, 7, 10],
            &["C", "F", "G", "Bb", "D"],
        ),
        (
            ChordExtension::Thirteen,
            "C13sus4",
            &[0, 2, 5, 7, 9, 10],
            &["C", "F", "G", "Bb", "D", "A"],
        ),
    ];
    for (token, display, pitch_classes, tones) in cases {
        let spelling = ChordSpelling::new(
            c(),
            TriadQuality::Major,
            SeventhQuality::Minor,
            &[token, ChordExtension::Sus4],
        );
        assert_eq!(spelling.display(), display);
        assert_eq!(keys(&spelling).as_deref(), Some(pitch_classes));
        assert_eq!(tone_displays(&spelling), tones);
        assert!(names(display, pitch_classes, 0));
    }
}

#[test]
fn a_suspended_fourth_and_the_eleventh_above_it_are_one_note_not_two() {
    let spelling = ChordSpelling::new(
        c(),
        TriadQuality::Major,
        SeventhQuality::Minor,
        &[ChordExtension::Thirteen, ChordExtension::Sus4],
    );
    let spelled = spelling.spelled().expect("C13sus4 must be writable");
    let f: Vec<_> = spelled
        .tones
        .iter()
        .filter(|t| t.note.display() == "F")
        .collect();
    assert_eq!(f.len(), 1, "the fourth is named twice");
    assert!(
        !f[0].omissible(),
        "the suspension is not the chord's to drop"
    );

    assert!(names("C13sus4", &[0, 5, 9, 10], 0));
    assert!(!names("C13sus4", &[0, 4, 9, 10], 0));
    assert!(!names("C13sus4", &[0, 5, 10], 0));
}

#[test]
fn an_added_fourth_names_a_chord_that_previously_declined() {
    let spelling = ChordSpelling::new(
        c(),
        TriadQuality::Major,
        SeventhQuality::None,
        &[ChordExtension::AddFour],
    );
    assert_eq!(spelling.display(), "Cadd4");
    assert_eq!(keys(&spelling), Some(vec![0, 4, 5, 7]));
    assert_eq!(tone_displays(&spelling), ["C", "E", "G", "F"]);
    assert!(names("Cadd4", &[0, 4, 5, 7], 0));
}

#[test]
fn add2_is_generated_for_ticket_22_but_never_takes_add9s_name() {
    let add_two = ChordSpelling::new(
        c(),
        TriadQuality::Major,
        SeventhQuality::None,
        &[ChordExtension::AddTwo],
    );
    let add_nine = ChordSpelling::new(
        c(),
        TriadQuality::Major,
        SeventhQuality::None,
        &[ChordExtension::AddNine],
    );
    assert_eq!(add_two.display(), "Cadd2");
    assert_eq!(
        keys(&add_two),
        keys(&add_nine),
        "the pair is not the same chord"
    );
    assert_eq!(keys(&add_two), Some(vec![0, 2, 4, 7]));

    assert!(catalog_has(|d| d == "Cadd2"));

    let analysis = analyze(&[0, 2, 4, 7], 0);
    assert!(displays(&analysis).contains(&"Cadd9".to_string()));
    assert!(!displays(&analysis).contains(&"Cadd2".to_string()));
    assert!(!catalog_has(|d| d == "Cadd11"));

    let catalog = QualityCatalog::shared();
    for mask in 0..4096u16 {
        let offered: Vec<String> = catalog
            .candidates(mask)
            .iter()
            .map(|e| e.display())
            .collect();
        assert!(
            !offered.iter().any(|d| d.contains("add2")),
            "mask {mask} offers an add2: {offered:?}"
        );
    }

    let add_twos: Vec<_> = catalog
        .all
        .iter()
        .filter(|e| {
            e.chord
                .spelling
                .extensions
                .contains(&ChordExtension::AddTwo)
        })
        .collect();
    assert_eq!(add_twos.len(), NoteSpelling::root_spellings().len());
    for twin in add_twos {
        let same_keys = catalog.candidates(twin.mask());
        assert!(
            same_keys
                .iter()
                .any(|e| e.root() == twin.root() && e.display().ends_with("add9")),
            "{} was collapsed and add9 did not take its place",
            twin.display()
        );
    }
}

#[test]
fn the_minor_added_tone_chords_the_research_doc_calls_missing_already_name() {
    assert!(names("Cmadd9", &[0, 2, 3, 7], 0));
    assert!(names("Cm6add9", &[0, 2, 3, 7, 9], 0));
}

#[test]
fn a_diminished_triad_can_now_carry_a_major_seventh() {
    let spelling = ChordSpelling::new(c(), TriadQuality::Diminished, SeventhQuality::Major, &[]);
    assert_eq!(spelling.display(), "C-Δ7");
    assert_eq!(spelling.spoken(), "C dim, major 7");
    assert_eq!(keys(&spelling), Some(vec![0, 3, 6, 11]));
    assert_eq!(tone_displays(&spelling), ["C", "Eb", "Gb", "B"]);
    assert!(names("C-Δ7", &[0, 3, 6, 11], 0));
}

#[test]
fn the_flat_five_workaround_gives_way_to_the_diminished_triad_it_stood_in_for() {
    let shown = displays(&analyze(&[0, 3, 6, 11], 0));
    assert!(shown.contains(&"C-Δ7".to_string()));
    assert!(!shown.contains(&"CmΔ7b5".to_string()));
    assert!(!shown.is_empty());

    for root in 0..12 {
        let mut set: Vec<i32> = [root, root + 3, root + 6, root + 11]
            .iter()
            .map(|k| k % 12)
            .collect();
        set.sort_unstable();
        let all = displays(&analyze(&set, root));
        assert!(
            all.iter().any(|d| d.ends_with("-Δ7")),
            "{set:?} lost its dim-major7 reading"
        );
        assert!(
            !all.iter().any(|d| d.contains("mΔ7b5")),
            "{set:?} still offers {all:?}"
        );
    }
}

#[test]
fn a_dominant_seventh_carries_two_alterations_at_once() {
    use ChordExtension as X;
    let cases: [(&str, &[ChordExtension], &[i32]); 6] = [
        (
            "C13b9",
            &[X::Thirteen, X::FlatNine],
            &[0, 1, 4, 5, 7, 9, 10],
        ),
        (
            "C13#9",
            &[X::Thirteen, X::SharpNine],
            &[0, 3, 4, 5, 7, 9, 10],
        ),
        (
            "C7b9#11",
            &[X::FlatNine, X::SharpEleven],
            &[0, 1, 4, 6, 7, 10],
        ),
        (
            "C7b9b13",
            &[X::FlatNine, X::FlatThirteen],
            &[0, 1, 4, 7, 8, 10],
        ),
        (
            "C7#9#11",
            &[X::SharpNine, X::SharpEleven],
            &[0, 3, 4, 6, 7, 10],
        ),
        (
            "C7#9b13",
            &[X::SharpNine, X::FlatThirteen],
            &[0, 3, 4, 7, 8, 10],
        ),
    ];
    for (display, tokens, set) in cases {
        let spelling = ChordSpelling::new(c(), TriadQuality::Major, SeventhQuality::Minor, tokens);
        assert_eq!(spelling.display(), display);
        assert_eq!(keys(&spelling).as_deref(), Some(set));
        assert!(names(display, set, 0));
    }
}

#[test]
fn a_flattened_ninth_displaces_the_stacks_ninth_instead_of_sitting_beside_it() {
    let spelled = ChordSpelling::new(
        c(),
        TriadQuality::Major,
        SeventhQuality::Minor,
        &[ChordExtension::Thirteen, ChordExtension::FlatNine],
    )
    .spelled()
    .expect("writable");
    let notes: Vec<String> = spelled.tones.iter().map(|t| t.note.display()).collect();
    assert!(notes.contains(&"Db".to_string()));
    assert!(
        !notes.contains(&"D".to_string()),
        "C13b9 names both ninths: {notes:?}"
    );
    assert_eq!(spelled.pitch_classes.len(), 7);
}

#[test]
fn the_combination_bound_is_exactly_the_list_on_the_dominant_and_nowhere_else() {
    use ChordExtension as X;
    let mut combinations: BTreeSet<Vec<ChordExtension>> = BTreeSet::new();
    for triad in TriadQuality::ALL_CASES {
        for seventh in SeventhQuality::ALL_CASES {
            for set in QualityCatalog::extension_sets(triad, seventh) {
                if set.len() <= 1 {
                    continue;
                }
                combinations.insert(set.clone());
                if set == [X::Six, X::AddNine] {
                    continue;
                }
                assert!(
                    triad == TriadQuality::Major && seventh == SeventhQuality::Minor,
                    "{triad:?}/{seventh:?} carries {set:?}, and only the dominant may"
                );
            }
        }
    }
    let expected: BTreeSet<Vec<ChordExtension>> = [
        vec![X::Six, X::AddNine],
        vec![X::Nine, X::Sus4],
        vec![X::Thirteen, X::Sus4],
        vec![X::Thirteen, X::FlatNine],
        vec![X::Thirteen, X::SharpNine],
        vec![X::FlatNine, X::SharpEleven],
        vec![X::FlatNine, X::FlatThirteen],
        vec![X::SharpNine, X::SharpEleven],
        vec![X::SharpNine, X::FlatThirteen],
        vec![X::FlatFive, X::FlatNine, X::SharpNine, X::FlatThirteen],
    ]
    .into_iter()
    .collect();
    assert_eq!(combinations, expected);
}

#[test]
fn a_sharp_eleventh_is_not_a_flat_fifth() {
    let sharp_eleven = ChordSpelling::new(
        c(),
        TriadQuality::Major,
        SeventhQuality::Minor,
        &[ChordExtension::SharpEleven],
    );
    let flat_five = ChordSpelling::new(
        c(),
        TriadQuality::Major,
        SeventhQuality::Minor,
        &[ChordExtension::FlatFive],
    );
    assert_eq!(keys(&sharp_eleven), Some(vec![0, 4, 6, 7, 10]));
    assert_eq!(keys(&flat_five), Some(vec![0, 4, 6, 10]));

    assert!(names("C7#11", &[0, 4, 6, 7, 10], 0));
    assert!(!names("C7b5", &[0, 4, 6, 7, 10], 0));
    assert!(names("C7b5", &[0, 4, 6, 10], 0));
    assert!(!names("C7#11", &[0, 4, 6, 10], 0));
}

#[test]
fn the_altered_dominant_names_spelled_out_pending_a_token_from_michael() {
    use ChordExtension as X;
    let altered = ChordSpelling::new(
        c(),
        TriadQuality::Major,
        SeventhQuality::Minor,
        &[X::FlatFive, X::FlatNine, X::SharpNine, X::FlatThirteen],
    );
    assert_eq!(altered.display(), "C7b5b9#9b13");
    assert_eq!(keys(&altered), Some(vec![0, 1, 3, 4, 6, 8, 10]));
    assert!(names("C7b5b9#9b13", &[0, 1, 3, 4, 6, 8, 10], 0));
    assert!(!catalog_has(|d| d.contains("alt")));
}

#[test]
fn six_nine_keeps_the_spelling_it_already_had() {
    assert!(names("C6add9", &[0, 2, 4, 7, 9], 0));
    assert!(!catalog_has(|d| d.contains('/')));
}

#[test]
fn two_suspensions_in_one_name_are_not_written() {
    assert!(!catalog_has(|d| d.contains("sus2sus4")));
    assert!(!catalog_has(|d| d.contains("sus4sus2")));
}

#[test]
fn every_name_michael_has_seen_still_headlines_its_own_keys() {
    let cases: [(&str, &[i32]); 19] = [
        ("C-7", &[0, 3, 6, 10]),
        ("C-dim7", &[0, 3, 6, 9]),
        ("C+7", &[0, 4, 8, 10]),
        ("C7", &[0, 4, 7, 10]),
        ("C", &[0, 4, 7]),
        ("Cm", &[0, 3, 7]),
        ("C+", &[0, 4, 8]),
        ("C-", &[0, 3, 6]),
        ("CΔ7", &[0, 4, 7, 11]),
        ("CmΔ7", &[0, 3, 7, 11]),
        ("C6add9", &[0, 2, 4, 7, 9]),
        ("C7b5", &[0, 4, 6, 10]),
        ("C6", &[0, 4, 7, 9]),
        ("Cadd9", &[0, 2, 4, 7]),
        ("Csus4", &[0, 5, 7]),
        ("Csus2", &[0, 2, 7]),
        ("C7sus4", &[0, 5, 7, 10]),
        ("C9", &[0, 2, 4, 7, 10]),
        ("C13", &[0, 2, 4, 5, 7, 9, 10]),
    ];
    for (display, set) in cases {
        let analysis = analyze(set, set[0]);
        assert_eq!(
            analysis.headline.as_ref().map(|h| h.display.as_str()),
            Some(display),
            "{set:?} now headlines {:?}",
            displays(&analysis)
        );
    }
}

#[test]
fn the_bass_still_decides_between_c6_and_am7_and_both_are_still_listed() {
    assert_eq!(displays(&analyze(&[0, 4, 7, 9], 0)), ["C6", "Am7"]);
    assert_eq!(displays(&analyze(&[0, 4, 7, 9], 9)), ["Am7", "C6"]);
}

#[test]
fn coverage_improved_and_by_the_amount_this_batch_actually_accounts_for() {
    let catalog = QualityCatalog::shared();
    let named = (1..4096u16)
        .filter(|&m| !catalog.candidates(m).is_empty())
        .count();
    assert_eq!(named, 1231);

    let dyads = (1..4096u16)
        .filter(|&m| pcs_of_mask(m).len() == 2)
        .filter(|&m| !catalog.candidates(m).is_empty())
        .count();
    assert_eq!(dyads, 12);
    let _ = mask_of;
}

#[test]
fn the_qualities_this_batch_added_carry_weights_of_their_own() {
    assert_eq!(tuning::quality_commonness("power"), Some(12));
    assert_eq!(tuning::quality_commonness("diminishedMajor7"), Some(3));
    assert!(
        tuning::QUALITY_COMMONNESS
            .iter()
            .all(|(_, v)| (0..=18).contains(v))
    );

    let keys: BTreeSet<&str> = QualityCatalog::shared()
        .all
        .iter()
        .map(|e| e.commonness_key)
        .collect();
    assert!(keys.contains("power"));
    assert!(keys.contains("diminishedMajor7"));
}
