//! Ported case for case from note-view `Tests/MIDIInputTests/HeldNoteTrackerTests.swift`.
//!
//! The state machine, exhaustively, with no device. 60 = middle C, 64 = E, 67 = G.

use watchord_core::tuning;
use watchord_core::{ChordKey, PitchClass, SoundingSet};
use watchord_midi::{HeldNoteTracker, MidiEvent};

/// Feeds events and returns the sounding note numbers.
fn play(events: &[MidiEvent]) -> Vec<u8> {
    let mut tracker = HeldNoteTracker::new();
    tracker.apply_all(events.iter().copied());
    tracker.sounding().midi_notes().to_vec()
}

fn on(note: u8) -> MidiEvent {
    on_v(note, 100)
}
fn on_v(note: u8, velocity: u8) -> MidiEvent {
    MidiEvent::NoteOn {
        note,
        velocity,
        channel: 0,
    }
}
fn on_ch(note: u8, channel: u8) -> MidiEvent {
    MidiEvent::NoteOn {
        note,
        velocity: 100,
        channel,
    }
}
fn off(note: u8) -> MidiEvent {
    MidiEvent::NoteOff { note, channel: 0 }
}
fn off_ch(note: u8, channel: u8) -> MidiEvent {
    MidiEvent::NoteOff { note, channel }
}
fn pedal(value: u8) -> MidiEvent {
    pedal_ch(value, 0)
}
fn pedal_ch(value: u8, channel: u8) -> MidiEvent {
    MidiEvent::ControlChange {
        controller: 64,
        value,
        channel,
    }
}
fn mod_wheel(value: u8) -> MidiEvent {
    MidiEvent::ControlChange {
        controller: 1,
        value,
        channel: 0,
    }
}
fn sostenuto(value: u8) -> MidiEvent {
    MidiEvent::ControlChange {
        controller: 66,
        value,
        channel: 0,
    }
}
fn soft(value: u8) -> MidiEvent {
    MidiEvent::ControlChange {
        controller: 67,
        value,
        channel: 0,
    }
}

// MARK: - The basics

#[test]
fn a_fresh_tracker_is_silent() {
    let tracker = HeldNoteTracker::new();
    assert_eq!(tracker.sounding(), SoundingSet::silent());
    assert!(tracker.sounding().is_empty());
    assert!(!tracker.is_sustain_down());
}

#[test]
fn note_on_adds_note_off_removes() {
    assert_eq!(play(&[on(60)]), [60]);
    assert_eq!(play(&[on(60), off(60)]), []);
}

#[test]
fn a_chord_holds_every_note_ascending() {
    assert_eq!(play(&[on(67), on(60), on(64)]), [60, 64, 67]);
}

#[test]
fn releasing_one_note_of_a_chord_leaves_the_rest() {
    assert_eq!(play(&[on(60), on(64), on(67), off(64)]), [60, 67]);
}

// MARK: - Velocity 0 (the stuck-note bug)

#[test]
fn note_on_with_velocity_0_releases() {
    assert_eq!(play(&[on(60), on_v(60, 0)]), []);
}

#[test]
fn a_whole_chord_released_by_velocity_0_note_ons_leaves_nothing_held() {
    assert_eq!(
        play(&[
            on(60),
            on(64),
            on(67),
            on_v(60, 0),
            on_v(64, 0),
            on_v(67, 0)
        ]),
        []
    );
}

#[test]
fn velocity_1_is_a_real_note_on_not_a_release() {
    assert_eq!(play(&[on_v(60, 1)]), [60]);
}

#[test]
fn a_velocity_0_note_on_under_the_pedal_sustains_exactly_as_a_note_off_would() {
    assert_eq!(play(&[pedal(127), on(60), on_v(60, 0)]), [60]);
    assert_eq!(play(&[pedal(127), on(60), on_v(60, 0), pedal(0)]), []);
}

// MARK: - Sustain

#[test]
fn pedal_down_holds_a_note_past_its_note_off() {
    assert_eq!(play(&[pedal(127), on(60), off(60)]), [60]);
}

#[test]
fn pedal_lift_releases_what_was_pending() {
    assert_eq!(play(&[pedal(127), on(60), off(60), pedal(0)]), []);
}

#[test]
fn pedal_lift_releases_several_pending_notes_in_one_go() {
    let mut tracker = HeldNoteTracker::new();
    tracker.apply_all([
        pedal(127),
        on(60),
        on(64),
        on(67),
        off(60),
        off(64),
        off(67),
    ]);
    assert_eq!(tracker.sounding().midi_notes(), [60, 64, 67]);

    let changed = tracker.apply(pedal(0));
    assert!(changed);
    assert!(tracker.sounding().is_empty());
}

#[test]
fn sustain_held_across_a_chord_change_accumulates_both_chords() {
    let events = [
        pedal(127),
        on(60),
        on(64),
        on(67),
        off(60),
        off(64),
        off(67),
        on(65),
        on(69),
        on(72),
    ];
    assert_eq!(play(&events), [60, 64, 65, 67, 69, 72]);
}

#[test]
fn lifting_the_pedal_mid_chord_keeps_the_keys_still_under_a_finger() {
    let events = [
        pedal(127),
        on(60),
        on(64),
        on(67),
        off(60),
        off(64),
        pedal(0),
    ];
    assert_eq!(play(&events), [67]);
}

#[test]
fn re_striking_a_sustaining_note_takes_it_back_under_the_finger() {
    let events = [pedal(127), on(60), off(60), on(60), pedal(0)];
    assert_eq!(play(&events), [60]);
}

#[test]
fn pressing_the_pedal_with_nothing_held_changes_nothing() {
    let mut tracker = HeldNoteTracker::new();
    let changed = tracker.apply(pedal(127));
    assert!(!changed);
    assert!(tracker.sounding().is_empty());
    assert!(tracker.is_sustain_down());
}

#[test]
fn a_note_pressed_after_the_pedal_is_already_down_still_sustains() {
    assert_eq!(play(&[pedal(127), on(60), off(60)]), [60]);
}

#[test]
fn a_note_off_arriving_before_the_pedal_goes_down_is_a_plain_release() {
    assert_eq!(play(&[on(60), off(60), pedal(127)]), []);
}

// MARK: - The sustain threshold

#[test]
fn cc64_counts_as_down_at_the_threshold_and_above() {
    for value in [64u8, 65, 100, 127] {
        assert_eq!(
            play(&[pedal(value), on(60), off(60)]),
            [60],
            "value {value}"
        );
    }
}

#[test]
fn cc64_counts_as_up_below_the_threshold() {
    for value in [0u8, 1, 32, 63] {
        assert_eq!(play(&[pedal(value), on(60), off(60)]), [], "value {value}");
    }
}

#[test]
fn the_threshold_is_tunings_not_a_literal_in_this_crate() {
    let just_under = tuning::SUSTAIN_ON_THRESHOLD - 1;
    assert_eq!(
        play(&[pedal(tuning::SUSTAIN_ON_THRESHOLD), on(60), off(60)]),
        [60]
    );
    assert_eq!(play(&[pedal(just_under), on(60), off(60)]), []);
}

#[test]
fn a_sweep_through_the_threshold_releases_at_the_crossing_not_before() {
    let mut tracker = HeldNoteTracker::new();
    tracker.apply_all([pedal(127), on(60), off(60)]);
    assert_eq!(tracker.sounding().midi_notes(), [60]);

    let still_down_at_100 = tracker.apply(pedal(100));
    let still_down_at_64 = tracker.apply(pedal(64));
    assert!(!still_down_at_100);
    assert!(!still_down_at_64);
    assert_eq!(tracker.sounding().midi_notes(), [60]);

    let released_at_63 = tracker.apply(pedal(63));
    assert!(released_at_63);
    assert!(tracker.sounding().is_empty());
}

#[test]
fn a_control_change_that_is_not_cc64_is_ignored() {
    assert_eq!(play(&[mod_wheel(127), on(60), off(60)]), []);
}

// MARK: - Overlapping, repeated and out-of-order events

#[test]
fn a_repeated_note_on_for_a_key_already_down_is_not_a_second_note() {
    let mut tracker = HeldNoteTracker::new();
    let first_strike = tracker.apply(on(60));
    let second_strike = tracker.apply(on_v(60, 90));
    assert!(first_strike);
    assert!(!second_strike);
    assert_eq!(tracker.sounding().midi_notes(), [60]);

    let release = tracker.apply(off(60));
    assert!(release);
    assert!(tracker.sounding().is_empty());
}

#[test]
fn a_note_off_for_a_key_that_was_never_down_changes_nothing() {
    let mut tracker = HeldNoteTracker::new();
    let changed = tracker.apply(off(60));
    assert!(!changed);
    assert!(tracker.sounding().is_empty());
}

#[test]
fn a_note_off_for_a_key_never_struck_does_not_conjure_a_note_even_under_the_pedal() {
    let mut tracker = HeldNoteTracker::new();
    let changed = tracker.apply_all([pedal(127), off(60), off(64)]);
    assert!(!changed);
    assert!(tracker.sounding().is_empty());
    assert!(tracker.is_sustain_down());
}

#[test]
fn a_note_off_cannot_raise_a_damper_on_a_key_still_under_a_finger() {
    assert_eq!(play(&[pedal(127), on(60), off(60), off(60)]), [60]);
}

#[test]
fn a_trill_leaves_the_key_in_whichever_state_the_last_event_put_it() {
    assert_eq!(play(&[on(60), off(60), on(60), off(60), on(60)]), [60]);
    assert_eq!(play(&[on(60), off(60), on(60), off(60)]), []);
}

#[test]
fn octave_doubling_is_two_held_notes_but_one_pitch_class() {
    let mut tracker = HeldNoteTracker::new();
    tracker.apply_all([on(60), on(72)]);
    assert_eq!(tracker.sounding().midi_notes(), [60, 72]);
    assert_eq!(tracker.sounding().distinct_pitch_class_count(), 1);
}

#[test]
fn the_full_128_note_range_is_representable() {
    assert_eq!(play(&[on(0), on(127)]), [0, 127]);
}

// MARK: - Channels

#[test]
fn a_note_off_on_a_different_channel_still_releases_the_note() {
    assert_eq!(play(&[on_ch(60, 0), off_ch(60, 9)]), []);
}

#[test]
fn sustain_applies_across_channels() {
    assert_eq!(play(&[pedal_ch(127, 5), on_ch(60, 0), off_ch(60, 0)]), [60]);
}

// MARK: - Change reporting

#[test]
fn apply_reports_a_change_only_when_the_sounding_set_actually_differs() {
    let mut tracker = HeldNoteTracker::new();
    let strike = tracker.apply(on(60));
    let restrike = tracker.apply(on(60));
    let wheel = tracker.apply(mod_wheel(64));
    let release = tracker.apply(off(60));
    let second_release = tracker.apply(off(60));
    assert!(strike);
    assert!(!restrike);
    assert!(!wheel);
    assert!(release);
    assert!(!second_release);
}

#[test]
fn a_batch_that_nets_out_to_no_change_reports_no_change() {
    let mut tracker = HeldNoteTracker::new();
    tracker.apply(on(60));
    let changed = tracker.apply_all([on(64), off(64)]);
    assert!(!changed);
    assert_eq!(tracker.sounding().midi_notes(), [60]);
}

#[test]
fn a_batch_that_does_change_the_set_reports_it() {
    let mut tracker = HeldNoteTracker::new();
    let changed = tracker.apply_all([on(60), on(64), off(64)]);
    assert!(changed);
    assert_eq!(tracker.sounding().midi_notes(), [60]);
}

#[test]
fn an_empty_batch_reports_no_change() {
    let mut tracker = HeldNoteTracker::new();
    assert!(!tracker.apply_all([]));
}

// MARK: - Reset

#[test]
fn reset_drops_held_notes_sustained_notes_and_the_pedal() {
    let mut tracker = HeldNoteTracker::new();
    tracker.apply_all([pedal(127), on(60), on(64), off(60)]);
    assert_eq!(tracker.sounding().midi_notes(), [60, 64]);

    tracker.reset();
    assert!(tracker.sounding().is_empty());
    assert!(!tracker.is_sustain_down());
    let changed = tracker.apply_all([on(67), off(67)]);
    assert!(!changed);
    assert!(tracker.sounding().is_empty());
}

// MARK: - Sostenuto and soft (ticket 13)

#[test]
fn sostenuto_and_soft_read_independently_of_sustain_and_of_each_other() {
    let mut tracker = HeldNoteTracker::new();
    assert!(!tracker.is_sostenuto_down());
    assert!(!tracker.is_soft_down());

    tracker.apply_all([sostenuto(127)]);
    assert!(tracker.is_sostenuto_down());
    assert!(!tracker.is_sustain_down());
    assert!(!tracker.is_soft_down());

    tracker.apply_all([soft(127)]);
    assert!(tracker.is_soft_down());
    assert!(tracker.is_sostenuto_down(), "soft must not clear sostenuto");

    tracker.apply_all([sostenuto(0)]);
    assert!(!tracker.is_sostenuto_down());
    assert!(tracker.is_soft_down(), "sostenuto lifting must not clear soft");

    tracker.apply_all([soft(0)]);
    assert!(!tracker.is_soft_down());
}

#[test]
fn sostenuto_and_soft_use_the_same_threshold_as_sustain() {
    let mut tracker = HeldNoteTracker::new();
    tracker.apply_all([sostenuto(tuning::SUSTAIN_ON_THRESHOLD - 1)]);
    assert!(!tracker.is_sostenuto_down(), "one under the threshold is up");
    tracker.apply_all([sostenuto(tuning::SUSTAIN_ON_THRESHOLD)]);
    assert!(tracker.is_sostenuto_down(), "at the threshold is down");

    tracker.apply_all([soft(tuning::SUSTAIN_ON_THRESHOLD - 1)]);
    assert!(!tracker.is_soft_down());
    tracker.apply_all([soft(tuning::SUSTAIN_ON_THRESHOLD)]);
    assert!(tracker.is_soft_down());
}

#[test]
fn sostenuto_and_soft_never_change_what_is_sounding() {
    let mut tracker = HeldNoteTracker::new();
    tracker.apply_all([on(60), on(64)]);
    let before = tracker.sounding();
    let sounding_changed = tracker.apply_all([sostenuto(127), soft(127), sostenuto(0), soft(0)]);
    assert!(!sounding_changed);
    assert_eq!(tracker.sounding(), before);
}

#[test]
fn reset_also_drops_sostenuto_and_soft() {
    let mut tracker = HeldNoteTracker::new();
    tracker.apply_all([sostenuto(127), soft(127)]);
    tracker.reset();
    assert!(!tracker.is_sostenuto_down());
    assert!(!tracker.is_soft_down());
}

// MARK: - Sounding set shape

#[test]
fn the_sounding_set_the_tracker_produces_is_what_the_engine_expects() {
    let mut tracker = HeldNoteTracker::new();
    tracker.apply_all([on(67), on(52), on(60)]);
    let sounding = tracker.sounding();
    assert_eq!(sounding.midi_notes(), [52, 60, 67]);
    assert_eq!(sounding.bass(), Some(52));
    assert_eq!(sounding.bass_pitch_class(), Some(PitchClass::new(4)));
    assert_eq!(
        sounding.key(),
        ChordKey::new([PitchClass::new(0), PitchClass::new(4), PitchClass::new(7)])
    );
}
