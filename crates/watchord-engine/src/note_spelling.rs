//! Ported from note-view `Sources/ChordEngine/NoteSpelling.swift`.

use watchord_core::PitchClass;

/// A letter name, `A` through `G`.
///
/// Stored in scale order **from C** rather than alphabetically, because every
/// interval in the grammar is defined by counting letters: the third is two
/// letters up, the fifth is four, the seventh is six. Counting in this order makes
/// that plain modular addition, and it is what lets the engine work out that the
/// augmented fifth of `C#` is spelled `G##` rather than `A`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NoteLetter {
    C = 0,
    D,
    E,
    F,
    G,
    A,
    B,
}

impl NoteLetter {
    /// Ascending from C — the order the discriminant counts in.
    pub const SCALE_ORDER: [NoteLetter; 7] = [
        NoteLetter::C,
        NoteLetter::D,
        NoteLetter::E,
        NoteLetter::F,
        NoteLetter::G,
        NoteLetter::A,
        NoteLetter::B,
    ];

    /// The letter as written: `"C"`.
    pub fn character(self) -> &'static str {
        match self {
            NoteLetter::C => "C",
            NoteLetter::D => "D",
            NoteLetter::E => "E",
            NoteLetter::F => "F",
            NoteLetter::G => "G",
            NoteLetter::A => "A",
            NoteLetter::B => "B",
        }
    }

    /// The pitch class this letter sounds with no accidental on it.
    pub fn natural_pitch_class(self) -> i32 {
        match self {
            NoteLetter::C => 0,
            NoteLetter::D => 2,
            NoteLetter::E => 4,
            NoteLetter::F => 5,
            NoteLetter::G => 7,
            NoteLetter::A => 9,
            NoteLetter::B => 11,
        }
    }

    /// The letter `count` steps up the alphabet, wrapping — the third of `C` is
    /// `NoteLetter::C.advanced(2)`, which is `E`.
    pub fn advanced(self, count: i32) -> NoteLetter {
        NoteLetter::SCALE_ORDER[(self as i32 + count).rem_euclid(7) as usize]
    }
}

/// A written note: a letter plus an alteration in semitones.
///
/// This is the *output* side of the spelling-blind problem. MIDI hands the engine
/// a `PitchClass`, which cannot distinguish `C#` from `Db`; a `NoteSpelling` is one
/// of the ways that pitch class can be written down, and the engine's whole job is
/// to generate them and rank them rather than to read one off the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NoteSpelling {
    /// The letter name.
    pub letter: NoteLetter,
    /// Semitones away from the natural letter: `-2` double flat through `+2`
    /// double sharp.
    pub alteration: i32,
}

impl NoteSpelling {
    /// What western notation can write. A degree landing outside this range — a
    /// triple sharp — has no spelling, and the candidate carrying it is discarded
    /// rather than rounded to something writable.
    pub const WRITABLE_ALTERATIONS: std::ops::RangeInclusive<i32> = -2..=2;

    /// `None` when the alteration is unwritable. Failing here rather than clamping
    /// is deliberate: a clamped tone would be a *different note*.
    pub fn new(letter: NoteLetter, alteration: i32) -> Option<Self> {
        if !Self::WRITABLE_ALTERATIONS.contains(&alteration) {
            return None;
        }
        Some(NoteSpelling { letter, alteration })
    }

    /// The pitch class this spelling sounds.
    pub fn pitch_class(&self) -> PitchClass {
        PitchClass::new(self.letter.natural_pitch_class() + self.alteration)
    }

    /// `C`, `C#`, `Db`, `G##`, `Bbb`.
    pub fn display(&self) -> String {
        format!(
            "{}{}",
            self.letter.character(),
            Self::accidental_symbol(self.alteration)
        )
    }

    /// `C sharp`, `B double flat`.
    pub fn spoken(&self) -> String {
        format!(
            "{}{}",
            self.letter.character(),
            Self::spoken_accidental(self.alteration)
        )
    }

    /// True for `##` and `bb`.
    pub fn is_double_accidental(&self) -> bool {
        self.alteration.abs() == 2
    }

    /// `bb`, `b`, empty, `#`, `##` for `-2..=2`.
    pub fn accidental_symbol(alteration: i32) -> &'static str {
        match alteration {
            -2 => "bb",
            -1 => "b",
            1 => "#",
            2 => "##",
            _ => "",
        }
    }

    /// The accidental said aloud, with its leading space: `" sharp"`.
    pub fn spoken_accidental(alteration: i32) -> &'static str {
        match alteration {
            -2 => " double flat",
            -1 => " flat",
            1 => " sharp",
            2 => " double sharp",
            _ => "",
        }
    }

    /// True when some letter sounds this pitch class with no accidental on it —
    /// that is, when the pitch already has a plain name.
    pub fn pitch_has_a_natural_name(&self) -> bool {
        let pc = self.pitch_class().value() as i32;
        NoteLetter::SCALE_ORDER
            .iter()
            .any(|l| l.natural_pitch_class() == pc)
    }

    /// Every root the app will write a chord on.
    ///
    /// Spec §2 gives the root a letter and an optional `#` or `b`, which is
    /// twenty-one spellings. Four of them are struck out here: **`B#`, `Cb`, `E#`
    /// and `Fb`** — the ones that rename a pitch that already has a plain name.
    /// What survives is the seven naturals plus the five genuine two-name pitches.
    /// Both members of every pair are kept, always. Do not "restore" the four:
    /// ranking cannot substitute for the filter, because `NATURAL_ROOT` is worth
    /// three points against a quality commonness of eighteen.
    pub fn root_spellings() -> Vec<NoteSpelling> {
        let mut roots: Vec<NoteSpelling> = NoteLetter::SCALE_ORDER
            .iter()
            .flat_map(|&letter| {
                [-1, 0, 1]
                    .into_iter()
                    .filter_map(move |alt| NoteSpelling::new(letter, alt))
            })
            .filter(|s| s.alteration == 0 || !s.pitch_has_a_natural_name())
            .collect();
        roots.sort_by_key(|s| s.display());
        roots
    }
}
