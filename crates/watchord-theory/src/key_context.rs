//! Key context: a tonic and a mode the user sets by hand, or from the soft
//! pedal. Spec #9 "Key context".
//!
//! It relabels readings — a Roman numeral, a Nashville number, a function —
//! and never re-ranks them: nothing here touches a `ChordReading`'s score or
//! its position in the alternates list.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use watchord_core::{NoteName, PitchClass};

/// Major or natural minor — the two modes a key can name (spec #9: "Minor
/// keys use the natural minor scale for diatonic membership").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    Major,
    Minor,
}

impl Mode {
    /// `major` or `minor`, as the KEY plate and `--print` write it.
    pub fn word(self) -> &'static str {
        match self {
            Mode::Major => "major",
            Mode::Minor => "minor",
        }
    }

    /// The other mode.
    pub fn toggled(self) -> Mode {
        match self {
            Mode::Major => Mode::Minor,
            Mode::Minor => Mode::Major,
        }
    }
}

/// `Key { tonic, mode }` (spec #9). Session only — the model never saves it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Key {
    pub tonic: PitchClass,
    pub mode: Mode,
}

impl Key {
    pub fn new(tonic: PitchClass, mode: Mode) -> Self {
        Key { tonic, mode }
    }

    /// `C major`, `F# minor` — sharps only, matching `FrameReading::root_name`.
    pub fn label(&self) -> String {
        format!(
            "{} {}",
            NoteName::pitch_class(self.tonic, false),
            self.mode.word()
        )
    }

    /// The tonic moved up one semitone, mode unchanged. Bound to `k` (design
    /// note: "two keys in the app set it").
    pub fn cycle_tonic(&self) -> Key {
        Key {
            tonic: self.tonic.transposed(1),
            mode: self.mode,
        }
    }

    /// The mode flipped, tonic unchanged. Bound to `m`.
    pub fn toggle_mode(&self) -> Key {
        Key {
            tonic: self.tonic,
            mode: self.mode.toggled(),
        }
    }
}

/// A chord's role against a `Key` (spec #9 "Key context").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Function {
    /// Degrees 1, 3, 6.
    Tonic,
    /// Degrees 2, 4.
    Subdominant,
    /// Degrees 5, 7.
    Dominant,
    /// The root is diatonic; the triad quality is not the diatonic one.
    Borrowed,
    /// The root is not diatonic.
    Chromatic,
}

impl Function {
    /// `tonic`, `subdominant`, `dominant`, `borrowed`, `chromatic`.
    pub fn label(self) -> &'static str {
        match self {
            Function::Tonic => "tonic",
            Function::Subdominant => "subdominant",
            Function::Dominant => "dominant",
            Function::Borrowed => "borrowed",
            Function::Chromatic => "chromatic",
        }
    }
}

/// One reading's numeral, Nashville number, and function against a `Key`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadingKeyContext {
    /// `IΔ7`, `bIIIm7`, `#ivo7` — case carries the reading's own triad
    /// quality; the accidental carries a non-diatonic root; the seventh and
    /// extensions are carried as written, verbatim off the reading's display.
    pub numeral: String,
    /// The same degree as an Arabic numeral, with the same accidental and
    /// suffix: `1Δ7`, `b3m7`.
    pub nashville: String,
    pub function: Function,
}

/// A triad's quality by the intervals actually sounding above its root.
/// Private: the only thing outside this module that needs "major or minor"
/// is `key_from_headline`, and outside that, casing and function decisions —
/// both stay in here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TriadQuality {
    Major,
    Minor,
    Augmented,
    Diminished,
}

/// Reads the triad quality off `root` and `pitch_classes` by interval, never
/// off the written name — the annotation is a fact about what is sounding,
/// not about how the engine happened to spell it.
///
/// A chord with no third sounding (a power chord, a sus chord) has no triad
/// to read this way; it is scored `Major` here as a documented default —
/// spec #9 does not name this case, and every named `Function`/casing rule
/// needs *some* answer to build on.
fn triad_quality(root: PitchClass, pitch_classes: &BTreeSet<PitchClass>) -> TriadQuality {
    let has_major_third = pitch_classes.contains(&root.transposed(4));
    let has_minor_third = pitch_classes.contains(&root.transposed(3));
    let has_perfect_fifth = pitch_classes.contains(&root.transposed(7));
    let has_dim_fifth = pitch_classes.contains(&root.transposed(6));
    let has_aug_fifth = pitch_classes.contains(&root.transposed(8));
    if has_major_third {
        if has_aug_fifth && !has_perfect_fifth {
            TriadQuality::Augmented
        } else {
            TriadQuality::Major
        }
    } else if has_minor_third {
        if has_dim_fifth && !has_perfect_fifth {
            TriadQuality::Diminished
        } else {
            TriadQuality::Minor
        }
    } else {
        TriadQuality::Major
    }
}

/// The tonic and the mode a settled headline sets by itself: the root
/// becomes the tonic, and the triad quality picks the mode. Augmented reads
/// as major, diminished as minor — the same grouping the numeral's case
/// uses (spec #9: "uppercase for major and augmented, lowercase for minor
/// and diminished").
pub fn key_from_headline(root: PitchClass, pitch_classes: &BTreeSet<PitchClass>) -> Key {
    let mode = match triad_quality(root, pitch_classes) {
        TriadQuality::Major | TriadQuality::Augmented => Mode::Major,
        TriadQuality::Minor | TriadQuality::Diminished => Mode::Minor,
    };
    Key::new(root, mode)
}

/// Ascending semitone offsets from the tonic, major.
const MAJOR_SCALE: [u8; 7] = [0, 2, 4, 5, 7, 9, 11];
/// Ascending semitone offsets from the tonic, natural minor.
const NATURAL_MINOR_SCALE: [u8; 7] = [0, 2, 3, 5, 7, 8, 10];

/// The five chromatic semitones a major scale leaves out, each written as
/// one alteration of one degree. Flat-of-the-degree-above throughout, except
/// the tritone gap between IV and V, conventionally the raised fourth
/// (`#IV`, Lydian) rather than `bV`.
const MAJOR_CHROMATIC: [(u8, u8, char); 5] = [
    (1, 2, 'b'),
    (3, 3, 'b'),
    (6, 4, '#'),
    (8, 6, 'b'),
    (10, 7, 'b'),
];

/// The five chromatic semitones a natural minor scale leaves out. Sharp-of-
/// the-degree-below throughout — the scale's own 3rd, 6th, and 7th are
/// already flat, so a further chromatic step is conventionally a raised
/// neighbour (`#III`, `#IV`, `#VI`, `#VII`) — except `bII`, the Neapolitan /
/// Phrygian second, which stays a flat above the tonic.
const MINOR_CHROMATIC: [(u8, u8, char); 5] = [
    (1, 2, 'b'),
    (4, 3, '#'),
    (6, 4, '#'),
    (9, 6, '#'),
    (11, 7, '#'),
];

/// The scale degree (1-7) `offset` semitones above the tonic, and the
/// accidental it needs when it is not one of the scale's own seven notes.
fn degree_info(mode: Mode, offset: u8) -> (u8, Option<char>) {
    let scale = match mode {
        Mode::Major => MAJOR_SCALE,
        Mode::Minor => NATURAL_MINOR_SCALE,
    };
    if let Some(index) = scale.iter().position(|&s| s == offset) {
        return ((index + 1) as u8, None);
    }
    let chromatic = match mode {
        Mode::Major => MAJOR_CHROMATIC,
        Mode::Minor => MINOR_CHROMATIC,
    };
    for (chromatic_offset, degree, accidental) in chromatic {
        if chromatic_offset == offset {
            return (degree, Some(accidental));
        }
    }
    unreachable!(
        "every semitone 0..12 is either a scale degree or one of its five chromatic neighbours"
    )
}

/// The triad quality diatonic membership expects at each degree (1-7 index).
fn expected_quality(mode: Mode, degree: u8) -> TriadQuality {
    use TriadQuality::{Diminished, Major, Minor};
    let table: [TriadQuality; 7] = match mode {
        // I ii iii IV V vi vii°
        Mode::Major => [Major, Minor, Minor, Major, Major, Minor, Diminished],
        // i ii° III iv v VI VII
        Mode::Minor => [Minor, Diminished, Major, Minor, Minor, Major, Major],
    };
    table[(degree - 1) as usize]
}

fn function_of(mode: Mode, degree: u8, accidental: Option<char>, actual: TriadQuality) -> Function {
    if accidental.is_some() {
        return Function::Chromatic;
    }
    if expected_quality(mode, degree) != actual {
        return Function::Borrowed;
    }
    match degree {
        1 | 3 | 6 => Function::Tonic,
        2 | 4 => Function::Subdominant,
        5 | 7 => Function::Dominant,
        _ => unreachable!("degree is always 1..=7"),
    }
}

const ROMAN: [&str; 7] = ["I", "II", "III", "IV", "V", "VI", "VII"];

fn roman_numeral(degree: u8, accidental: Option<char>, quality: TriadQuality) -> String {
    let base = ROMAN[(degree - 1) as usize];
    let cased = match quality {
        TriadQuality::Major | TriadQuality::Augmented => base.to_string(),
        TriadQuality::Minor | TriadQuality::Diminished => base.to_lowercase(),
    };
    match accidental {
        Some(mark) => format!("{mark}{cased}"),
        None => cased,
    }
}

fn nashville_number(degree: u8, accidental: Option<char>) -> String {
    match accidental {
        Some(mark) => format!("{mark}{degree}"),
        None => degree.to_string(),
    }
}

/// How many leading characters of a written chord name are the root: a
/// letter, and — immediately after it, never elsewhere — its accidental.
/// The grammar never writes an alteration token (`b5`, `#5`) directly against
/// a bare root, so this split is never ambiguous (see
/// `watchord_engine::QualityCatalog::extension_sets`'s
/// `root_would_run_into_the_alteration` guard).
fn root_char_len(display: &str) -> usize {
    let mut chars = display.chars();
    if chars.next().is_none() {
        return 0;
    }
    match chars.next() {
        Some('#') | Some('b') => 2,
        _ => 1,
    }
}

/// `display` past its root: `"m7"` off `"Cm7"`, `"Δ7"` off `"C#Δ7"`.
fn suffix_of(display: &str) -> &str {
    let n = root_char_len(display).min(display.len());
    &display[n..]
}

/// One reading's numeral, Nashville number, and function against `key`.
/// `root` and `pitch_classes` are the reading's own; `display` is its
/// written name, whose text past the root — the seventh and every extension
/// — is carried onto both numbers verbatim (spec #9).
pub fn annotate_reading(
    key: Key,
    root: PitchClass,
    pitch_classes: &BTreeSet<PitchClass>,
    display: &str,
) -> ReadingKeyContext {
    let offset = key.tonic.interval_to(root);
    let (degree, accidental) = degree_info(key.mode, offset);
    let quality = triad_quality(root, pitch_classes);
    let suffix = suffix_of(display);
    ReadingKeyContext {
        numeral: format!("{}{suffix}", roman_numeral(degree, accidental, quality)),
        nashville: format!("{}{suffix}", nashville_number(degree, accidental)),
        function: function_of(key.mode, degree, accidental, quality),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pc(v: i32) -> PitchClass {
        PitchClass::new(v)
    }

    fn triad(root: i32, third: i32, fifth: i32) -> BTreeSet<PitchClass> {
        [pc(root), pc(third), pc(fifth)].into_iter().collect()
    }

    // MARK: - triad_quality / key_from_headline

    #[test]
    fn every_triad_quality_and_the_mode_it_picks() {
        assert_eq!(
            key_from_headline(pc(0), &triad(0, 4, 7)),
            Key::new(pc(0), Mode::Major)
        );
        assert_eq!(
            key_from_headline(pc(0), &triad(0, 3, 7)),
            Key::new(pc(0), Mode::Minor)
        );
        // Augmented: major third, sharp fifth, no perfect fifth.
        assert_eq!(
            key_from_headline(pc(0), &triad(0, 4, 8)),
            Key::new(pc(0), Mode::Major)
        );
        // Diminished: minor third, flat fifth, no perfect fifth.
        assert_eq!(
            key_from_headline(pc(0), &triad(0, 3, 6)),
            Key::new(pc(0), Mode::Minor)
        );
    }

    #[test]
    fn a_third_less_chord_defaults_to_major_mode() {
        // A bare power chord: root and fifth only, no third at all.
        let power: BTreeSet<PitchClass> = [pc(0), pc(7)].into_iter().collect();
        assert_eq!(
            key_from_headline(pc(0), &power),
            Key::new(pc(0), Mode::Major)
        );
    }

    // MARK: - degree_info: every diatonic degree, both modes

    #[test]
    fn every_diatonic_degree_in_major_has_no_accidental() {
        for (offset, degree) in MAJOR_SCALE.iter().zip(1u8..=7) {
            assert_eq!(degree_info(Mode::Major, *offset), (degree, None));
        }
    }

    #[test]
    fn every_diatonic_degree_in_minor_has_no_accidental() {
        for (offset, degree) in NATURAL_MINOR_SCALE.iter().zip(1u8..=7) {
            assert_eq!(degree_info(Mode::Minor, *offset), (degree, None));
        }
    }

    #[test]
    fn every_chromatic_offset_in_major_and_minor_is_covered() {
        for (offset, degree, accidental) in MAJOR_CHROMATIC {
            assert_eq!(degree_info(Mode::Major, offset), (degree, Some(accidental)));
        }
        for (offset, degree, accidental) in MINOR_CHROMATIC {
            assert_eq!(degree_info(Mode::Minor, offset), (degree, Some(accidental)));
        }
    }

    // MARK: - annotate_reading: numeral case, Nashville, and extensions "as written"

    #[test]
    fn the_tonic_triad_is_an_uppercase_bare_numeral_in_major() {
        let key = Key::new(pc(0), Mode::Major); // C major
        let out = annotate_reading(key, pc(0), &triad(0, 4, 7), "C");
        assert_eq!(out.numeral, "I");
        assert_eq!(out.nashville, "1");
        assert_eq!(out.function, Function::Tonic);
    }

    #[test]
    fn casing_follows_the_readings_own_quality_not_the_key() {
        // A minor triad written on the tonic of a MAJOR key: same degree (1,
        // no accidental — the root C is diatonic to C major) but the wrong
        // quality, so it is lowercase *and* Borrowed, not Tonic.
        let key = Key::new(pc(0), Mode::Major);
        let out = annotate_reading(key, pc(0), &triad(0, 3, 7), "Cm");
        assert_eq!(out.numeral, "im");
        assert_eq!(out.nashville, "1m");
        assert_eq!(out.function, Function::Borrowed);
    }

    #[test]
    fn the_seventh_and_extensions_are_carried_onto_both_numbers_as_written() {
        let key = Key::new(pc(0), Mode::Major); // C major
        let g7: BTreeSet<PitchClass> = [pc(7), pc(11), pc(2), pc(5)].into_iter().collect();
        let out = annotate_reading(key, pc(7), &g7, "G7");
        assert_eq!(out.numeral, "V7");
        assert_eq!(out.nashville, "57");
        assert_eq!(out.function, Function::Dominant);
    }

    #[test]
    fn a_two_character_root_splits_off_the_flat_correctly() {
        let key = Key::new(pc(1), Mode::Major); // Db major
        // Db major7, written flat: root is diatonic degree 1.
        let db_maj7: BTreeSet<PitchClass> = [pc(1), pc(5), pc(8), pc(0)].into_iter().collect();
        let out = annotate_reading(key, pc(1), &db_maj7, "DbΔ7");
        assert_eq!(out.numeral, "IΔ7");
        assert_eq!(out.nashville, "1Δ7");
    }

    // MARK: - function: every label, both modes

    #[test]
    fn every_function_label_in_major() {
        let key = Key::new(pc(0), Mode::Major); // C major
        // Tonic: I, iii, vi.
        assert_eq!(
            annotate_reading(key, pc(0), &triad(0, 4, 7), "C").function,
            Function::Tonic
        );
        assert_eq!(
            annotate_reading(key, pc(4), &triad(4, 7, 11), "Em").function,
            Function::Tonic
        );
        assert_eq!(
            annotate_reading(key, pc(9), &triad(9, 0, 4), "Am").function,
            Function::Tonic
        );
        // Subdominant: ii, IV.
        assert_eq!(
            annotate_reading(key, pc(2), &triad(2, 5, 9), "Dm").function,
            Function::Subdominant
        );
        assert_eq!(
            annotate_reading(key, pc(5), &triad(5, 9, 0), "F").function,
            Function::Subdominant
        );
        // Dominant: V, vii°.
        assert_eq!(
            annotate_reading(key, pc(7), &triad(7, 11, 2), "G").function,
            Function::Dominant
        );
        assert_eq!(
            annotate_reading(key, pc(11), &triad(11, 2, 5), "Bdim").function,
            Function::Dominant
        );
        // Borrowed: a diatonic root, the wrong quality — D major, not D minor.
        assert_eq!(
            annotate_reading(key, pc(2), &triad(2, 6, 9), "D").function,
            Function::Borrowed
        );
        // Chromatic: a non-diatonic root — Eb major, bIII.
        assert_eq!(
            annotate_reading(key, pc(3), &triad(3, 7, 10), "Eb").function,
            Function::Chromatic
        );
    }

    #[test]
    fn every_function_label_in_minor() {
        let key = Key::new(pc(0), Mode::Minor); // C minor
        // Tonic: i, III, VI.
        assert_eq!(
            annotate_reading(key, pc(0), &triad(0, 3, 7), "Cm").function,
            Function::Tonic
        );
        assert_eq!(
            annotate_reading(key, pc(3), &triad(3, 7, 10), "Eb").function,
            Function::Tonic
        );
        assert_eq!(
            annotate_reading(key, pc(8), &triad(8, 0, 3), "Ab").function,
            Function::Tonic
        );
        // Subdominant: iv, ii°... use iv here (ii° also subdominant-function).
        assert_eq!(
            annotate_reading(key, pc(5), &triad(5, 8, 0), "Fm").function,
            Function::Subdominant
        );
        // Dominant: v, VII.
        assert_eq!(
            annotate_reading(key, pc(7), &triad(7, 10, 2), "Gm").function,
            Function::Dominant
        );
        assert_eq!(
            annotate_reading(key, pc(10), &triad(10, 2, 5), "Bb").function,
            Function::Dominant
        );
        // Borrowed: the tonic root, major quality (a picardy third) instead
        // of the natural-minor-expected minor.
        assert_eq!(
            annotate_reading(key, pc(0), &triad(0, 4, 7), "C").function,
            Function::Borrowed
        );
        // Chromatic: bII, the Neapolitan.
        assert_eq!(
            annotate_reading(key, pc(1), &triad(1, 5, 8), "Db").function,
            Function::Chromatic
        );
    }

    // MARK: - Key

    #[test]
    fn the_key_label_names_the_tonic_with_sharps_and_the_mode() {
        assert_eq!(Key::new(pc(6), Mode::Major).label(), "F# major");
        assert_eq!(Key::new(pc(10), Mode::Minor).label(), "A# minor");
    }

    #[test]
    fn cycle_tonic_and_toggle_mode_change_exactly_one_field() {
        let key = Key::new(pc(0), Mode::Major);
        assert_eq!(key.cycle_tonic(), Key::new(pc(1), Mode::Major));
        assert_eq!(key.toggle_mode(), Key::new(pc(0), Mode::Minor));
    }

    // MARK: - serde round-trip (`Frame` carries `Key`/`Function` through JSON)

    #[test]
    fn key_and_function_round_trip_through_json() {
        let key = Key::new(pc(3), Mode::Minor);
        let json = serde_json::to_string(&key).expect("Key serialises");
        let back: Key = serde_json::from_str(&json).expect("Key parses back");
        assert_eq!(back, key);

        let f = Function::Borrowed;
        let json = serde_json::to_string(&f).expect("Function serialises");
        let back: Function = serde_json::from_str(&json).expect("Function parses back");
        assert_eq!(back, f);
    }
}
