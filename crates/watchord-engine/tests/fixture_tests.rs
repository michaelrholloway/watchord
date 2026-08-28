//! The engine-parity gate (ADR-0002): the Swift engine's answer for every
//! pitch-class set, committed as `tests/fixtures/engine-fixture.json`, must match
//! the Rust engine row for row.
//!
//! The strings compared are what the screen shows — the slash-bass name, the
//! `·no5 ·no11` detail, the `≈` flag, the spoken name — produced here by the
//! same `ReadingDisplay` the model uses. A failing row prints the set, the Swift
//! answer, and the Rust answer.

use std::path::PathBuf;

use serde::Deserialize;
use watchord_core::{ChordFit, ChordNaming, ChordReading, PitchClass, ReadingDisplay, SoundingSet};
use watchord_engine::NamingEngine;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fixture {
    generated_from: String,
    schema: u32,
    sets: Vec<Row>,
    voiced: Vec<Row>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Row {
    pitch_classes: Vec<u8>,
    midi_notes: Vec<u8>,
    name: Option<String>,
    headline: Headline,
    alternates: Vec<Alternate>,
}

/// The engine's rank-1 reading, or the declined state. Every optional field is
/// absent when the Swift value was nil.
#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Headline {
    text: String,
    spoken: Option<String>,
    fit_tier: Option<String>,
    fit_note: Option<String>,
    approximation: bool,
    decline_reason: Option<String>,
    root: Option<u8>,
    score: Option<i32>,
    claims: Option<Vec<u8>>,
    missing_degrees: Option<Vec<u8>>,
    extra_pitch_classes: Option<Vec<u8>>,
    distance: Option<i32>,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Alternate {
    text: String,
    axis: String,
    spoken: String,
    fit_tier: String,
    fit_note: Option<String>,
    approximation: bool,
    root: u8,
    score: i32,
    claims: Vec<u8>,
    missing_degrees: Option<Vec<u8>>,
    extra_pitch_classes: Option<Vec<u8>>,
    distance: Option<i32>,
}

/// The committed fixture, or the file named by `WATCHORD_FIXTURE` — the override
/// exists so a mutated copy can prove the comparison fails on a changed row.
fn fixture_path() -> PathBuf {
    match std::env::var_os("WATCHORD_FIXTURE") {
        Some(path) => PathBuf::from(path),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/engine-fixture.json"),
    }
}

fn load() -> Fixture {
    let path = fixture_path();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("fixture {} does not parse: {e}", path.display()))
}

fn pcs(values: &[PitchClass]) -> Vec<u8> {
    values.iter().map(|pc| pc.value()).collect()
}

/// The Rust engine's headline for a row, in the fixture's own shape.
fn rust_headline(reading: &ChordReading, bass: Option<PitchClass>) -> Headline {
    let display = ReadingDisplay::new(reading, bass);
    let (missing_degrees, extra_pitch_classes, distance) = match &reading.fit {
        ChordFit::Exact => (None, None, None),
        ChordFit::Missing(degrees) => (Some(degrees.clone()), None, None),
        ChordFit::Plus(extras) => (None, Some(pcs(extras)), None),
        ChordFit::Nearest { distance } => (None, None, Some(*distance)),
    };
    let approximation = display.is_approximate();
    Headline {
        text: display.name,
        spoken: Some(display.spoken),
        fit_tier: Some(reading.fit.tier_name().to_string()),
        fit_note: display.fit_detail,
        approximation,
        decline_reason: None,
        root: Some(reading.root.value()),
        score: Some(reading.score),
        claims: Some(pcs(&reading
            .pitch_classes
            .iter()
            .copied()
            .collect::<Vec<_>>())),
        missing_degrees,
        extra_pitch_classes,
        distance,
    }
}

fn rust_alternate(reading: &ChordReading, bass: Option<PitchClass>) -> Alternate {
    let h = rust_headline(reading, bass);
    Alternate {
        text: h.text,
        axis: reading.origin.raw_value().to_string(),
        spoken: h.spoken.unwrap_or_default(),
        fit_tier: h.fit_tier.unwrap_or_default(),
        fit_note: h.fit_note,
        approximation: h.approximation,
        root: reading.root.value(),
        score: reading.score,
        claims: h.claims.unwrap_or_default(),
        missing_degrees: h.missing_degrees,
        extra_pitch_classes: h.extra_pitch_classes,
        distance: h.distance,
    }
}

/// Compares one row; returns a description of the first difference, if any.
fn compare(engine: &NamingEngine, row: &Row) -> Option<String> {
    let sounding = SoundingSet::new(row.midi_notes.iter().copied());
    assert_eq!(
        pcs(&sounding.pitch_classes().iter().copied().collect::<Vec<_>>()),
        row.pitch_classes,
        "fixture row's midiNotes and pitchClasses disagree"
    );
    let analysis = engine.analyze(&sounding);
    let bass = sounding.bass_pitch_class();
    let label = match &row.name {
        Some(name) => format!("{name} {:?}", row.midi_notes),
        None => format!("{:?}", row.pitch_classes),
    };

    let rust = match &analysis.headline {
        Some(headline) => rust_headline(headline, bass),
        None => Headline {
            text: watchord_core::NoteName::pitch_classes_row(&sounding),
            spoken: None,
            fit_tier: None,
            fit_note: None,
            approximation: false,
            decline_reason: analysis.declined_reason.map(|r| r.raw_value().to_string()),
            root: None,
            score: None,
            claims: None,
            missing_degrees: None,
            extra_pitch_classes: None,
            distance: None,
        },
    };
    if rust != row.headline {
        return Some(format!(
            "set {label}\n  headline\n    swift: {:?}\n    rust:  {rust:?}",
            row.headline
        ));
    }

    let rust_alternates: Vec<Alternate> = analysis
        .alternates
        .iter()
        .map(|r| rust_alternate(r, bass))
        .collect();
    if rust_alternates.len() != row.alternates.len() {
        return Some(format!(
            "set {label}\n  alternates count\n    swift: {} {:?}\n    rust:  {} {:?}",
            row.alternates.len(),
            row.alternates.iter().map(|a| &a.text).collect::<Vec<_>>(),
            rust_alternates.len(),
            rust_alternates.iter().map(|a| &a.text).collect::<Vec<_>>()
        ));
    }
    for (rank, (swift, rust)) in row.alternates.iter().zip(&rust_alternates).enumerate() {
        if swift != rust {
            return Some(format!(
                "set {label}\n  alternate {}\n    swift: {swift:?}\n    rust:  {rust:?}",
                rank + 1
            ));
        }
    }
    None
}

fn failures(rows: &[Row]) -> Vec<String> {
    let engine = NamingEngine::new();
    rows.iter()
        .filter_map(|row| compare(&engine, row))
        .collect()
}

#[test]
fn fixture_is_schema_1_from_a_named_note_view_commit() {
    let fixture = load();
    assert_eq!(fixture.schema, 1);
    assert_eq!(
        fixture.generated_from.len(),
        40,
        "generatedFrom is a full sha"
    );
}

#[test]
fn every_one_of_the_4095_sets_matches_the_swift_engine() {
    let fixture = load();
    assert_eq!(
        fixture.sets.len(),
        4095,
        "the fixture must hold every non-empty set"
    );
    let failures = failures(&fixture.sets);
    assert!(
        failures.is_empty(),
        "{} of 4095 sets differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn every_voiced_row_matches_the_swift_engine() {
    let fixture = load();
    assert!(
        fixture.voiced.len() >= 60,
        "the voiced table holds {} rows",
        fixture.voiced.len()
    );
    let failures = failures(&fixture.voiced);
    assert!(
        failures.is_empty(),
        "{} of {} voiced rows differ:\n{}",
        failures.len(),
        fixture.voiced.len(),
        failures.join("\n")
    );
}
