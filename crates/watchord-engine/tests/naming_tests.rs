//! Ported case for case from note-view `Tests/ChordEngineTests/NamingTests.swift`.

mod common;

use std::collections::BTreeSet;

use common::{analyze, displays, mask_of, pcs_of_mask, sounding_set};
use watchord_core::tuning;
use watchord_core::{ChordFit, ChordNaming, DeclineReason, SoundingSet, SpellingOrigin};
use watchord_engine::quality_catalog::QualityCatalog;
use watchord_engine::{ChordExtension, NamingEngine};

#[test]
fn every_row_of_michaels_table_comes_back_when_its_own_keys_are_played() {
    for row in common::worked_table() {
        let spelled = row.spelling.spelled().expect("writable");
        let played = sounding_set(&spelled.pitch_classes, row.spelling.root.pitch_class());
        let analysis = NamingEngine::new().analyze(&played);
        let shown = displays(&analysis);
        assert!(
            shown.iter().any(|d| d == row.display),
            "{} is missing from {shown:?}",
            row.display
        );
    }
}

#[test]
fn c_e_g_a_headlines_c6_over_a_c_and_am7_over_an_a_and_lists_both_either_way() {
    let over_c = analyze(&[0, 4, 7, 9], 0);
    assert_eq!(
        over_c.headline.as_ref().map(|h| h.display.as_str()),
        Some("C6")
    );
    assert!(displays(&over_c).contains(&"Am7".to_string()));

    let over_a = analyze(&[0, 4, 7, 9], 9);
    assert_eq!(
        over_a.headline.as_ref().map(|h| h.display.as_str()),
        Some("Am7")
    );
    assert!(displays(&over_a).contains(&"C6".to_string()));

    assert_eq!(over_c.key, over_a.key);
    assert_eq!(over_c.key.raw(), "0.4.7.9");
}

#[test]
fn the_two_axes_are_labelled_apart() {
    let over_c = analyze(&[0, 4, 7, 9], 0);
    assert_eq!(
        over_c.headline.as_ref().map(|h| h.origin),
        Some(SpellingOrigin::Headline)
    );
    let am7 = over_c
        .alternates
        .iter()
        .find(|r| r.display == "Am7")
        .expect("Am7 offered");
    assert_eq!(am7.origin, SpellingOrigin::ReRooted);

    let c_sharp_major7 = analyze(&[1, 5, 8, 0], 1);
    assert_eq!(
        c_sharp_major7.headline.as_ref().map(|h| h.display.as_str()),
        Some("C#Δ7")
    );
    let flat = c_sharp_major7
        .alternates
        .iter()
        .find(|r| r.display == "DbΔ7")
        .expect("DbΔ7 offered");
    assert_eq!(flat.origin, SpellingOrigin::Enharmonic);
}

#[test]
fn a_chord_is_never_offered_under_a_root_that_renames_a_natural() {
    assert_eq!(displays(&analyze(&[0, 4, 7, 9], 0)), ["C6", "Am7"]);
    assert_eq!(displays(&analyze(&[0, 4, 7], 0)), ["C"]);
    assert_eq!(displays(&analyze(&[0, 4, 7, 10], 0)), ["C7"]);

    for entry in &QualityCatalog::shared().all {
        let root = entry.root();
        assert!(
            root.alteration == 0 || !root.pitch_has_a_natural_name(),
            "{} is rooted on {}",
            entry.display(),
            root.display()
        );
    }
}

#[test]
fn an_altered_fifth_is_not_invented_to_re_explain_a_chord_that_reads_plainly() {
    assert!(!displays(&analyze(&[0, 4, 7], 0)).contains(&"Em#5".to_string()));
    assert_eq!(displays(&analyze(&[0, 3, 8], 8)), ["Ab", "G#"]);
    assert!(displays(&analyze(&[0, 4, 6, 10], 0)).contains(&"C7b5".to_string()));
    assert!(displays(&analyze(&[1, 5, 7, 11], 1)).contains(&"C#7b5".to_string()));
}

#[test]
fn keys_the_notation_cannot_write_unambiguously_are_never_written() {
    let analysis = analyze(&[0, 4, 6], 0);
    assert!(!displays(&analysis).contains(&"Cb5".to_string()));
    let headline = analysis.headline.as_ref().expect("named");
    assert_ne!(
        headline.fit,
        ChordFit::Exact,
        "these keys have no exact spelling and must say so"
    );

    for entry in &QualityCatalog::shared().all {
        let altering = entry
            .chord
            .spelling
            .extensions
            .iter()
            .any(|x| matches!(x, ChordExtension::FlatFive | ChordExtension::SharpFive));
        if !altering {
            continue;
        }
        let symbols_before_it = format!(
            "{}{}",
            entry.chord.spelling.triad.symbol(),
            entry.chord.spelling.seventh.symbol()
        );
        assert!(
            !symbols_before_it.is_empty(),
            "{} runs its alteration into the root",
            entry.display()
        );
    }
}

#[test]
fn a_fully_diminished_chord_reads_from_each_of_its_four_tones() {
    let analysis = analyze(&[0, 3, 6, 9], 0);
    let shown = displays(&analysis);
    assert!(shown.iter().all(|d| d.ends_with("-dim7")), "{shown:?}");
    assert_eq!(common::all_readings(&analysis).len(), 6);

    let catalog = QualityCatalog::shared();
    let roots: BTreeSet<u8> = common::all_readings(&analysis)
        .iter()
        .filter_map(|reading| {
            catalog
                .candidates(QualityCatalog::mask(reading.pitch_classes.iter().copied()))
                .iter()
                .find(|e| e.display() == reading.display)
                .map(|e| e.root().pitch_class().value())
        })
        .collect();
    assert_eq!(roots, [0, 3, 6, 9].into_iter().collect());
}

#[test]
fn the_bass_picks_which_of_the_four_dim7_readings_is_the_headline() {
    for (bass, expected) in [(0, "C-dim7"), (3, "D#-dim7"), (6, "F#-dim7"), (9, "A-dim7")] {
        let analysis = analyze(&[0, 3, 6, 9], bass);
        assert_eq!(
            analysis.headline.as_ref().map(|h| h.display.as_str()),
            Some(expected)
        );
        let shown = displays(&analysis);
        assert!(shown.contains(&"Eb-dim7".to_string()));
        assert!(shown.contains(&"Gb-dim7".to_string()));
    }
}

#[test]
fn a_major_triad_never_acquires_a_plus_7_reading() {
    let analysis = analyze(&[0, 4, 7], 0);
    assert_eq!(
        analysis.headline.as_ref().map(|h| h.display.as_str()),
        Some("C")
    );
    assert!(displays(&analysis).iter().all(|d| !d.contains("+7")));
    assert!(
        common::all_readings(&analysis)
            .iter()
            .all(|r| r.pitch_classes.len() == 3)
    );

    for entry in &QualityCatalog::shared().all {
        for tone in entry.chord.tones.iter().filter(|t| !t.omissible()) {
            assert!(
                entry.pitch_classes.contains(&tone.note.pitch_class()),
                "{} is offered for keys missing its {}",
                entry.display(),
                tone.note.display()
            );
        }
    }
}

#[test]
fn c_sharp_dim_triad_with_a_minor_7_is_half_diminished_and_is_never_called_c_sharp_6() {
    let analysis = analyze(&[1, 4, 7, 11], 1);
    assert_eq!(
        analysis.headline.as_ref().map(|h| h.display.as_str()),
        Some("C#-7")
    );
    assert!(!displays(&analysis).contains(&"C#6".to_string()));

    let actual = analyze(&[1, 5, 8, 10], 1);
    assert!(displays(&actual).contains(&"C#6".to_string()));
    assert!(!displays(&actual).contains(&"C#-7".to_string()));
}

#[test]
fn silence_declines_as_silence() {
    let analysis = NamingEngine::new().analyze(&SoundingSet::silent());
    assert!(analysis.headline.is_none());
    assert!(analysis.alternates.is_empty());
    assert_eq!(analysis.declined_reason, Some(DeclineReason::Silent));
}

#[test]
fn one_note_is_below_the_naming_floor() {
    let analysis = NamingEngine::new().analyze(&SoundingSet::new([60, 72]));
    assert!(analysis.headline.is_none());
    assert_eq!(analysis.declined_reason, Some(DeclineReason::SingleNote));
    assert_eq!(analysis.sounding.distinct_pitch_class_count(), 1);
}

#[test]
fn more_distinct_pitch_classes_than_the_ceiling_declines_rather_than_guessing() {
    let all: Vec<u8> = (0..=tuning::MAXIMUM_PITCH_CLASSES)
        .map(|i| (60 + i) as u8)
        .collect();
    let analysis = NamingEngine::new().analyze(&SoundingSet::new(all));
    assert!(analysis.headline.is_none());
    assert_eq!(
        analysis.declined_reason,
        Some(DeclineReason::TooManyPitchClasses)
    );
}

#[test]
fn a_fifth_is_a_chord_of_its_own_the_other_dyads_get_the_nearest_chord_marked() {
    let fifth = analyze(&[0, 7], 0);
    assert!(fifth.sounding.distinct_pitch_class_count() >= tuning::MINIMUM_PITCH_CLASSES);
    let headline = fifth.headline.as_ref().expect("named");
    assert_eq!(headline.display, "C5");
    assert_eq!(headline.fit, ChordFit::Exact);

    let third = analyze(&[0, 4], 0);
    let headline = third.headline.as_ref().expect("named");
    assert_eq!(headline.display, "C");
    assert_eq!(headline.fit, ChordFit::Missing(vec![5]));
    assert_eq!(headline.fit_note(), "·no5");
}

#[test]
fn a_chord_no_spelling_fits_is_named_as_the_nearest_one_marked() {
    let played = common::sounding(&[0, 1, 2], 0);
    let analysis = NamingEngine::new().analyze(&played);
    let headline = analysis.headline.as_ref().expect("named");
    assert_eq!(analysis.declined_reason, None);
    assert_eq!(headline.display, "C#Δ7b9");
    assert_eq!(headline.fit, ChordFit::Missing(vec![3, 5]));
    assert!(played.pitch_classes().is_subset(&headline.pitch_classes));
}

#[test]
fn every_quality_the_catalog_generates_has_a_commonness_weight_to_be_scored_by() {
    let keys: BTreeSet<&str> = QualityCatalog::shared()
        .all
        .iter()
        .map(|e| e.commonness_key)
        .collect();
    assert!(!keys.is_empty());
    for key in keys {
        assert!(
            tuning::quality_commonness(key).is_some(),
            "'{key}' is generated but has no weight in tuning::QUALITY_COMMONNESS"
        );
    }
}

#[test]
fn no_two_candidates_for_one_chord_render_the_same_string() {
    let catalog = QualityCatalog::shared();
    for mask in 0..4096u16 {
        let shown: Vec<String> = catalog
            .candidates(mask)
            .iter()
            .map(|e| e.display())
            .collect();
        let distinct: BTreeSet<&String> = shown.iter().collect();
        assert_eq!(
            distinct.len(),
            shown.len(),
            "duplicate display in mask {mask}"
        );
    }
}

#[test]
fn a_synonym_on_the_same_root_is_dropped_in_favour_of_the_simpler_spelling() {
    let shown = displays(&analyze(&[0, 3, 6, 9], 0));
    assert!(shown.contains(&"C-dim7".to_string()));
    assert!(!shown.contains(&"C-6".to_string()));
    assert!(shown.iter().all(|d| !d.ends_with("-6")));
    let _ = (pcs_of_mask, mask_of);
}
