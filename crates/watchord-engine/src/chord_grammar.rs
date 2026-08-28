//! Ported from note-view `Sources/ChordEngine/ChordGrammar.swift`.
//!
//! Ruled live by Michael on 2026-08-10:
//!
//! ```text
//! <root><triad><seventh><extensions>
//!
//! triad:       (none) major   m minor   + augmented   - diminished
//! seventh:     (none) none    7 minor   Δ7 major   dim7 diminished   aug7 augmented
//! extensions:  6  add9  9  11  13  b5  #5  b9  #9  #11  b13
//! ```
//!
//! A quality symbol always describes the **triad**; the seventh is altered by
//! spelling the alteration out as a word. A bare `7` is a *minor* seventh.

use std::collections::BTreeSet;

use watchord_core::PitchClass;

use crate::NoteSpelling;

/// The triad a quality symbol names. Always the triad — never the seventh.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TriadQuality {
    Major,
    Minor,
    Augmented,
    Diminished,
}

impl TriadQuality {
    /// Declaration order.
    pub const ALL_CASES: [TriadQuality; 4] = [
        TriadQuality::Major,
        TriadQuality::Minor,
        TriadQuality::Augmented,
        TriadQuality::Diminished,
    ];

    /// As written after the root: empty, `m`, `+`, `-`.
    pub fn symbol(self) -> &'static str {
        match self {
            TriadQuality::Major => "",
            TriadQuality::Minor => "m",
            TriadQuality::Augmented => "+",
            TriadQuality::Diminished => "-",
        }
    }

    /// How Michael says it: *"c sharp major"*, *"c sharp dim"*.
    pub fn spoken_word(self) -> &'static str {
        match self {
            TriadQuality::Major => "major",
            TriadQuality::Minor => "minor",
            TriadQuality::Augmented => "aug",
            TriadQuality::Diminished => "dim",
        }
    }

    /// Semitones from the root to the third.
    pub fn third_semitones(self) -> i32 {
        match self {
            TriadQuality::Major | TriadQuality::Augmented => 4,
            TriadQuality::Minor | TriadQuality::Diminished => 3,
        }
    }

    /// Semitones from the root to the fifth.
    pub fn fifth_semitones(self) -> i32 {
        match self {
            TriadQuality::Major | TriadQuality::Minor => 7,
            TriadQuality::Augmented => 8,
            TriadQuality::Diminished => 6,
        }
    }
}

/// The seventh, written as its own token so that the quality symbol is free to
/// keep describing the triad.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SeventhQuality {
    None,
    Minor,
    Major,
    Diminished,
    Augmented,
}

impl SeventhQuality {
    /// Declaration order.
    pub const ALL_CASES: [SeventhQuality; 5] = [
        SeventhQuality::None,
        SeventhQuality::Minor,
        SeventhQuality::Major,
        SeventhQuality::Diminished,
        SeventhQuality::Augmented,
    ];

    /// As written after the triad: empty, `7`, `Δ7`, `dim7`, `aug7`.
    pub fn symbol(self) -> &'static str {
        match self {
            SeventhQuality::None => "",
            SeventhQuality::Minor => "7",
            SeventhQuality::Major => "Δ7",
            SeventhQuality::Diminished => "dim7",
            SeventhQuality::Augmented => "aug7",
        }
    }

    pub fn spoken_phrase(self) -> Option<&'static str> {
        match self {
            SeventhQuality::None => None,
            SeventhQuality::Minor => Some("minor 7"),
            SeventhQuality::Major => Some("major 7"),
            SeventhQuality::Diminished => Some("dim 7"),
            SeventhQuality::Augmented => Some("aug 7"),
        }
    }

    /// Semitones above the root, or `None` for no seventh.
    ///
    /// Note `Augmented` is **twelve** — a major seventh raised a semitone is the
    /// octave. The grammar can write `Caug7`, but the chord it describes names the
    /// root twice, so nothing can ever spell it; see `ChordSpelling::spelled`.
    pub fn semitones(self) -> Option<i32> {
        match self {
            SeventhQuality::None => None,
            SeventhQuality::Minor => Some(10),
            SeventhQuality::Major => Some(11),
            SeventhQuality::Diminished => Some(9),
            SeventhQuality::Augmented => Some(12),
        }
    }
}

/// One note a spelling claims: where it is written relative to the root, where it
/// sounds, and whether a real chord may leave it out and still be that chord.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChordDegree {
    /// Letters above the root. The third is two letters up, the fifth four, the
    /// seventh six, the ninth one (the second letter, an octave up).
    pub letters: i32,
    /// Semitones above the root. May exceed twelve; the octave is folded out when
    /// the pitch class is computed.
    pub semitones: i32,
    /// Whether a chord missing this note is still honestly named by this spelling.
    ///
    /// **The root, the third and the seventh are never omissible**, while the
    /// fifth carries no information unless it has been altered, and the lower
    /// rungs of an extended stack are routinely left out. The *top* rung is never
    /// omissible.
    pub omissible: bool,
}

impl ChordDegree {
    /// A degree that may not be dropped.
    pub const fn new(letters: i32, semitones: i32) -> Self {
        ChordDegree {
            letters,
            semitones,
            omissible: false,
        }
    }

    /// A degree a player may drop and still be playing this chord.
    pub const fn omissible(letters: i32, semitones: i32) -> Self {
        ChordDegree {
            letters,
            semitones,
            omissible: true,
        }
    }

    /// The degree as a musician says it — 1, 3, 5, 7, 9, 11, 13 — which is what
    /// the `·no5` mark is written from. A degree that sits at or above the octave
    /// is named above it — `C6`'s A is a sixth (9 semitones), `C13`'s A is a
    /// thirteenth (21).
    pub fn number(&self) -> u8 {
        (if self.semitones >= 12 {
            self.letters + 8
        } else {
            self.letters + 1
        }) as u8
    }
}

/// An extension or alteration. Declaration order **is** the order they are written
/// in, so `C#6add9` can never come out as `C#add96`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ChordExtension {
    Five,
    Six,
    AddTwo,
    AddFour,
    AddNine,
    Nine,
    Eleven,
    Thirteen,
    Sus4,
    Sus2,
    FlatFive,
    SharpFive,
    FlatNine,
    SharpNine,
    SharpEleven,
    FlatThirteen,
}

/// What a token does to the chord. The four cases are genuinely different
/// operations and collapsing any two of them would produce a wrong chord.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Effect {
    /// Adds these notes. A `13` adds three of them.
    AddsDegrees(Vec<ChordDegree>),
    /// Moves the triad's fifth by this many semitones.
    AltersFifth(i32),
    /// Puts this note where the third was.
    ReplacesThird(ChordDegree),
    /// Takes the third out and adds nothing. Root and fifth are all that is left.
    OmitsThird,
}

impl ChordExtension {
    /// Written order. `ChordSpelling::new` sorts extensions into it.
    pub const ALL_CASES: [ChordExtension; 16] = [
        ChordExtension::Five,
        ChordExtension::Six,
        ChordExtension::AddTwo,
        ChordExtension::AddFour,
        ChordExtension::AddNine,
        ChordExtension::Nine,
        ChordExtension::Eleven,
        ChordExtension::Thirteen,
        ChordExtension::Sus4,
        ChordExtension::Sus2,
        ChordExtension::FlatFive,
        ChordExtension::SharpFive,
        ChordExtension::FlatNine,
        ChordExtension::SharpNine,
        ChordExtension::SharpEleven,
        ChordExtension::FlatThirteen,
    ];

    /// As written after the seventh.
    pub fn symbol(self) -> &'static str {
        match self {
            ChordExtension::Five => "5",
            ChordExtension::Six => "6",
            ChordExtension::AddTwo => "add2",
            ChordExtension::AddFour => "add4",
            ChordExtension::AddNine => "add9",
            ChordExtension::Nine => "9",
            ChordExtension::Eleven => "11",
            ChordExtension::Thirteen => "13",
            ChordExtension::Sus4 => "sus4",
            ChordExtension::Sus2 => "sus2",
            ChordExtension::FlatFive => "b5",
            ChordExtension::SharpFive => "#5",
            ChordExtension::FlatNine => "b9",
            ChordExtension::SharpNine => "#9",
            ChordExtension::SharpEleven => "#11",
            ChordExtension::FlatThirteen => "b13",
        }
    }

    /// As said aloud.
    pub fn spoken_phrase(self) -> &'static str {
        match self {
            ChordExtension::Five => "no 3",
            ChordExtension::Six => "major 6",
            ChordExtension::AddTwo => "add 2",
            ChordExtension::AddFour => "add 4",
            ChordExtension::AddNine => "add 9",
            ChordExtension::Nine => "9",
            ChordExtension::Eleven => "11",
            ChordExtension::Thirteen => "13",
            ChordExtension::Sus4 => "sus 4",
            ChordExtension::Sus2 => "sus 2",
            ChordExtension::FlatFive => "flat 5",
            ChordExtension::SharpFive => "sharp 5",
            ChordExtension::FlatNine => "flat 9",
            ChordExtension::SharpNine => "sharp 9",
            ChordExtension::SharpEleven => "sharp 11",
            ChordExtension::FlatThirteen => "flat 13",
        }
    }

    /// Whether this token stands for a **stack** — `9`, `11`, `13` — which implies
    /// the seventh beneath it and therefore swallows the seventh's own symbol.
    pub fn implies_the_seventh(self) -> bool {
        matches!(
            self,
            ChordExtension::Nine | ChordExtension::Eleven | ChordExtension::Thirteen
        )
    }

    /// Whether this token differs from another spelling of the **same pitch
    /// classes** only in which octave the added note sits in. `Cadd2` and `Cadd9`
    /// are both `C E G D`; the token marked here loses the synonym collapse.
    pub fn names_an_octave(self) -> bool {
        matches!(self, ChordExtension::AddTwo | ChordExtension::AddFour)
    }

    /// What this token does to the chord.
    pub fn effect(self) -> Effect {
        match self {
            // The power chord. Michael named it `C5 (no 3)`.
            ChordExtension::Five => Effect::OmitsThird,

            ChordExtension::Six => Effect::AddsDegrees(vec![ChordDegree::new(5, 9)]),

            // `add2` and `add4` name the same pitch classes as `add9` and `add11`.
            // The interval is written as it is *said* — within the octave.
            ChordExtension::AddTwo => Effect::AddsDegrees(vec![ChordDegree::new(1, 2)]),
            ChordExtension::AddFour => Effect::AddsDegrees(vec![ChordDegree::new(3, 5)]),

            ChordExtension::AddNine => Effect::AddsDegrees(vec![ChordDegree::new(1, 14)]),

            // The stack. Lower rungs are omissible, the top rung never is.
            ChordExtension::Nine => Effect::AddsDegrees(vec![ChordDegree::new(1, 14)]),
            ChordExtension::Eleven => {
                Effect::AddsDegrees(vec![ChordDegree::omissible(1, 14), ChordDegree::new(3, 17)])
            }
            ChordExtension::Thirteen => Effect::AddsDegrees(vec![
                ChordDegree::omissible(1, 14),
                ChordDegree::omissible(3, 17),
                ChordDegree::new(5, 21),
            ]),

            ChordExtension::Sus4 => Effect::ReplacesThird(ChordDegree::new(3, 5)),
            ChordExtension::Sus2 => Effect::ReplacesThird(ChordDegree::new(1, 2)),

            ChordExtension::FlatFive => Effect::AltersFifth(-1),
            ChordExtension::SharpFive => Effect::AltersFifth(1),
            ChordExtension::FlatNine => Effect::AddsDegrees(vec![ChordDegree::new(1, 13)]),
            ChordExtension::SharpNine => Effect::AddsDegrees(vec![ChordDegree::new(1, 15)]),
            ChordExtension::SharpEleven => Effect::AddsDegrees(vec![ChordDegree::new(3, 18)]),
            ChordExtension::FlatThirteen => Effect::AddsDegrees(vec![ChordDegree::new(5, 20)]),
        }
    }
}

/// One chord as the grammar writes it: a root, a triad, a seventh, extensions.
///
/// A `ChordSpelling` is a *claim*, not a fact. Nothing here has been checked
/// against anything sounding — `spelled()` turns the claim into the notes it
/// implies, and only then can it be compared with what is actually being played.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChordSpelling {
    /// The written root.
    pub root: NoteSpelling,
    /// The triad the quality symbol names.
    pub triad: TriadQuality,
    pub seventh: SeventhQuality,
    /// Always in `ChordExtension::ALL_CASES` order, whatever order they arrived in.
    pub extensions: Vec<ChordExtension>,
}

impl ChordSpelling {
    /// Builds a spelling; `extensions` are sorted into written order.
    pub fn new(
        root: NoteSpelling,
        triad: TriadQuality,
        seventh: SeventhQuality,
        extensions: &[ChordExtension],
    ) -> Self {
        let supplied: BTreeSet<ChordExtension> = extensions.iter().copied().collect();
        ChordSpelling {
            root,
            triad,
            seventh,
            extensions: ChordExtension::ALL_CASES
                .iter()
                .copied()
                .filter(|e| supplied.contains(e))
                .collect(),
        }
    }

    /// True when a `9`, `11` or `13` is carrying the seventh for us.
    pub fn seventh_is_implied(&self) -> bool {
        self.extensions.iter().any(|e| e.implies_the_seventh())
    }

    /// True when a token has taken the third out altogether — today only `5`.
    /// It is the condition under which the **triad symbol stops meaning anything**.
    pub fn omits_the_third(&self) -> bool {
        self.extensions
            .iter()
            .any(|e| e.effect() == Effect::OmitsThird)
    }

    /// One line of plain text: `C#-dim7`, `C#6add9`, `C13`.
    ///
    /// The seventh's symbol disappears in front of a `9`, `11` or `13`. A **major**
    /// seventh keeps its `Δ` and loses only the `7`, so a major ninth is `CΔ9`.
    pub fn display(&self) -> String {
        let mut s = self.root.display();
        s.push_str(self.triad.symbol());
        s.push_str(self.written_seventh_symbol());
        for e in &self.extensions {
            s.push_str(e.symbol());
        }
        s
    }

    fn written_seventh_symbol(&self) -> &'static str {
        if !self.seventh_is_implied() {
            return self.seventh.symbol();
        }
        if self.seventh == SeventhQuality::Major {
            "Δ"
        } else {
            ""
        }
    }

    /// The same chord said aloud: `C#-dim7` → "C sharp dim, dim 7".
    ///
    /// The seventh is **spoken even when it is not written** — `C13` says "C major,
    /// minor 7, 13". A chord with no third is spoken under the token that removed
    /// it — "C 5, no 3" and never "C major, no 3".
    pub fn spoken(&self) -> String {
        let quality = self
            .extensions
            .iter()
            .find(|e| e.effect() == Effect::OmitsThird)
            .map(|e| e.symbol())
            .unwrap_or(self.triad.spoken_word());
        let mut parts = vec![format!("{} {}", self.root.spoken(), quality)];
        if let Some(seventh) = self.seventh.spoken_phrase() {
            parts.push(seventh.to_string());
        }
        parts.extend(
            self.extensions
                .iter()
                .map(|e| e.spoken_phrase().to_string()),
        );
        parts.join(", ")
    }

    /// The notes this spelling implies, computed forward from the grammar.
    ///
    /// This function is the honesty rule. Every candidate the engine considers is
    /// produced by *writing a name and working out what it would sound like* — the
    /// reverse direction, matching a name onto a set of keys, is never performed.
    ///
    /// Returns `None` when the spelling is not a real chord: **unwritable** (a
    /// degree landing more than two semitones from its letter) or **degenerate**
    /// (two degrees landing on the same key — `Caug7` is `C E G B#`, and `B#` *is*
    /// `C`).
    pub fn spelled(&self) -> Option<SpelledChord> {
        let mut degrees: Vec<ChordDegree> = vec![ChordDegree::new(0, 0)];

        // The third — unless something is suspended in its place, or a `5` has taken
        // it out and put nothing there.
        let suspension = self.extensions.iter().find_map(|e| match e.effect() {
            Effect::ReplacesThird(degree) => Some(degree),
            _ => None,
        });
        if let Some(suspended) = suspension {
            degrees.push(suspended);
        } else if !self.omits_the_third() {
            degrees.push(ChordDegree::new(2, self.triad.third_semitones()));
        }

        // The fifth. Omissible only when it is **perfect** and a seventh is giving
        // the chord its identity elsewhere. Read "perfect" as the arithmetic, not as
        // the triad: a diminished triad's fifth is as load-bearing as a `b5`.
        let mut fifth = self.triad.fifth_semitones();
        for e in &self.extensions {
            if let Effect::AltersFifth(by) = e.effect() {
                fifth += by;
            }
        }
        let fifth_is_perfect = fifth == 7;
        degrees.push(ChordDegree {
            letters: 4,
            semitones: fifth,
            omissible: self.seventh != SeventhQuality::None && fifth_is_perfect,
        });

        if let Some(seventh_semitones) = self.seventh.semitones() {
            degrees.push(ChordDegree::new(6, seventh_semitones));
        }

        // The added notes, in two passes, because a stack yields to everything else.
        // A `9`, `11` or `13` claims the whole stack beneath it, but the tones it
        // claims are *implied* rather than written, and where a written token names
        // the same letter, the written one is the note that sounds (`C13sus4`,
        // `C13b9`).
        let mut stacked: Vec<ChordDegree> = vec![];
        let mut written: Vec<ChordDegree> = vec![];
        for e in &self.extensions {
            let Effect::AddsDegrees(added) = e.effect() else {
                continue;
            };
            if e.implies_the_seventh() {
                stacked.extend(added);
            } else {
                written.extend(added);
            }
        }
        degrees.extend(written);
        let spoken_for: BTreeSet<i32> = degrees.iter().map(|d| d.letters).collect();
        degrees.extend(
            stacked
                .into_iter()
                .filter(|d| !spoken_for.contains(&d.letters)),
        );

        let mut tones: Vec<SpelledTone> = vec![];
        let mut sounded: BTreeSet<PitchClass> = BTreeSet::new();
        for degree in degrees {
            let letter = self.root.letter.advanced(degree.letters);
            let target = PitchClass::new(self.root.pitch_class().value() as i32 + degree.semitones);
            let distance = (target.value() as i32 - letter.natural_pitch_class()).rem_euclid(12);
            let alteration = if distance > 6 {
                distance - 12
            } else {
                distance
            };
            let note = NoteSpelling::new(letter, alteration)?;
            if !sounded.insert(note.pitch_class()) {
                return None;
            }
            tones.push(SpelledTone { note, degree });
        }

        Some(SpelledChord {
            spelling: self.clone(),
            tones,
            pitch_classes: sounded,
        })
    }
}

/// One written note of a chord, and where in the spelling it came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SpelledTone {
    /// The note as written.
    pub note: NoteSpelling,
    pub degree: ChordDegree,
}

impl SpelledTone {
    /// Whether a chord missing this note is still honestly named by this spelling.
    pub fn omissible(&self) -> bool {
        self.degree.omissible
    }
}

/// A spelling together with the notes it implies — the result of computing forward
/// from the grammar.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SpelledChord {
    pub spelling: ChordSpelling,
    /// Root, third (or the note suspended in its place), fifth, seventh, then the
    /// extensions in written order.
    pub tones: Vec<SpelledTone>,
    /// Every key this spelling can account for. Nothing sounding may fall outside
    /// this set — that half of the honesty rule never bends.
    pub pitch_classes: BTreeSet<PitchClass>,
}

impl SpelledChord {
    /// True when any tone is written `##` or `bb`.
    pub fn uses_double_accidental(&self) -> bool {
        self.tones.iter().any(|t| t.note.is_double_accidental())
    }

    /// The notes a player may drop.
    pub fn omissible_pitch_classes(&self) -> Vec<PitchClass> {
        self.tones
            .iter()
            .filter(|t| t.omissible())
            .map(|t| t.note.pitch_class())
            .collect()
    }
}
