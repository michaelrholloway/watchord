//! Ported from note-view `Sources/ChordEngine/QualityCatalog.swift`.
//!
//! Every chord the grammar can write, indexed by the keys it sounds.
//!
//! The catalog is built **once**, forward from the grammar: write a name, work out
//! what it would sound like, keep it under that fingerprint. Naming a chord is then
//! a lookup of the keys that are actually sounding, which is why the answer is exact
//! by construction. There is no path in this file that starts from a set of keys
//! and reasons towards a name.
//!
//! A second table, `readings_by_mask`, files the **closest** spellings under
//! fingerprints that no spelling produces. Those candidates were written forward
//! from the grammar like every other, and the table is built by *measuring* each
//! of them against the keys.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, LazyLock};

use watchord_core::PitchClass;
use watchord_core::tuning;

use crate::chord_ranking::ChordFitting;
use crate::{
    ChordExtension, ChordSpelling, NoteSpelling, SeventhQuality, SpelledChord, TriadQuality,
};

/// The twelve pitch classes as a bit per key. Used instead of a set as a map key
/// so that nothing downstream can inherit a hash-seeded ordering.
pub type Mask = u16;

/// One quality the grammar can write, independent of any root: a triad, a
/// seventh, its extensions, and the name it is scored under in
/// `tuning::quality_commonness`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct QualityTemplate {
    /// The triad.
    pub triad: TriadQuality,
    pub seventh: SeventhQuality,
    pub extensions: Vec<ChordExtension>,
    /// The key into `tuning::quality_commonness`.
    pub commonness_key: &'static str,
}

/// A quality written on a specific root, filed under one particular set of keys
/// it can honestly name.
///
/// One spelling produces several entries when it has omissible notes: `C13` is
/// filed under the complete seven-note chord, and again under the four- and
/// five-note voicings a player's hands actually produce. Each entry knows how
/// many notes were left out, because that is what the ranking charges for.
#[derive(Clone, Debug)]
pub struct CatalogEntry {
    /// The spelling and the notes it implies; shared between its voicings.
    pub chord: Arc<SpelledChord>,
    /// Exactly the keys this entry names. Always a subset of `chord.pitch_classes`.
    pub pitch_classes: BTreeSet<PitchClass>,
    /// The key into `tuning::quality_commonness`.
    pub commonness_key: &'static str,
}

impl CatalogEntry {
    /// The spelling as written.
    pub fn display(&self) -> String {
        self.chord.spelling.display()
    }

    /// The written root.
    pub fn root(&self) -> NoteSpelling {
        self.chord.spelling.root
    }

    /// How many extension tokens the spelling carries.
    pub fn extension_count(&self) -> usize {
        self.chord.spelling.extensions.len()
    }

    /// How many of the spelling's notes are not being played.
    pub fn omitted_tone_count(&self) -> usize {
        self.chord.pitch_classes.len() - self.pitch_classes.len()
    }

    /// The keys this entry names, as a fingerprint.
    pub fn mask(&self) -> Mask {
        QualityCatalog::mask(self.pitch_classes.iter().copied())
    }
}

/// A base pair: triad, seventh, and the commonness key they are scored under.
type BasePair = (TriadQuality, SeventhQuality, &'static str);

/// The built catalog. One instance, built on first use, behind `QualityCatalog::shared`.
pub struct QualityCatalog {
    /// Every writable spelling at every voicing, sorted by display then omitted count.
    pub all: Vec<CatalogEntry>,
    /// The whole catalog filed under the keys each spelling sounds, after the
    /// contrived-fifth and synonym filters.
    pub by_mask: BTreeMap<Mask, Vec<CatalogEntry>>,
    /// Every spelling once each, at its full complement of notes, sorted by display.
    pub complete_spellings: Vec<CatalogEntry>,
    /// `complete_spellings` as fingerprints, in the same order.
    pub complete_spelling_masks: Vec<Mask>,
    /// `by_mask` plus the nearest readings for every fingerprint inside the naming
    /// window that no spelling matches.
    pub readings_by_mask: BTreeMap<Mask, Vec<CatalogEntry>>,
}

static SHARED: LazyLock<QualityCatalog> = LazyLock::new(QualityCatalog::build);

impl QualityCatalog {
    /// The catalog, built once per process.
    pub fn shared() -> &'static QualityCatalog {
        &SHARED
    }

    /// The thirteen triad-and-seventh pairs the app names, each with its key into
    /// `tuning::quality_commonness`. This list *is* the commonness table.
    pub fn base_pairs() -> [BasePair; 13] {
        use SeventhQuality as S;
        use TriadQuality as T;
        [
            (T::Major, S::None, "major"),
            (T::Minor, S::None, "minor"),
            (T::Augmented, S::None, "augmented"),
            (T::Diminished, S::None, "diminished"),
            (T::Major, S::Minor, "dominant7"),
            (T::Major, S::Major, "major7"),
            (T::Minor, S::Minor, "minor7"),
            (T::Minor, S::Major, "minorMajor7"),
            (T::Diminished, S::Minor, "halfDiminished"),
            (T::Diminished, S::Diminished, "fullyDiminished"),
            // The diminished triad was the only one of the four missing a major
            // seventh; without it `C Eb Gb B` came out `CmΔ7b5`.
            (T::Diminished, S::Major, "diminishedMajor7"),
            (T::Augmented, S::Minor, "augmented7"),
            (T::Augmented, S::Major, "augmentedMajor7"),
        ]
    }

    /// Which extension combinations are written on a given triad and seventh.
    ///
    /// Restrained on purpose: the vocabulary admits any combination of eleven
    /// tokens, which is two thousand spellings per root and mostly nonsense; what
    /// is generated here is the set that names chords people play. The bounds are
    /// the Swift engine's, comment for comment; see the source file for the
    /// reasoning behind each.
    pub fn extension_sets(
        triad: TriadQuality,
        seventh: SeventhQuality,
    ) -> Vec<Vec<ChordExtension>> {
        use ChordExtension as X;
        let mut sets: Vec<Vec<ChordExtension>> = vec![vec![]];
        if seventh == SeventhQuality::None {
            sets.extend([vec![X::Six], vec![X::AddNine], vec![X::Six, X::AddNine]]);

            // `5` only on a major triad: with no third sounding there is no triad
            // to name, and the major triad is the one whose symbol is empty.
            if triad == TriadQuality::Major {
                sets.push(vec![X::Five]);
            }

            // `add2` and `add4`, on a major triad only.
            if triad == TriadQuality::Major {
                sets.extend([vec![X::AddTwo], vec![X::AddFour]]);
            }
        } else {
            sets.extend([
                vec![X::FlatNine],
                vec![X::SharpNine],
                vec![X::SharpEleven],
                vec![X::FlatThirteen],
            ]);

            // The stack — `9`, `11`, `13` — only over an ordinary triad with an
            // ordinary seventh.
            if (triad == TriadQuality::Major || triad == TriadQuality::Minor)
                && (seventh == SeventhQuality::Minor || seventh == SeventhQuality::Major)
            {
                sets.extend([vec![X::Nine], vec![X::Eleven], vec![X::Thirteen]]);
            }

            // The bound on combining alterations: dominant sevenths only; one
            // alteration of the ninth, one from above it; the same two ninth
            // alterations over a `13`; and one four-token exception, the altered
            // dominant `7b5b9#9b13`.
            if triad == TriadQuality::Major && seventh == SeventhQuality::Minor {
                for ninth in [X::FlatNine, X::SharpNine] {
                    for upper in [X::SharpEleven, X::FlatThirteen] {
                        sets.push(vec![ninth, upper]);
                    }
                    sets.push(vec![X::Thirteen, ninth]);
                }
                sets.push(vec![
                    X::FlatFive,
                    X::FlatNine,
                    X::SharpNine,
                    X::FlatThirteen,
                ]);
            }
        }

        // Suspensions replace the third, so they belong only where there is an
        // ordinary third to replace — a major triad, with or without a seventh.
        if triad == TriadQuality::Major
            && matches!(
                seventh,
                SeventhQuality::None | SeventhQuality::Minor | SeventhQuality::Major
            )
        {
            sets.extend([vec![X::Sus4], vec![X::Sus2]]);
            // A suspension carrying the stack above it — `C9sus4`, `C13sus4` —
            // bounded to the dominant and to these two rungs.
            if seventh == SeventhQuality::Minor {
                sets.extend([vec![X::Nine, X::Sus4], vec![X::Thirteen, X::Sus4]]);
            }
        }

        // `b5` and `#5` need a symbol in front of them, or they collide with the
        // root: `Cb5` reads as a chord on C flat. So the alteration is only
        // written where something separates it from the root.
        let root_would_run_into_the_alteration =
            triad.symbol().is_empty() && seventh.symbol().is_empty();
        if (triad == TriadQuality::Major || triad == TriadQuality::Minor)
            && !root_would_run_into_the_alteration
        {
            sets.extend([vec![X::FlatFive], vec![X::SharpFive]]);
        }
        sets
    }

    /// Every quality the app names: each base pair with each of its extension sets.
    pub fn templates() -> Vec<QualityTemplate> {
        Self::base_pairs()
            .iter()
            .flat_map(|&base| {
                Self::extension_sets(base.0, base.1)
                    .into_iter()
                    .map(move |extensions| QualityTemplate {
                        triad: base.0,
                        seventh: base.1,
                        commonness_key: Self::commonness_key(base, &extensions),
                        extensions,
                    })
            })
            .collect()
    }

    /// `C6`, `Cm6`, `Csus4` and `Csus2` are named qualities in their own right
    /// rather than a triad wearing an extension. Everything else is scored as the
    /// seventh chord underneath it, less the charge for the token it carries.
    fn commonness_key(base: BasePair, extensions: &[ChordExtension]) -> &'static str {
        use ChordExtension as X;
        if base.1 != SeventhQuality::None {
            return base.2;
        }
        match (base.0, extensions) {
            (TriadQuality::Major, [X::Six]) => "major6",
            (TriadQuality::Minor, [X::Six]) => "minor6",
            (TriadQuality::Major, [X::Sus4]) => "sus4",
            (TriadQuality::Major, [X::Sus2]) => "sus2",
            (TriadQuality::Major, [X::Five]) => "power",
            _ => base.2,
        }
    }

    /// Every set of keys this spelling can honestly name: the complete chord, and
    /// the complete chord less any combination of the notes it is allowed to drop.
    ///
    /// Every voicing here is a subset of the spelling's own notes, so a key the
    /// spelling does not contain can never be matched.
    fn voicings(chord: &SpelledChord) -> Vec<BTreeSet<PitchClass>> {
        let droppable = chord.omissible_pitch_classes();
        if droppable.is_empty() {
            return vec![chord.pitch_classes.clone()];
        }
        (0..(1u32 << droppable.len()))
            .map(|pattern| {
                let mut keys = chord.pitch_classes.clone();
                for (index, pitch_class) in droppable.iter().enumerate() {
                    if pattern & (1 << index) != 0 {
                        keys.remove(pitch_class);
                    }
                }
                keys
            })
            .collect()
    }

    /// Every writable spelling: each template on each of the seventeen roots,
    /// sorted by display so construction order never depends on set iteration.
    fn build_all() -> Vec<CatalogEntry> {
        let templates = Self::templates();
        let mut all: Vec<CatalogEntry> = NoteSpelling::root_spellings()
            .into_iter()
            .flat_map(|root| {
                templates.iter().flat_map(move |template| {
                    let spelling = ChordSpelling::new(
                        root,
                        template.triad,
                        template.seventh,
                        &template.extensions,
                    );
                    let entries: Vec<CatalogEntry> = match spelling.spelled() {
                        None => vec![],
                        Some(chord) => {
                            let chord = Arc::new(chord);
                            Self::voicings(&chord)
                                .into_iter()
                                .map(|pitch_classes| CatalogEntry {
                                    chord: Arc::clone(&chord),
                                    pitch_classes,
                                    commonness_key: template.commonness_key,
                                })
                                .collect()
                        }
                    };
                    entries
                })
            })
            .collect();
        all.sort_by_cached_key(|e| (e.display(), e.omitted_tone_count()));
        all
    }

    /// Drops a reading that had to move the fifth to explain keys some other
    /// reading explains with a plain triad and no extension at all. `Em#5` is a
    /// real spelling of `C E G` and is also not what anyone means.
    fn dropping_contrived_fifths(entries: Vec<CatalogEntry>) -> Vec<CatalogEntry> {
        if !entries
            .iter()
            .any(|e| e.chord.spelling.extensions.is_empty())
        {
            return entries;
        }
        entries
            .into_iter()
            .filter(|e| {
                !e.chord
                    .spelling
                    .extensions
                    .iter()
                    .any(|x| matches!(x, ChordExtension::FlatFive | ChordExtension::SharpFive))
            })
            .collect()
    }

    /// Two spellings on the **same root** that sound the **same keys** are two
    /// ways of writing one chord, not two readings of it: `C-dim7` and `C-6`. The
    /// plainest survives. Two spellings on the same root *pitch class* but
    /// different letters (`C#Δ7`, `DbΔ7`) are the enharmonic axis and both stay.
    fn collapsing_synonyms(entries: Vec<CatalogEntry>) -> Vec<CatalogEntry> {
        let mut best: BTreeMap<String, CatalogEntry> = BTreeMap::new();
        for entry in entries {
            let written = entry.root().display();
            match best.get(&written) {
                None => {
                    best.insert(written, entry);
                }
                Some(incumbent) => {
                    if Self::is_plainer(&entry, incumbent) {
                        best.insert(written, entry);
                    }
                }
            }
        }
        let mut kept: Vec<CatalogEntry> = best.into_values().collect();
        kept.sort_by_cached_key(|e| e.display());
        kept
    }

    fn is_plainer(lhs: &CatalogEntry, rhs: &CatalogEntry) -> bool {
        if lhs.omitted_tone_count() != rhs.omitted_tone_count() {
            return lhs.omitted_tone_count() < rhs.omitted_tone_count();
        }
        if lhs.extension_count() != rhs.extension_count() {
            return lhs.extension_count() < rhs.extension_count();
        }
        // A spelling whose name is a claim about which octave a note sits in —
        // `add2` against `add9` — always loses to the twin that makes no such claim.
        let lhs_names_an_octave = lhs
            .chord
            .spelling
            .extensions
            .iter()
            .any(|x| x.names_an_octave());
        let rhs_names_an_octave = rhs
            .chord
            .spelling
            .extensions
            .iter()
            .any(|x| x.names_an_octave());
        if lhs_names_an_octave != rhs_names_an_octave {
            return rhs_names_an_octave;
        }
        let (lhs_display, rhs_display) = (lhs.display(), rhs.display());
        if lhs_display.chars().count() != rhs_display.chars().count() {
            return lhs_display.chars().count() < rhs_display.chars().count();
        }
        lhs_display < rhs_display
    }

    /// Fingerprints a set of pitch classes, one bit per key.
    pub fn mask(pitch_classes: impl IntoIterator<Item = PitchClass>) -> Mask {
        pitch_classes
            .into_iter()
            .fold(0, |acc, pc| acc | (1 << pc.value()))
    }

    fn build() -> QualityCatalog {
        let all = Self::build_all();

        let mut grouped: BTreeMap<Mask, Vec<CatalogEntry>> = BTreeMap::new();
        for entry in &all {
            grouped.entry(entry.mask()).or_default().push(entry.clone());
        }
        let by_mask: BTreeMap<Mask, Vec<CatalogEntry>> = grouped
            .into_iter()
            .map(|(mask, entries)| {
                (
                    mask,
                    Self::collapsing_synonyms(Self::dropping_contrived_fifths(entries)),
                )
            })
            .collect();

        // Derived from `by_mask` rather than from `all`, so the fall-through
        // inherits the two filters the exact path already applies.
        let mut complete_spellings: Vec<CatalogEntry> = by_mask
            .values()
            .flat_map(|entries| {
                entries
                    .iter()
                    .filter(|e| e.omitted_tone_count() == 0)
                    .cloned()
            })
            .collect();
        complete_spellings.sort_by_cached_key(|e| e.display());
        let complete_spelling_masks: Vec<Mask> =
            complete_spellings.iter().map(|e| e.mask()).collect();

        let mut readings_by_mask = by_mask.clone();
        for mask in 0..4096u16 {
            if readings_by_mask.contains_key(&mask) {
                continue;
            }
            let count = mask.count_ones() as usize;
            if !(tuning::MINIMUM_PITCH_CLASSES..=tuning::MAXIMUM_PITCH_CLASSES).contains(&count) {
                continue;
            }
            readings_by_mask.insert(
                mask,
                Self::closest_spellings(&complete_spellings, &complete_spelling_masks, mask),
            );
        }

        QualityCatalog {
            all,
            by_mask,
            complete_spellings,
            complete_spelling_masks,
            readings_by_mask,
        }
    }

    /// Every honest reading of exactly these keys. Empty is a normal answer.
    pub fn candidates(&self, mask: Mask) -> &[CatalogEntry] {
        self.by_mask.get(&mask).map_or(&[], Vec::as_slice)
    }

    /// What to offer for a set of keys: the exact readings where there are any,
    /// and otherwise the closest the vocabulary can come.
    pub fn readings(&self, mask: Mask) -> &[CatalogEntry] {
        self.readings_by_mask.get(&mask).map_or(&[], Vec::as_slice)
    }

    /// The nearest things the grammar can write, for keys no spelling matches:
    /// the best tier reached, and within it the least weighted distance. Only
    /// complete spellings are scanned; the omission voicings are subsets of them.
    pub fn closest_spellings(
        complete_spellings: &[CatalogEntry],
        masks: &[Mask],
        sounding: Mask,
    ) -> Vec<CatalogEntry> {
        let mut best_tier = u8::MAX;
        let mut nearest_tier: Vec<usize> = vec![];
        for (index, &mask) in masks.iter().enumerate() {
            let tier = ChordFitting::tier(mask, sounding);
            if tier > best_tier {
                continue;
            }
            if tier < best_tier {
                best_tier = tier;
                nearest_tier.clear();
            }
            nearest_tier.push(index);
        }

        let mut best_cost = i32::MAX;
        let mut closest: Vec<CatalogEntry> = vec![];
        for index in nearest_tier {
            let entry = &complete_spellings[index];
            let cost = ChordFitting::cost(&entry.chord.tones, masks[index], sounding);
            if cost > best_cost {
                continue;
            }
            if cost < best_cost {
                best_cost = cost;
                closest.clear();
            }
            closest.push(entry.clone());
        }
        closest
    }
}
