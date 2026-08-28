//! Ported case for case from note-view `Tests/ChordEngineTests/RankingTests.swift`.
//! The ordering, pinned chord by chord.

mod common;

use std::collections::BTreeSet;

use common::{all_readings, analyze, displays, every_chord, mask_of, pcs, pcs_of_mask, sounding};
use watchord_core::tuning;
use watchord_core::{ChordNaming, PitchClass};
use watchord_engine::NamingEngine;
use watchord_engine::chord_ranking::{ChordRanking, ScoredReading};
use watchord_engine::quality_catalog::{CatalogEntry, QualityCatalog};

#[test]
fn the_bass_and_only_the_bass_decides_between_c6_and_am7() {
    assert_eq!(displays(&analyze(&[0, 4, 7, 9], 0)), ["C6", "Am7"]);
    assert_eq!(displays(&analyze(&[0, 4, 7, 9], 9)), ["Am7", "C6"]);
}

#[test]
fn all_six_spellings_of_a_diminished_seventh_come_out_in_one_fixed_order() {
    assert_eq!(
        displays(&analyze(&[0, 3, 6, 9], 0)),
        [
            "C-dim7", "A-dim7", "D#-dim7", "F#-dim7", "Eb-dim7", "Gb-dim7"
        ]
    );
}

#[test]
fn a_natural_root_outranks_an_accidental_one_when_nothing_else_separates_them() {
    assert_eq!(
        displays(&analyze(&[0, 4, 8], 0)),
        ["C+", "E+", "Ab+", "G#+"]
    );
}

#[test]
fn an_unaltered_reading_outranks_a_contorted_one_when_the_bass_helps_neither() {
    assert_eq!(
        displays(&analyze(&[0, 1, 4, 8], 0)),
        ["C#mΔ7", "DbmΔ7", "E+6"]
    );
}

#[test]
fn a_commoner_quality_outranks_a_rarer_one_when_the_bass_favours_neither() {
    assert_eq!(displays(&analyze(&[0, 3, 5, 8], 0)), ["Fm7", "Ab6", "G#6"]);
}

#[test]
fn the_headline_score_is_the_sum_of_the_terms_ticket_05_lists() {
    let analysis = analyze(&[0, 4, 7, 9], 0);
    let c6 = analysis.headline.as_ref().expect("named");
    //  +100 root C is the bass, +20 root present, +15 all tones accounted for,
    //  +12 major6 commonness, -6 one extension, +3 natural root, +2 no double accidentals
    assert_eq!(c6.score, 146);
    assert_eq!(
        c6.score - tuning::weight::ROOT_IS_PRESENT - tuning::weight::ALL_TONES_ACCOUNTED_FOR,
        111
    );
}

#[test]
fn every_reading_of_a_chord_is_paid_both_constant_bonuses() {
    let catalog = QualityCatalog::shared();
    for mask in 0..4096u16 {
        let set = pcs_of_mask(mask);
        if !(2..=8).contains(&set.len()) {
            continue;
        }
        for entry in catalog.candidates(mask) {
            let sounding = pcs(&set);
            assert!(sounding.contains(&entry.root().pitch_class()));
            assert_eq!(entry.pitch_classes, sounding);
        }
    }
}

#[test]
fn equal_scores_break_toward_the_reading_with_fewer_alterations() {
    assert_eq!(
        displays(&analyze(&[0, 2, 3, 6, 9], 2)),
        ["D7b9", "F#-dim7b13", "Gb-dim7b13", "C-6add9"]
    );
}

#[test]
fn a_tied_pair_really_is_separated_by_the_omission_not_by_the_root() {
    let analysis = analyze(&[1, 3, 4, 7], 4);
    let readings = all_readings(&analysis);
    let complete = readings
        .iter()
        .position(|r| r.display == "Db-add9")
        .expect("Db-add9");
    let missing_its_fifth = readings
        .iter()
        .position(|r| r.display == "D#7b9")
        .expect("D#7b9");
    assert_eq!(readings[complete].score, readings[missing_its_fifth].score);
    assert!(complete < missing_its_fifth);
}

fn scored_for(entries: &[CatalogEntry], played: &watchord_core::SoundingSet) -> Vec<ScoredReading> {
    let sounding = played.pitch_classes();
    entries
        .iter()
        .map(|e| {
            let score = ChordRanking::score(e, played.bass_pitch_class(), &sounding);
            ScoredReading::measured(e.clone(), &sounding, score)
        })
        .collect()
}

#[test]
fn the_order_comes_from_the_comparator_not_from_the_order_candidates_arrive_in() {
    let catalog = QualityCatalog::shared();
    for mask in 0..4096u16 {
        let set = pcs_of_mask(mask);
        if !(2..=8).contains(&set.len()) {
            continue;
        }
        let entries = catalog.candidates(mask);
        if entries.len() <= 1 {
            continue;
        }
        for &bass in &set {
            let played = sounding(&set, bass);
            let scored = scored_for(entries, &played);
            let mut reversed = scored.clone();
            reversed.reverse();
            let forwards: Vec<String> = ChordRanking::rank(scored)
                .iter()
                .map(|s| s.entry.display())
                .collect();
            let backwards: Vec<String> = ChordRanking::rank(reversed)
                .iter()
                .map(|s| s.entry.display())
                .collect();
            assert_eq!(forwards, backwards, "input order reached the output");
        }
    }
}

fn entry(display: &str, keys: &[i32]) -> Option<CatalogEntry> {
    QualityCatalog::shared()
        .candidates(mask_of(keys))
        .iter()
        .find(|e| e.display() == display)
        .cloned()
}

#[test]
fn every_note_left_out_is_charged_for() {
    let complete = entry("C13", &[0, 4, 7, 10, 2, 5, 9]).expect("C13 complete");
    let without_fifth = entry("C13", &[0, 4, 10, 2, 5, 9]).expect("C13 no5");
    let as_hands_play_it = entry("C13", &[0, 4, 10, 2, 9]).expect("C13 no5 no11");

    assert_eq!(complete.omitted_tone_count(), 0);
    assert_eq!(without_fifth.omitted_tone_count(), 1);
    assert_eq!(as_hands_play_it.omitted_tone_count(), 2);

    let scores: Vec<i32> = [&complete, &without_fifth, &as_hands_play_it]
        .iter()
        .map(|e| ChordRanking::score(e, Some(PitchClass::new(0)), &e.pitch_classes))
        .collect();
    assert_eq!(scores[0] - scores[1], -tuning::weight::PER_ALTERATION);
    assert_eq!(scores[0] - scores[2], -2 * tuning::weight::PER_ALTERATION);
}

#[test]
fn an_equal_score_always_goes_to_the_reading_with_nothing_missing() {
    let catalog = QualityCatalog::shared();
    for (set, bass) in every_chord() {
        let played = sounding(&set, bass);
        let entries =
            catalog.candidates(QualityCatalog::mask(played.pitch_classes().iter().copied()));
        let ranked = ChordRanking::rank(scored_for(entries, &played));
        for pair in ranked.windows(2) {
            let (earlier, later) = (&pair[0], &pair[1]);
            if earlier.score == later.score {
                assert!(
                    earlier.entry.omitted_tone_count() <= later.entry.omitted_tone_count(),
                    "{} is ranked above {} on an equal score",
                    earlier.entry.display(),
                    later.entry.display()
                );
            }
        }
    }
}

#[test]
fn readings_never_tie_without_a_tiebreak_so_no_two_orders_are_possible() {
    for mask in 0..4096u16 {
        let set = pcs_of_mask(mask);
        if !(2..=8).contains(&set.len()) {
            continue;
        }
        for &bass in &set {
            let analysis = NamingEngine::new().analyze(&sounding(&set, bass));
            let readings = all_readings(&analysis);
            let distinct: BTreeSet<&str> = readings.iter().map(|r| r.display.as_str()).collect();
            assert_eq!(distinct.len(), readings.len());
            assert!(readings.windows(2).all(|w| w[0].score >= w[1].score));
        }
    }
}
