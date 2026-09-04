//! Ported case for case from note-view `Tests/NoteViewCoreTests/ChordKeyTests.swift`.

use std::time::SystemTime;

use watchord_core::{ChordKey, ChordNote, PitchClass, SoundingSet};

fn pcs(values: &[i32]) -> Vec<PitchClass> {
    values.iter().map(|&v| PitchClass::new(v)).collect()
}

#[test]
fn canonicalises_to_ascending_deduplicated_pitch_classes() {
    assert_eq!(ChordKey::new(pcs(&[9, 0, 4, 7])).raw(), "0.4.7.9");
    assert_eq!(ChordKey::new(pcs(&[0, 0, 4])).raw(), "0.4");
}

#[test]
fn is_blind_to_octave_inversion_and_doubling() {
    let root_position = SoundingSet::new([60, 64, 67, 69]);
    let second_inversion = SoundingSet::new([67, 69, 72, 76]);
    let spread_and_doubled = SoundingSet::new([36, 52, 60, 64, 79, 81, 84]);

    assert_eq!(root_position.key(), second_inversion.key());
    assert_eq!(root_position.key(), spread_and_doubled.key());
    assert_eq!(root_position.key().raw(), "0.4.7.9");
}

#[test]
fn transposition_is_a_different_identity() {
    assert_ne!(
        SoundingSet::new([60, 64, 67]).key(),
        SoundingSet::new([62, 66, 69]).key()
    );
}

#[test]
fn round_trips_through_raw() {
    let original = ChordKey::new(pcs(&[0, 4, 7]));
    let parsed = ChordKey::parse(original.raw());
    assert_eq!(parsed.as_ref(), Some(&original));
    assert_eq!(parsed.unwrap().pitch_classes(), original.pitch_classes());
}

#[test]
fn rejects_every_non_canonical_form() {
    for raw in [
        "",         // empty — `parse` rejects it on purpose; see its doc comment
        "0.4.7.",   // trailing separator
        "0,4,7",    // wrong separator
        "0.4.12",   // out of range
        "0.4.-1",   // negative
        "7.4.0",    // not ascending
        "0.4.4",    // duplicate
        "0.four.7", // not a number
        "abc",
    ] {
        assert_eq!(ChordKey::parse(raw), None, "{raw:?} should be rejected");
    }
}

#[test]
fn the_empty_key_is_silence_and_holds_nothing() {
    assert!(ChordKey::empty().is_empty());
    assert!(ChordKey::empty().pitch_classes().is_empty());
    assert_eq!(SoundingSet::silent().key(), ChordKey::empty());
}

#[test]
fn survives_a_codable_round_trip() {
    let key = ChordKey::new(pcs(&[1, 5, 8]));
    let data = serde_json::to_string(&key).unwrap();
    assert_eq!(serde_json::from_str::<ChordKey>(&data).unwrap(), key);
}

// ChordKey JSON shape

#[test]
fn encodes_as_a_bare_string_not_a_wrapped_object() {
    let json = serde_json::to_string(&ChordKey::parse("0.4.7").unwrap()).unwrap();
    assert_eq!(json, "\"0.4.7\"");
}

#[test]
fn decoding_validates_a_corrupt_key_throws() {
    for bad in ["\"7.4.0\"", "\"0.4.12\"", "\"0.4.7.\"", "\"\"", "\"abc\""] {
        assert!(
            serde_json::from_str::<ChordKey>(bad).is_err(),
            "{bad} should fail to decode"
        );
    }
}

/// `deserialize_key_or_empty` is the one sanctioned exception: a field that
/// legitimately means "nothing displayed" opts into it explicitly, and every
/// other `ChordKey` field keeps the strict `Deserialize` tested above.
#[derive(serde::Deserialize)]
struct KeyOrEmptyField {
    #[serde(deserialize_with = "watchord_core::deserialize_key_or_empty")]
    key: ChordKey,
}

#[test]
fn deserialize_key_or_empty_reads_the_empty_string_as_the_empty_key() {
    let parsed: KeyOrEmptyField = serde_json::from_str("{\"key\":\"\"}").unwrap();
    assert_eq!(parsed.key, ChordKey::empty());
}

#[test]
fn deserialize_key_or_empty_still_validates_a_real_key() {
    let parsed: KeyOrEmptyField = serde_json::from_str("{\"key\":\"0.4.7\"}").unwrap();
    assert_eq!(parsed.key, ChordKey::parse("0.4.7").unwrap());
    // The control: it still rejects a corrupt non-empty key.
    let bad: Result<KeyOrEmptyField, _> = serde_json::from_str("{\"key\":\"7.4.0\"}");
    assert!(bad.is_err());
}

#[test]
fn a_good_key_still_round_trips() {
    let key = ChordKey::parse("1.5.8").unwrap();
    let json = serde_json::to_string(&key).unwrap();
    assert_eq!(serde_json::from_str::<ChordKey>(&json).unwrap(), key);
}

#[test]
fn chord_note_carries_the_key_as_a_bare_string() {
    // The Swift test encodes a whole ChordNote; the notes-file encoding is the
    // store crate's (ADR-0004). What this crate can promise is the key's shape
    // inside any document that carries it.
    let note = ChordNote::new(
        "E6D2A0C4-0000-4000-8000-000000000000",
        ChordKey::parse("0.4.7.9").unwrap(),
        "C6",
        "like the Rhodes on Voodoo",
        SystemTime::UNIX_EPOCH,
    );
    let json = serde_json::to_string(&note.chord_key).unwrap();
    assert_eq!(json, "\"0.4.7.9\"");
    assert!(!json.contains("raw"));
}
