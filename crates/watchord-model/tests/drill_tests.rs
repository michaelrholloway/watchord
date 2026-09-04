//! Drill through the fakes (spec #9, ticket #14): entering it, grading a
//! settled sounding set against the target with the fit tiers, and weighting
//! future draws toward weak chords. No terminal in this file, same
//! discipline as `app_model_tests.rs`.

use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use watchord_core::{ChordKey, DrillChordStat, DrillStoring, PitchClass, SoundingSet};
use watchord_model::AppModel;
use watchord_model::fakes::{
    FixedVocabulary, InMemoryDrillStore, InMemoryNoteStore, ScriptedSoundingSetSource,
    StubChordNaming,
};

fn key(values: &[i32]) -> ChordKey {
    ChordKey::new(values.iter().map(|&v| PitchClass::new(v)))
}

fn vocabulary() -> FixedVocabulary {
    FixedVocabulary::new(vec![
        FixedVocabulary::target("C", &[0, 4, 7]),
        FixedVocabulary::target("Dm", &[2, 5, 9]),
    ])
}

fn model_with_drill(store: InMemoryDrillStore) -> AppModel {
    AppModel::new(
        Arc::new(StubChordNaming::new()),
        Box::new(ScriptedSoundingSetSource::default()),
        Arc::new(InMemoryNoteStore::default()),
    )
    .with_drill(Arc::new(vocabulary()), Arc::new(store))
}

#[test]
fn a_model_with_no_drill_seams_never_offers_to_enter() {
    let mut model = AppModel::new(
        Arc::new(StubChordNaming::new()),
        Box::new(ScriptedSoundingSetSource::default()),
        Arc::new(InMemoryNoteStore::default()),
    );
    assert!(!model.can_drill());
    model.enter_drill();
    assert!(!model.is_drilling());
    assert!(!model.frame().drill.active);
}

#[test]
fn entering_drill_shows_a_target_and_the_next_target() {
    let mut model = model_with_drill(InMemoryDrillStore::default());
    assert!(model.can_drill());
    assert!(!model.frame().drill.active);

    model.enter_drill();
    let frame = model.frame();
    assert!(frame.drill.active);
    let target = frame.drill.target.expect("a target");
    let next = frame.drill.next_target.expect("a next target");
    assert!(["C", "Dm"].contains(&target.as_str()));
    assert!(["C", "Dm"].contains(&next.as_str()));
}

#[test]
fn toggling_drill_twice_returns_to_off() {
    let mut model = model_with_drill(InMemoryDrillStore::default());
    model.toggle_drill();
    assert!(model.is_drilling());
    model.toggle_drill();
    assert!(!model.is_drilling());
    assert!(model.frame().drill.target.is_none());
}

#[test]
fn an_exact_attempt_grades_exact_and_advances_to_the_next_target() {
    let mut model = model_with_drill(InMemoryDrillStore::default());
    model.enter_drill();
    let first_target = model.frame().drill.target.clone().unwrap();
    let first_next = model.frame().drill.next_target.clone().unwrap();
    let notes: Vec<u8> = match first_target.as_str() {
        "C" => vec![60, 64, 67],
        "Dm" => vec![62, 65, 69],
        other => panic!("unexpected target {other}"),
    };

    model.receive(&SoundingSet::new(notes));

    let frame = model.frame();
    assert_eq!(frame.drill.grade.as_deref(), Some("exact"));
    assert_eq!(frame.drill.grade_note, None, "exact carries no note");
    assert_eq!(
        frame.drill.target.as_deref(),
        Some(first_next.as_str()),
        "the next target becomes current"
    );
    assert!(frame.drill.next_target.is_some(), "a fresh next is drawn");
}

#[test]
fn a_mismatched_attempt_grades_by_tier_and_names_what_was_missed_or_extra() {
    let mut model = model_with_drill(InMemoryDrillStore::default());
    model.enter_drill();
    // Force the target to C (0 4 7) by exiting/entering until it lands there
    // — the fixed two-chord vocabulary makes this bounded and cheap.
    for _ in 0..50 {
        if model.frame().drill.target.as_deref() == Some("C") {
            break;
        }
        model.toggle_drill();
        model.toggle_drill();
    }
    assert_eq!(model.frame().drill.target.as_deref(), Some("C"));

    // Play C E only — G never sounds: `missing`.
    model.receive(&SoundingSet::new([60, 64]));
    let frame = model.frame();
    assert_eq!(frame.drill.grade.as_deref(), Some("missing"));
    assert!(
        frame.drill.grade_note.as_deref().unwrap().contains('G'),
        "{:?}",
        frame.drill.grade_note
    );
}

#[test]
fn drill_stats_list_attempts_exact_and_last_tried_per_chord() {
    let store = InMemoryDrillStore::default();
    let c_major = key(&[0, 4, 7]);
    store
        .record(
            &c_major,
            true,
            UNIX_EPOCH + Duration::from_secs(1_800_000_000),
        )
        .unwrap();
    store
        .record(
            &c_major,
            false,
            UNIX_EPOCH + Duration::from_secs(1_800_000_100),
        )
        .unwrap();

    let model = model_with_drill(store);
    let frame = model.frame();
    assert_eq!(frame.drill.stats.len(), 1);
    let row = &frame.drill.stats[0];
    assert_eq!(row.chord, "C");
    assert_eq!(row.attempts, 2);
    assert_eq!(row.exact, 1);
    assert_eq!(
        row.last_at,
        Some(UNIX_EPOCH + Duration::from_secs(1_800_000_100))
    );
}

/// Ticket #14's acceptance criterion, through the model rather than the
/// weighting function directly: over a seeded run of 200 draws, the chord
/// with more misses is drawn more often than the chord with none.
#[test]
fn weak_chords_draw_more_often_over_a_seeded_run_of_two_hundred() {
    let strong = key(&[0, 4, 7]); // "C" — a clean record
    let weak = key(&[2, 5, 9]); // "Dm" — missed a lot
    let store = InMemoryDrillStore::new(vec![
        DrillChordStat {
            chord_key: strong,
            attempts: 10,
            exact: 10,
            last_at: None,
        },
        DrillChordStat {
            chord_key: weak,
            attempts: 10,
            exact: 0,
            last_at: None,
        },
    ]);
    let mut model = model_with_drill(store);
    model.seed_drill_rng(7);

    let mut counts = std::collections::HashMap::new();
    for _ in 0..200 {
        model.enter_drill();
        let target = model.frame().drill.target.clone().unwrap();
        *counts.entry(target).or_insert(0u32) += 1;
        model.exit_drill();
    }

    let c_count = *counts.get("C").unwrap_or(&0);
    let dm_count = *counts.get("Dm").unwrap_or(&0);
    assert!(
        dm_count > c_count,
        "Dm (weak, {dm_count}) should draw more than C (clean, {c_count})"
    );
}

/// A drill session never touches the notes seam — only `receive` does, and
/// that already happens whether or not drill is active. This proves the
/// stats calls land on the drill store, not the note store.
#[test]
fn a_drill_session_never_calls_the_note_store() {
    let note_store = InMemoryNoteStore::default();
    let drill_store = InMemoryDrillStore::default();
    let mut model = AppModel::new(
        Arc::new(StubChordNaming::new()),
        Box::new(ScriptedSoundingSetSource::default()),
        Arc::new(note_store.clone()),
    )
    .with_drill(Arc::new(vocabulary()), Arc::new(drill_store.clone()));

    model.enter_drill();
    model.receive(&SoundingSet::new([60, 64, 67]));
    model.receive(&SoundingSet::silent());
    model.receive(&SoundingSet::new([62, 65, 69]));

    assert_eq!(
        note_store.add_call_count(),
        0,
        "grading a drill attempt must never write a note"
    );
    assert!(
        drill_store.record_call_count() >= 2,
        "each settled attempt records against the drill store"
    );
}
