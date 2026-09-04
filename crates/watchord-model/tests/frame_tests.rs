//! The `Frame` is one value that holds everything the screen shows, and it
//! round-trips through JSON to an equal value. Tested through the fakes, as the
//! model tests are.

use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use watchord_core::{ChordFit, ChordNote, PitchClass, SoundingSet, SpellingOrigin};
use watchord_model::fakes::{
    InMemoryNoteStore, ScriptedSoundingSetSource, StubAlternate, StubChordNaming,
};
use watchord_model::{AppModel, Frame, FrameState, Screen};

/// C6 with two alternates and two notes, settled.
fn c6_model(script: Vec<SoundingSet>) -> AppModel {
    let c6 = SoundingSet::new([60, 64, 67, 69]);
    let naming = StubChordNaming::new();
    naming.stub(
        &c6,
        StubChordNaming::naming(
            &c6,
            "C6",
            "C major 6",
            vec![
                StubAlternate::new("Am7", SpellingOrigin::ReRooted, "A minor 7"),
                StubAlternate::new("CΔ6", SpellingOrigin::Enharmonic, "C major 6, major 7"),
            ],
        ),
    );
    let store = InMemoryNoteStore::new(vec![
        ChordNote::new("1", c6.key(), "C6", "older", UNIX_EPOCH),
        ChordNote::new(
            "2",
            c6.key(),
            "Am7",
            "newer",
            UNIX_EPOCH + Duration::from_secs(60),
        ),
    ]);
    let mut model = AppModel::new(
        Arc::new(naming),
        Box::new(ScriptedSoundingSetSource::new(script)),
        Arc::new(store),
    );
    model.announce("fake");
    model.start();
    while model.wait(Duration::from_millis(50)) {}
    model
}

#[test]
fn the_frame_carries_what_the_screen_shows() {
    let model = c6_model(vec![SoundingSet::new([60, 64, 67, 69])]);
    let frame = model.frame();
    assert_eq!(frame.screen, Screen::NowPlaying);
    assert_eq!(frame.state, FrameState::Held);
    assert_eq!(frame.headline_text, "C6");
    assert_eq!(frame.keys, "C3  E3  G3  A3");
    assert_eq!(frame.key.raw(), "0.4.7.9");
    assert_eq!(frame.sounding.midi_notes(), [60, 64, 67, 69]);
    let headline = frame.headline.as_ref().expect("a headline");
    assert_eq!(headline.origin, SpellingOrigin::Headline);
    assert_eq!(headline.fit, ChordFit::Exact);
    assert_eq!(headline.root, PitchClass::new(0));
    assert_eq!(headline.claimed_row(), "C E G A");
    let names: Vec<&str> = frame.alternates.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["Am7/C", "CΔ6"]);
    assert_eq!(frame.alternates[0].root, PitchClass::new(9));
    assert_eq!(frame.notes.len(), 2);
    assert_eq!(frame.notes[0].text, "newer");
    assert_eq!(frame.groups.len(), 1);
    assert_eq!(frame.notes_total, 2);
    assert!(frame.annotations.is_empty());
    assert_eq!(frame.readings().len(), 3);
}

#[test]
fn released_and_idle_read_on_the_state() {
    let released = c6_model(vec![
        SoundingSet::new([60, 64, 67, 69]),
        SoundingSet::silent(),
    ]);
    assert_eq!(released.frame().state, FrameState::Released);
    assert_eq!(released.frame().headline_text, "C6", "the display holds");

    let idle = c6_model(vec![]);
    let frame = idle.frame();
    assert_eq!(frame.state, FrameState::Idle);
    assert_eq!(frame.headline_text, "—");
    assert!(frame.key.is_empty());
    assert!(frame.note_target_key().is_none());
}

#[test]
fn the_frame_round_trips_through_json_to_an_equal_value() {
    let model = c6_model(vec![SoundingSet::new([60, 64, 67, 69])]);
    let frame = model.frame();
    let line = serde_json::to_string(&frame).expect("serialises");
    assert!(!line.contains('\n'), "one line: {line}");
    let back: Frame = serde_json::from_str(&line).expect("parses back");
    assert_eq!(back, frame);
    // The control: a different frame does not compare equal.
    let mut other = frame.clone();
    other.draft.push('x');
    assert_ne!(other, frame);
}

#[test]
fn the_wire_shape_is_the_value_not_the_struct() {
    let model = c6_model(vec![SoundingSet::new([60, 64, 67, 69])]);
    let line = serde_json::to_string(&model.frame()).expect("serialises");
    assert!(line.contains("\"sounding\":[60,64,67,69]"), "{line}");
    assert!(line.contains("\"key\":\"0.4.7.9\""), "{line}");
    assert!(line.contains("\"origin\":\"reRooted\""), "{line}");
    assert!(line.contains("\"fit\":\"exact\""), "{line}");
    assert!(line.contains("\"annotations\":{}"), "{line}");
    assert!(line.contains("\"screen\":\"nowPlaying\""), "{line}");
}

#[test]
fn a_frame_written_before_annotations_exist_still_reads() {
    let model = c6_model(vec![SoundingSet::new([60, 64, 67, 69])]);
    let frame = model.frame();
    let line = serde_json::to_string(&frame).expect("serialises");
    let without = line.replace("\"annotations\":{},", "");
    assert_ne!(without, line, "the control: the field was there to remove");
    let back: Frame = serde_json::from_str(&without).expect("parses without annotations");
    assert_eq!(back, frame);
}

#[test]
fn every_label_is_on_one_screen_or_the_other() {
    let mut union: Vec<&str> = Frame::NOW_PLAYING_LABELS
        .iter()
        .chain(Frame::ALL_NOTES_LABELS.iter())
        .copied()
        .collect();
    union.sort_unstable();
    union.dedup();
    let mut all: Vec<&str> = Frame::LABELS.to_vec();
    all.sort_unstable();
    assert_eq!(union, all);
}
