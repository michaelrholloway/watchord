//! Ported case for case from note-view `Tests/NoteViewCoreTests/SoundingSetTests.swift`.

use watchord_core::{PitchClass, SoundingSet};

#[test]
fn stores_distinct_notes_ascending_whatever_order_they_arrived_in() {
    let set = SoundingSet::new([67, 60, 64, 60, 67]);
    assert_eq!(set.midi_notes(), &[60, 64, 67]);
}

#[test]
fn bass_is_the_lowest_sounding_note() {
    assert_eq!(SoundingSet::new([67, 60, 64]).bass(), Some(60));
    assert_eq!(SoundingSet::new([67, 72, 76]).bass(), Some(67));
    assert_eq!(SoundingSet::silent().bass(), None);
}

#[test]
fn bass_pitch_class_is_the_bass_notes_class_not_the_lowest_class() {
    // A first-inversion C major sounds E in the bass. The lowest *pitch class*
    // present is C (0), but the bass is E (4).
    let first_inversion_c = SoundingSet::new([64, 67, 72]);
    assert_eq!(first_inversion_c.bass_pitch_class(), Some(PitchClass::new(4)));
    assert_eq!(
        first_inversion_c.pitch_classes().iter().next().copied(),
        Some(PitchClass::new(0))
    );
}

#[test]
fn counts_distinct_pitch_classes_not_keys() {
    let doubled_triad = SoundingSet::new([48, 52, 55, 60, 64, 67, 72]);
    assert_eq!(doubled_triad.midi_notes().len(), 7);
    assert_eq!(doubled_triad.distinct_pitch_class_count(), 3);
}

#[test]
fn silence_is_empty_and_keys_to_the_empty_chord_key() {
    assert!(SoundingSet::silent().is_empty());
    assert_eq!(SoundingSet::silent().distinct_pitch_class_count(), 0);
    assert!(SoundingSet::silent().key().is_empty());
}

#[test]
fn equal_sets_are_equal_regardless_of_the_order_they_were_built_in() {
    assert_eq!(SoundingSet::new([60, 64, 67]), SoundingSet::new([67, 64, 60]));
    assert_ne!(SoundingSet::new([60, 64, 67]), SoundingSet::new([60, 64, 67, 72]));
}
