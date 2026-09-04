//! Ported from note-view `Tests/MIDIInputTests/SoundingSetRelayTests.swift`.
//!
//! The settle window on instants the test owns. Every case asserts both
//! halves: that nothing is due *before* the interval elapses, and that the
//! right thing is emitted after. The first half is the one that matters — it is
//! what stops a four-note chord flickering through three wrong names.

use std::time::{Duration, Instant};

use watchord_core::tuning;
use watchord_core::{ControlEvent, PedalKind, SoundingSet};
use watchord_midi::{MidiEvent, SettleRelay};

const SETTLE: Duration = Duration::from_millis(60);

fn on(note: u8) -> MidiEvent {
    MidiEvent::NoteOn {
        note,
        velocity: 100,
        channel: 0,
    }
}
fn off(note: u8) -> MidiEvent {
    MidiEvent::NoteOff { note, channel: 0 }
}
fn pedal(value: u8) -> MidiEvent {
    cc(64, value)
}
fn cc(controller: u8, value: u8) -> MidiEvent {
    MidiEvent::ControlChange {
        controller,
        value,
        channel: 0,
    }
}
fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

/// A relay plus the clock it is driven with.
struct Harness {
    relay: SettleRelay,
    now: Instant,
    emitted: Vec<SoundingSet>,
}

impl Harness {
    fn new() -> Self {
        Self::with_settle(SETTLE)
    }

    fn with_settle(settle: Duration) -> Self {
        Harness {
            relay: SettleRelay::new(settle),
            now: Instant::now(),
            emitted: Vec::new(),
        }
    }

    fn receive(&mut self, events: &[MidiEvent]) {
        self.relay.receive(events.iter().copied(), self.now);
    }

    /// Moves time forward and fires whatever is due, as the relay thread does.
    fn advance(&mut self, by: Duration) {
        self.now += by;
        if let Some(sounding) = self.relay.fire(self.now) {
            self.emitted.push(sounding);
        }
    }
}

#[test]
fn nothing_is_emitted_until_the_settle_interval_has_fully_elapsed() {
    let mut h = Harness::new();
    h.receive(&[on(60)]);
    assert!(h.relay.due_at().is_some(), "a settle was scheduled");

    h.advance(ms(59));
    assert!(h.emitted.is_empty());

    h.advance(ms(1));
    assert_eq!(h.emitted, [SoundingSet::new([60])]);
}

#[test]
fn a_chord_struck_across_several_events_is_named_once_not_once_per_note() {
    let mut h = Harness::new();

    h.receive(&[on(60)]);
    h.advance(ms(20));
    h.receive(&[on(64)]);
    h.advance(ms(20));
    h.receive(&[on(67)]);
    h.advance(ms(20));

    // 60 ms of wall time has passed, but never 60 ms without a change.
    assert!(h.emitted.is_empty());

    h.advance(ms(40));
    assert_eq!(h.emitted, [SoundingSet::new([60, 64, 67])]);
}

#[test]
fn a_chord_arriving_in_one_batch_settles_once() {
    let mut h = Harness::new();
    h.receive(&[on(60), on(64), on(67)]);
    h.advance(SETTLE);
    h.advance(SETTLE);

    assert_eq!(h.emitted, [SoundingSet::new([60, 64, 67])]);
    assert!(h.relay.due_at().is_none());
}

#[test]
fn the_empty_set_is_emitted_on_full_release() {
    let mut h = Harness::new();
    h.receive(&[on(60), on(64)]);
    h.advance(SETTLE);

    h.receive(&[off(60), off(64)]);
    h.advance(SETTLE);

    assert_eq!(h.emitted.len(), 2);
    assert_eq!(h.emitted.last(), Some(&SoundingSet::silent()));
}

#[test]
fn an_event_that_does_not_change_the_sounding_set_does_not_restart_the_timer() {
    let mut h = Harness::new();
    h.receive(&[on(60)]);
    h.advance(ms(50));

    // A repeated note-on for a key already down, and a mod-wheel move.
    h.receive(&[
        on(60),
        MidiEvent::ControlChange {
            controller: 1,
            value: 90,
            channel: 0,
        },
    ]);
    h.advance(ms(10));

    assert_eq!(h.emitted, [SoundingSet::new([60])]);
}

#[test]
fn the_emitted_set_is_the_freshest_one_not_the_one_that_scheduled_the_timer() {
    let mut h = Harness::new();
    h.receive(&[on(60)]);
    h.receive(&[on(64)]);
    h.advance(SETTLE);

    assert_eq!(h.emitted, [SoundingSet::new([60, 64])]);
}

#[test]
fn sustain_is_honoured_through_the_settled_stream_not_only_in_the_tracker() {
    let mut h = Harness::new();
    h.receive(&[pedal(127)]);
    h.receive(&[on(60), on(64), on(67)]);
    h.advance(SETTLE);
    assert_eq!(h.emitted.len(), 1);

    // Hands off. The pedal is down, so nothing changes and nothing is emitted.
    h.receive(&[off(60), off(64), off(67)]);
    h.advance(Duration::from_secs(1));
    assert_eq!(h.emitted.len(), 1);

    // Pedal lifts: everything releases at once, as one emission.
    h.receive(&[pedal(0)]);
    h.advance(SETTLE);

    assert_eq!(
        h.emitted,
        [SoundingSet::new([60, 64, 67]), SoundingSet::silent()]
    );
}

#[test]
fn the_settle_interval_defaults_to_tunings_not_to_a_literal_in_this_crate() {
    let mut h = Harness::with_settle(tuning::SETTLE_INTERVAL);
    let default = SettleRelay::default();
    h.relay = default;

    h.receive(&[on(60)]);
    h.advance(tuning::SETTLE_INTERVAL - ms(1));
    assert!(h.emitted.is_empty());

    h.advance(ms(1));
    assert_eq!(h.emitted, [SoundingSet::new([60])]);
}

#[test]
fn shut_down_cancels_a_pending_emission_and_forgets_what_was_held() {
    let mut h = Harness::new();
    h.receive(&[on(60), on(64)]);
    assert!(h.relay.due_at().is_some());

    h.relay.shut_down();
    h.advance(Duration::from_secs(1));
    assert!(h.emitted.is_empty());
    assert!(h.relay.currently_sounding().is_empty());
}

#[test]
fn the_relay_keeps_working_after_shut_down() {
    let mut h = Harness::new();
    h.receive(&[on(60)]);
    h.relay.shut_down();

    h.receive(&[on(67)]);
    h.advance(SETTLE);

    // 60 was forgotten by the reset, so only 67 is sounding.
    assert_eq!(h.emitted, [SoundingSet::new([67])]);
}

// MARK: - Control events (ticket 13)

#[test]
fn cc64_cc66_and_cc67_each_emit_a_control_event_on_and_off() {
    let mut relay = SettleRelay::new(SETTLE);
    let now = Instant::now();

    let on_events = relay.receive([cc(64, 127), cc(66, 127), cc(67, 127)], now);
    assert_eq!(
        on_events,
        [
            ControlEvent {
                pedal: PedalKind::Sustain,
                down: true
            },
            ControlEvent {
                pedal: PedalKind::Sostenuto,
                down: true
            },
            ControlEvent {
                pedal: PedalKind::Soft,
                down: true
            },
        ]
    );

    let off_events = relay.receive([cc(64, 0), cc(66, 0), cc(67, 0)], now);
    assert_eq!(
        off_events,
        [
            ControlEvent {
                pedal: PedalKind::Sustain,
                down: false
            },
            ControlEvent {
                pedal: PedalKind::Sostenuto,
                down: false
            },
            ControlEvent {
                pedal: PedalKind::Soft,
                down: false
            },
        ]
    );
}

#[test]
fn a_control_event_that_does_not_change_a_pedal_emits_nothing() {
    let mut relay = SettleRelay::new(SETTLE);
    let now = Instant::now();
    relay.receive([cc(64, 127)], now);
    // The control: repeating the same value emits nothing further.
    assert_eq!(relay.receive([cc(64, 127)], now), []);
    // A note event alone — no pedal in the batch — also emits nothing.
    assert_eq!(relay.receive([on(60)], now), []);
}

#[test]
fn pedal_changes_are_not_debounced_by_the_settle_window() {
    let mut h = Harness::new();
    let events = h.relay.receive([cc(64, 127)], h.now);
    assert_eq!(
        events.len(),
        1,
        "reported on the same call, not after settle"
    );
}

#[test]
fn an_empty_batch_of_events_emits_nothing() {
    let mut h = Harness::new();
    h.receive(&[]);
    assert!(h.relay.due_at().is_none());
    h.advance(Duration::from_secs(1));
    assert!(h.emitted.is_empty());
}
