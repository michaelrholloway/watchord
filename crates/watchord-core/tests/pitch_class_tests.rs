//! Ported case for case from note-view `Tests/NoteViewCoreTests/PitchClassTests.swift`.

use std::collections::BTreeSet;

use watchord_core::PitchClass;

#[test]
fn wraps_into_0_to_12_in_both_directions() {
    assert_eq!(PitchClass::new(0).value(), 0);
    assert_eq!(PitchClass::new(12).value(), 0);
    assert_eq!(PitchClass::new(13).value(), 1);
    assert_eq!(PitchClass::new(-1).value(), 11);
    assert_eq!(PitchClass::new(-12).value(), 0);
    assert_eq!(PitchClass::new(-13).value(), 11);
}

#[test]
fn middle_c_is_pitch_class_0() {
    assert_eq!(PitchClass::from_midi_note(60).value(), 0);
    assert_eq!(PitchClass::from_midi_note(61).value(), 1);
    assert_eq!(PitchClass::from_midi_note(72).value(), 0);
    assert_eq!(PitchClass::from_midi_note(0).value(), 0);
    assert_eq!(PitchClass::from_midi_note(127).value(), 7);
}

#[test]
fn interval_is_measured_upward_and_always_lands_in_0_to_12() {
    let (c, e, a) = (PitchClass::new(0), PitchClass::new(4), PitchClass::new(9));
    assert_eq!(c.interval_to(e), 4);
    // Downward by ear is still upward by definition — E up to C is 8, not -4.
    assert_eq!(e.interval_to(c), 8);
    assert_eq!(a.interval_to(c), 3);
    assert_eq!(c.interval_to(c), 0);
}

#[test]
fn transposition_wraps_rather_than_escaping_the_octave() {
    assert_eq!(PitchClass::new(10).transposed(5).value(), 3);
    assert_eq!(PitchClass::new(2).transposed(-5).value(), 9);
    assert_eq!(PitchClass::new(7).transposed(12).value(), 7);
}

#[test]
fn enharmonics_collapse() {
    // MIDI 61 is one pitch class, not two, and no amount of context recovers
    // which letter was meant.
    assert_eq!(PitchClass::from_midi_note(61), PitchClass::new(1));
    let set: BTreeSet<PitchClass> = [PitchClass::new(1), PitchClass::new(1)].into_iter().collect();
    assert_eq!(set.len(), 1);
}

#[test]
fn all_cases_is_the_twelve_ascending_distinct() {
    let all = PitchClass::all_cases();
    assert_eq!(all.len(), 12);
    let set: BTreeSet<PitchClass> = all.iter().copied().collect();
    assert_eq!(set.len(), 12);
    assert!(all.is_sorted());
}
