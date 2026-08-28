//! Ported case for case from note-view `Tests/ChordEngineTests/HonestyTests.swift`.
//! The invariants, swept over every chord the engine can be handed.

mod common;

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use common::{all_readings, displays, entry_named, every_chord, pcs_of_mask, sounding};
use watchord_core::tuning;
use watchord_core::{
    ChordAnalysis, ChordFit, ChordNaming, ChordReading, DeclineReason, PitchClass, SoundingSet,
    SpellingOrigin,
};
use watchord_engine::NamingEngine;
use watchord_engine::quality_catalog::QualityCatalog;

fn engine_analysis(played: &SoundingSet) -> ChordAnalysis {
    NamingEngine::new().analyze(played)
}

fn root_pitch_class(reading: &ChordReading) -> Option<PitchClass> {
    entry_named(&reading.display, false).map(|e| e.root().pitch_class())
}

/// Encodes what the screen is actually shown, in list order.
fn encoded(analysis: &ChordAnalysis) -> String {
    let rows: Vec<serde_json::Value> = all_readings(analysis)
        .iter()
        .map(|r| {
            serde_json::json!({
                "display": r.display,
                "spoken": r.spoken,
                "origin": r.origin.raw_value(),
                "score": r.score,
                "pitchClasses": r.pitch_classes.iter().map(|pc| pc.value()).collect::<Vec<_>>(),
            })
        })
        .collect();
    serde_json::to_string(&rows).expect("rows encode")
}

#[test]
fn every_reading_accounts_for_the_difference_between_its_keys_and_the_played_ones() {
    for (set, bass) in every_chord() {
        let played = sounding(&set, bass);
        let expected = played.pitch_classes();
        for reading in all_readings(&engine_analysis(&played)) {
            let claimed = &reading.pitch_classes;
            let unexplained: BTreeSet<PitchClass> = expected.difference(claimed).copied().collect();
            let unplayed: BTreeSet<PitchClass> = claimed.difference(&expected).copied().collect();
            let where_ = format!("{} on {set:?}", reading.display);
            match &reading.fit {
                ChordFit::Exact => assert_eq!(claimed, &expected, "{where_} says exact and is not"),
                ChordFit::Missing(degrees) => {
                    assert!(
                        unexplained.is_empty(),
                        "{where_} says missing but leaves a key out"
                    );
                    assert_eq!(
                        degrees.len(),
                        unplayed.len(),
                        "{where_} is missing {} keys but reports {degrees:?}",
                        unplayed.len()
                    );
                }
                ChordFit::Plus(extras) => {
                    assert!(
                        unplayed.is_empty(),
                        "{where_} says plus but claims a key not played"
                    );
                    let extras: BTreeSet<PitchClass> = extras.iter().copied().collect();
                    assert_eq!(extras, unexplained, "{where_} reports the wrong extras");
                }
                ChordFit::Nearest { .. } => {
                    assert!(
                        !unexplained.is_empty() && !unplayed.is_empty(),
                        "{where_} is not nearest"
                    );
                }
            }
        }
    }
}

#[test]
fn no_sounding_key_is_ever_silently_unexplained_over_all_4095_sets() {
    for mask in 1..4096u16 {
        let set = pcs_of_mask(mask);
        let played = sounding(&set, set[0]);
        let analysis = engine_analysis(&played);
        if analysis.headline.is_none() {
            assert!(analysis.declined_reason.is_some(), "silence on {set:?}");
            continue;
        }
        for reading in all_readings(&analysis) {
            let unexplained: BTreeSet<PitchClass> = played
                .pitch_classes()
                .difference(&reading.pitch_classes)
                .copied()
                .collect();
            if unexplained.is_empty() {
                continue;
            }
            let accounted_for = match &reading.fit {
                ChordFit::Plus(extras) => {
                    extras.iter().copied().collect::<BTreeSet<_>>() == unexplained
                }
                ChordFit::Nearest { .. } => true,
                ChordFit::Exact | ChordFit::Missing(_) => false,
            };
            assert!(
                accounted_for,
                "{} drops {unexplained:?} silently",
                reading.display
            );
        }
    }
}

#[test]
fn every_chord_inside_the_naming_window_is_named_and_none_declines_for_want_of_a_fit() {
    let mut named = 0;
    let mut namable = 0;
    for mask in 1..4096u16 {
        let set = pcs_of_mask(mask);
        let analysis = engine_analysis(&sounding(&set, set[0]));
        assert_ne!(
            analysis.declined_reason,
            Some(DeclineReason::NoHonestReading),
            "{set:?} has nothing to say"
        );
        if !(tuning::MINIMUM_PITCH_CLASSES..=tuning::MAXIMUM_PITCH_CLASSES).contains(&set.len()) {
            assert!(
                analysis.headline.is_none(),
                "{set:?} is outside the window but was named"
            );
            continue;
        }
        namable += 1;
        if analysis.headline.is_some() {
            named += 1;
        }
    }
    assert_eq!(
        named,
        namable,
        "{} chords inside the window went unnamed",
        namable - named
    );
    assert_eq!(
        namable, 3784,
        "the window is not the size it was measured to be"
    );
}

#[test]
fn a_readings_text_and_its_keys_are_two_views_of_the_same_spelling() {
    for (set, bass) in every_chord() {
        let played = sounding(&set, bass);
        for reading in all_readings(&engine_analysis(&played)) {
            let entry = entry_named(&reading.display, false).unwrap_or_else(|| {
                panic!(
                    "{} is not a spelling the catalog generated",
                    reading.display
                )
            });
            let respelled = entry.chord.spelling.spelled().expect("respellable");
            assert_eq!(entry.chord.spelling.display(), reading.display);
            assert_eq!(entry.chord.spelling.spoken(), reading.spoken);
            assert_eq!(
                reading.pitch_classes, respelled.pitch_classes,
                "{} claims keys its own spelling does not",
                reading.display
            );
        }
    }
}

#[test]
fn no_chord_offers_two_readings_on_the_same_root_spelling() {
    let catalog = QualityCatalog::shared();
    for mask in 0..4096u16 {
        let roots: Vec<String> = catalog
            .candidates(mask)
            .iter()
            .map(|e| e.root().display())
            .collect();
        let distinct: BTreeSet<&String> = roots.iter().collect();
        assert_eq!(
            distinct.len(),
            roots.len(),
            "mask {mask} repeats a root: {roots:?}"
        );
    }
}

#[test]
fn a_headline_and_a_decline_are_exclusive_and_one_of_them_always_happens() {
    for (set, bass) in every_chord() {
        let analysis = engine_analysis(&sounding(&set, bass));
        assert_eq!(
            analysis.headline.is_none(),
            analysis.declined_reason.is_some()
        );
        if analysis.headline.is_none() {
            assert!(analysis.alternates.is_empty());
        }
    }
}

#[test]
fn every_reading_is_tagged_with_the_axis_it_actually_came_off() {
    for (set, bass) in every_chord() {
        let analysis = engine_analysis(&sounding(&set, bass));
        let Some(headline) = &analysis.headline else {
            continue;
        };
        assert_eq!(headline.origin, SpellingOrigin::Headline);
        let headline_root = root_pitch_class(headline);
        for alternate in &analysis.alternates {
            let expected = if root_pitch_class(alternate) == headline_root {
                SpellingOrigin::Enharmonic
            } else {
                SpellingOrigin::ReRooted
            };
            assert_eq!(
                alternate.origin, expected,
                "{} mis-tagged",
                alternate.display
            );
        }
    }
}

#[test]
fn naming_the_same_chord_twice_produces_the_same_bytes() {
    for (set, bass) in every_chord() {
        let played = sounding(&set, bass);
        assert_eq!(
            encoded(&engine_analysis(&played)),
            encoded(&engine_analysis(&played))
        );
    }
}

#[test]
fn only_the_bass_survives_from_the_voicing_order_and_octave_do_not() {
    for (set, bass) in every_chord() {
        let plain = sounding(&set, bass);
        let mut reversed: Vec<u8> = plain.midi_notes().to_vec();
        reversed.reverse();
        let shuffled = SoundingSet::new(reversed);
        assert_eq!(
            encoded(&engine_analysis(&plain)),
            encoded(&engine_analysis(&shuffled))
        );

        let notes = plain.midi_notes();
        let mut doubled: Vec<u8> = vec![notes[0]];
        doubled.extend(notes[1..].iter().map(|n| n + 12));
        doubled.push(notes[0] + 24);
        let doubled = SoundingSet::new(doubled);
        assert_eq!(
            displays(&engine_analysis(&plain)),
            displays(&engine_analysis(&doubled))
        );
    }
}

#[test]
fn naming_a_chord_costs_far_less_than_the_settle_interval() {
    let engine = NamingEngine::new();
    let _ = engine.analyze(&sounding(&[0, 4, 7, 9], 0));
    let chords = [
        sounding(&[0, 4, 7], 0),
        sounding(&[0, 4, 7, 9], 9),
        sounding(&[0, 3, 6, 9], 3),
        sounding(&[1, 5, 8, 10, 3], 1),
        sounding(&[0, 1, 2, 3, 4], 0),
    ];
    let started = Instant::now();
    for _ in 0..200 {
        for chord in &chords {
            let _ = engine.analyze(chord);
        }
    }
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_millis(1000),
        "1000 namings took {elapsed:?}"
    );
}
