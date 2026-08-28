//! Ported case for case from note-view `Tests/ChordEngineTests/FitTests.swift`.
//! Honesty as a property that is *shown* rather than a gate that *refuses*.

mod common;

use std::collections::{BTreeMap, BTreeSet};

use common::{all_readings, analyze, displays, entry_named, pcs, sounding};
use watchord_core::tuning::fit as weight;
use watchord_core::{ChordFit, ChordNaming, PitchClass};
use watchord_engine::NamingEngine;
use watchord_engine::chord_ranking::{ChordFitting, ChordRanking, ScoredReading};
use watchord_engine::quality_catalog::CatalogEntry;

fn complete(display: &str) -> CatalogEntry {
    entry_named(display, true).unwrap_or_else(|| panic!("{display} is in the catalog"))
}

#[test]
fn a_chord_played_complete_is_exact_and_says_nothing() {
    let analysis = analyze(&[0, 4, 7], 0);
    let headline = analysis.headline.as_ref().expect("named");
    assert_eq!(headline.display, "C");
    assert_eq!(headline.fit, ChordFit::Exact);
    assert_eq!(headline.fit_note(), "");
}

#[test]
fn missing_an_omission_the_catalog_already_allowed_is_shown_rather_than_hidden() {
    let analysis = analyze(&[0, 4, 10, 2, 9], 0);
    let headline = analysis.headline.as_ref().expect("named");
    assert_eq!(headline.display, "C13");
    assert_eq!(headline.fit, ChordFit::Missing(vec![5, 11]));
    assert_eq!(headline.fit_note(), "·no5 ·no11");
    assert_eq!(headline.pitch_classes, pcs(&[0, 2, 4, 5, 7, 9, 10]));
}

#[test]
fn plus_a_whole_chord_with_a_key_it_cannot_explain_names_the_chord_and_names_the_rest() {
    let played = sounding(&[0, 4, 5, 6, 7], 0);
    let analysis = NamingEngine::new().analyze(&played);
    let headline = analysis.headline.as_ref().expect("named");
    assert_eq!(headline.display, "Cadd4");
    assert_eq!(headline.fit, ChordFit::Plus(vec![PitchClass::new(6)]));
    assert_eq!(headline.fit_note(), "·+F#");
    assert!(headline.pitch_classes.is_subset(&played.pitch_classes()));
}

#[test]
fn nearest_a_reading_that_is_wrong_in_both_directions_is_marked_as_an_approximation() {
    let played = sounding(&[0, 1, 2, 3], 0);
    let analysis = NamingEngine::new().analyze(&played);
    let headline = analysis.headline.as_ref().expect("named");
    assert_eq!(headline.display, "C#Δ7sus2");
    assert!(
        matches!(headline.fit, ChordFit::Nearest { .. }),
        "{:?}",
        headline.fit
    );
    assert_eq!(headline.fit_note(), "≈");
    let played_pcs = played.pitch_classes();
    assert!(
        played_pcs
            .difference(&headline.pitch_classes)
            .next()
            .is_some()
    );
    assert!(
        headline
            .pitch_classes
            .difference(&played_pcs)
            .next()
            .is_some()
    );
}

#[test]
fn a_badly_fitting_reading_loses_to_a_well_fitting_one_however_high_it_scores() {
    let exact = complete("C");
    let approximate = complete("F#13");
    let ranked = ChordRanking::rank(vec![
        ScoredReading::stated(approximate, ChordFit::Nearest { distance: 9 }, 9, 10_000),
        ScoredReading::stated(exact, ChordFit::Exact, 0, 1),
    ]);
    let shown: Vec<String> = ranked.iter().map(|s| s.entry.display()).collect();
    assert_eq!(shown, ["C", "F#13"]);
    assert!(
        ranked[0].score < ranked[1].score,
        "the test is vacuous if the scores agree"
    );
}

#[test]
fn a_reading_that_leaves_a_key_unexplained_loses_to_one_that_explains_them_all() {
    let thin = complete("C13");
    let incomplete = complete("Am7");
    let ranked = ChordRanking::rank(vec![
        ScoredReading::stated(
            incomplete,
            ChordFit::Plus(vec![PitchClass::new(1)]),
            weight::UNEXPLAINED_KEY,
            500,
        ),
        ScoredReading::stated(thin, ChordFit::Missing(vec![5, 11]), 0, 5),
    ]);
    let shown: Vec<String> = ranked.iter().map(|s| s.entry.display()).collect();
    assert_eq!(shown, ["C13", "Am7"]);
}

#[test]
fn a_sanctioned_omission_still_competes_on_score_and_can_still_win() {
    let analysis = analyze(&[0, 2, 3, 6], 2);
    let headline = analysis.headline.as_ref().expect("named");
    assert_eq!(headline.display, "D7b9");
    assert_eq!(headline.fit, ChordFit::Missing(vec![5]));
    assert!(
        displays(&analysis).contains(&"C-add9".to_string()),
        "the complete reading is still offered"
    );

    let complete = analysis
        .alternates
        .iter()
        .find(|r| r.display == "C-add9")
        .expect("C-add9");
    assert_eq!(complete.fit, ChordFit::Exact);
    assert!(
        complete.score < headline.score,
        "the exception is about score, not about fit"
    );
}

#[test]
fn a_chord_that_names_exactly_is_not_padded_with_near_misses() {
    let analysis = analyze(&[0, 4, 7, 9], 0);
    assert_eq!(displays(&analysis), ["C6", "Am7"]);
    assert!(
        all_readings(&analysis)
            .iter()
            .all(|r| r.fit == ChordFit::Exact)
    );
}

#[test]
fn two_readings_on_one_root_can_meet_in_a_fall_through_list_and_are_still_ordered() {
    let offered = displays(&analyze(&[0, 4, 6], 0));
    assert!(offered.contains(&"C7b5".to_string()) && offered.contains(&"C7#11".to_string()));

    let pair: Vec<ScoredReading> = ["C7#11", "C7b5"]
        .iter()
        .map(|d| ScoredReading::stated(complete(d), ChordFit::Exact, 0, 0))
        .collect();
    assert_eq!(
        pair[0].entry.root().display(),
        pair[1].entry.root().display(),
        "the pair no longer ties on the root, so it no longer reaches the display level"
    );
    let mut reversed = pair.clone();
    reversed.reverse();
    let forwards: Vec<String> = ChordRanking::rank(pair)
        .iter()
        .map(|s| s.entry.display())
        .collect();
    let backwards: Vec<String> = ChordRanking::rank(reversed)
        .iter()
        .map(|s| s.entry.display())
        .collect();
    assert_eq!(forwards, ["C7#11", "C7b5"]);
    assert_eq!(
        forwards, backwards,
        "the order came from the input, not the comparator"
    );
}

#[test]
fn a_fall_through_list_holds_only_the_readings_that_fit_equally_well() {
    for keys in [
        vec![0, 4],
        vec![0, 1, 2],
        vec![0, 4, 6],
        vec![0, 4, 5, 6, 7],
        vec![0, 1, 2, 3],
    ] {
        let analysis = analyze(&keys, keys[0]);
        let tiers: BTreeSet<u8> = all_readings(&analysis)
            .iter()
            .map(|r| r.fit.tier())
            .collect();
        assert_eq!(tiers.len(), 1, "{keys:?} mixes tiers: {tiers:?}");
    }
}

/// Asserted as the ordering rather than as the numbers, because the numbers are
/// guesses and the ordering is the ruling. The values are consts, so the assertion
/// is constant by design: it goes red the moment somebody re-tunes.
#[test]
#[expect(
    clippy::assertions_on_constants,
    reason = "the ordering of the tuning consts is the claim under test"
)]
fn the_distance_metric_charges_what_the_ticket_says_it_charges() {
    assert_eq!(weight::SANCTIONED_OMISSION, 0);
    assert!(weight::SANCTIONED_OMISSION < weight::MISSING_PERFECT_FIFTH);
    assert!(weight::MISSING_PERFECT_FIFTH < weight::MISSING_ADDED_TONE);
    assert!(weight::MISSING_ADDED_TONE < weight::UNEXPLAINED_KEY);
    assert!(weight::UNEXPLAINED_KEY < weight::MISSING_IDENTITY_TONE);
}

#[test]
fn losing_the_root_the_third_or_the_seventh_is_the_dearest_thing_that_can_happen() {
    let chord = complete("C7").chord;
    let costs: BTreeMap<u8, i32> = chord
        .tones
        .iter()
        .map(|t| (t.degree.number(), ChordFitting::cost_of_absent(t)))
        .collect();
    assert_eq!(costs[&1], weight::MISSING_IDENTITY_TONE);
    assert_eq!(costs[&3], weight::MISSING_IDENTITY_TONE);
    assert_eq!(costs[&7], weight::MISSING_IDENTITY_TONE);
    assert_eq!(costs[&5], weight::SANCTIONED_OMISSION);
}

#[test]
fn an_altered_fifth_costs_what_an_identity_tone_costs_not_what_a_fifth_costs() {
    let altered = complete("C7b5").chord;
    let fifth = altered
        .tones
        .iter()
        .find(|t| t.degree.number() == 5)
        .expect("has a fifth");
    assert_eq!(
        ChordFitting::cost_of_absent(fifth),
        weight::MISSING_IDENTITY_TONE
    );
}
