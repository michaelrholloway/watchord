//! Shared helpers for the ported `ChordEngineTests`.

#![allow(dead_code)]

use std::collections::{BTreeSet, HashMap};
use std::sync::LazyLock;

use watchord_core::{ChordAnalysis, ChordNaming, ChordReading, PitchClass, SoundingSet};
use watchord_engine::NamingEngine;
use watchord_engine::quality_catalog::{CatalogEntry, QualityCatalog};
use watchord_engine::{
    ChordExtension, ChordSpelling, NoteLetter, NoteSpelling, SeventhQuality, TriadQuality,
};

/// Builds a sounding set with a chosen bass: the bass an octave below, everything
/// else around middle C. The bass has to be a real MIDI note rather than a flag,
/// because it is the strongest term in the ranking and the engine reads it off the
/// lowest sounding key.
pub fn sounding(pitch_classes: &[i32], bass: i32) -> SoundingSet {
    assert!(
        pitch_classes.contains(&bass),
        "the bass has to be one of the sounding keys"
    );
    let mut notes = vec![(48 + bass) as u8];
    notes.extend(
        pitch_classes
            .iter()
            .filter(|&&pc| pc != bass)
            .map(|&pc| (60 + pc) as u8),
    );
    SoundingSet::new(notes)
}

pub fn sounding_set(pitch_classes: &BTreeSet<PitchClass>, bass: PitchClass) -> SoundingSet {
    let pcs: Vec<i32> = pitch_classes.iter().map(|pc| pc.value() as i32).collect();
    sounding(&pcs, bass.value() as i32)
}

/// Headline first, then the alternates — the whole list as the screen shows it.
pub fn all_readings(analysis: &ChordAnalysis) -> Vec<&ChordReading> {
    analysis
        .headline
        .iter()
        .chain(analysis.alternates.iter())
        .collect()
}

pub fn displays(analysis: &ChordAnalysis) -> Vec<String> {
    all_readings(analysis)
        .iter()
        .map(|r| r.display.clone())
        .collect()
}

pub fn analyze(pitch_classes: &[i32], bass: i32) -> ChordAnalysis {
    NamingEngine::new().analyze(&sounding(pitch_classes, bass))
}

pub fn pcs(values: &[i32]) -> BTreeSet<PitchClass> {
    values.iter().map(|&v| PitchClass::new(v)).collect()
}

/// The pitch classes in a mask, ascending.
pub fn pcs_of_mask(mask: u16) -> Vec<i32> {
    (0..12).filter(|pc| mask & (1 << pc) != 0).collect()
}

pub fn mask_of(pitch_classes: &[i32]) -> u16 {
    QualityCatalog::mask(pitch_classes.iter().map(|&v| PitchClass::new(v)))
}

/// Every pitch-class set that clears the naming floor and ceiling, paired with
/// every note that could be underneath it.
pub fn every_chord() -> Vec<(Vec<i32>, i32)> {
    let mut cases = vec![];
    for mask in 0..4096u16 {
        let pcs = pcs_of_mask(mask);
        if !(watchord_core::tuning::MINIMUM_PITCH_CLASSES
            ..=watchord_core::tuning::MAXIMUM_PITCH_CLASSES)
            .contains(&pcs.len())
        {
            continue;
        }
        for &bass in &pcs {
            cases.push((pcs.clone(), bass));
        }
    }
    cases
}

/// The catalog indexed by display, complete spellings preferred, built once.
static BY_DISPLAY: LazyLock<HashMap<String, CatalogEntry>> = LazyLock::new(|| {
    let mut found = HashMap::new();
    for entry in &QualityCatalog::shared().all {
        found.insert(entry.display(), entry.clone());
    }
    found
});

/// The catalog entry a written name came from, by display over the whole catalog.
/// The last entry per display wins, which is the one with the most omitted tones;
/// `complete_only` asks for the full spelling instead.
pub fn entry_named(display: &str, complete_only: bool) -> Option<CatalogEntry> {
    let entry = BY_DISPLAY.get(display)?;
    if complete_only && entry.omitted_tone_count() != 0 {
        return QualityCatalog::shared()
            .all
            .iter()
            .find(|e| e.omitted_tone_count() == 0 && e.display() == display)
            .cloned();
    }
    Some(entry.clone())
}

pub struct WorkedRow {
    pub spelling: ChordSpelling,
    pub display: &'static str,
    pub tones: &'static [&'static str],
    pub spoken: &'static str,
}

fn c_sharp() -> NoteSpelling {
    NoteSpelling::new(NoteLetter::C, 1).expect("C# is writable")
}

pub fn c() -> NoteSpelling {
    NoteSpelling::new(NoteLetter::C, 0).expect("C is writable")
}

fn chord(
    triad: TriadQuality,
    seventh: SeventhQuality,
    extensions: &[ChordExtension],
) -> ChordSpelling {
    ChordSpelling::new(c_sharp(), triad, seventh, extensions)
}

/// The whole of `CONTEXT.md`'s worked table, rooted on C# as Michael wrote it.
pub fn worked_table() -> Vec<WorkedRow> {
    use ChordExtension as X;
    use SeventhQuality as S;
    use TriadQuality as T;
    vec![
        WorkedRow {
            spelling: chord(T::Major, S::None, &[]),
            display: "C#",
            tones: &["C#", "E#", "G#"],
            spoken: "C sharp major",
        },
        WorkedRow {
            spelling: chord(T::Minor, S::None, &[]),
            display: "C#m",
            tones: &["C#", "E", "G#"],
            spoken: "C sharp minor",
        },
        WorkedRow {
            spelling: chord(T::Augmented, S::None, &[]),
            display: "C#+",
            tones: &["C#", "E#", "G##"],
            spoken: "C sharp aug",
        },
        WorkedRow {
            spelling: chord(T::Diminished, S::None, &[]),
            display: "C#-",
            tones: &["C#", "E", "G"],
            spoken: "C sharp dim",
        },
        WorkedRow {
            spelling: chord(T::Major, S::Minor, &[]),
            display: "C#7",
            tones: &["C#", "E#", "G#", "B"],
            spoken: "C sharp major, minor 7",
        },
        WorkedRow {
            spelling: chord(T::Major, S::Major, &[]),
            display: "C#Δ7",
            tones: &["C#", "E#", "G#", "B#"],
            spoken: "C sharp major, major 7",
        },
        WorkedRow {
            spelling: chord(T::Minor, S::Major, &[]),
            display: "C#mΔ7",
            tones: &["C#", "E", "G#", "B#"],
            spoken: "C sharp minor, major 7",
        },
        WorkedRow {
            spelling: chord(T::Major, S::None, &[X::Six, X::AddNine]),
            display: "C#6add9",
            tones: &["C#", "E#", "G#", "A#", "D#"],
            spoken: "C sharp major, major 6, add 9",
        },
        WorkedRow {
            spelling: chord(T::Major, S::Minor, &[X::FlatFive]),
            display: "C#7b5",
            tones: &["C#", "E#", "G", "B"],
            spoken: "C sharp major, minor 7, flat 5",
        },
        WorkedRow {
            spelling: chord(T::Augmented, S::Minor, &[]),
            display: "C#+7",
            tones: &["C#", "E#", "G##", "B"],
            spoken: "C sharp aug, minor 7",
        },
        WorkedRow {
            spelling: chord(T::Diminished, S::Minor, &[]),
            display: "C#-7",
            tones: &["C#", "E", "G", "B"],
            spoken: "C sharp dim, minor 7",
        },
        WorkedRow {
            spelling: chord(T::Diminished, S::Diminished, &[]),
            display: "C#-dim7",
            tones: &["C#", "E", "G", "Bb"],
            spoken: "C sharp dim, dim 7",
        },
    ]
}
