//! `Frame`: one value that holds everything the screen shows.
//!
//! Both skins draw from a `Frame`. `--print` prints it as labelled lines and
//! `--json` streams it as one line per settled sounding set. So the screen and
//! the pipe cannot disagree: they are the same value, formatted twice.
//!
//! The model builds one with [`crate::AppModel::frame`]. A `Frame` is data —
//! it is `Serialize` + `Deserialize` and round-trips through JSON to an equal
//! value. ADR-0005 records the decision.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use watchord_core::{
    ChordFit, ChordKey, ChordNote, ChordReading, NoteName, PitchClass, ReadingDisplay, SoundingSet,
    SpellingOrigin,
};

use crate::{NoteGroup, Screen};

/// What the STATE plate reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FrameState {
    /// Nothing has been played yet.
    Idle,
    /// A chord is sounding.
    Held,
    /// The keys are up and the display holds the last chord.
    Released,
}

impl FrameState {
    /// The word on the plate and in the pipe: `idle`, `held`, `released`.
    pub fn label(self) -> &'static str {
        match self {
            FrameState::Idle => "idle",
            FrameState::Held => "held",
            FrameState::Released => "released",
        }
    }
}

/// One reading as the frame carries it: the name as the screen writes it, and
/// every fact the engine ranked it on. The headline is rank 1 of the same list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameReading {
    /// The name in slash form when the bass is not the root: `Am7/C`.
    pub name: String,
    /// `≈`, or `None`. Beside the name.
    pub approximation: Option<String>,
    /// `·no5 ·no11`, `·+F#`, or `None`. Under the name.
    pub fit_detail: Option<String>,
    /// The reading said aloud.
    pub spoken: String,
    /// Which axis produced it.
    pub origin: SpellingOrigin,
    /// How well it fits the keys.
    pub fit: ChordFit,
    /// The ranking score. Higher wins.
    pub score: i32,
    /// The pitch class it is rooted on.
    pub root: PitchClass,
    /// The pitch classes it claims — its whole spelling.
    pub claimed: BTreeSet<PitchClass>,
}

impl FrameReading {
    /// `bass` is the lowest sounding pitch class, for the slash.
    pub fn new(reading: &ChordReading, bass: Option<PitchClass>) -> Self {
        let display = ReadingDisplay::new(reading, bass);
        FrameReading {
            name: display.name,
            approximation: display.approximation,
            fit_detail: display.fit_detail,
            spoken: display.spoken,
            origin: display.origin,
            fit: reading.fit.clone(),
            score: reading.score,
            root: reading.root,
            claimed: reading.pitch_classes.clone(),
        }
    }

    /// The root's name, with sharps: `C#`.
    pub fn root_name(&self) -> &'static str {
        NoteName::pitch_class(self.root, false)
    }

    /// The claimed pitch classes as names: `C E G A`.
    pub fn claimed_row(&self) -> String {
        self.claimed
            .iter()
            .map(|&pc| NoteName::pitch_class(pc, false))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The fit's tier name: `exact`, `missing`, `plus`, `nearest`.
    pub fn fit_name(&self) -> &'static str {
        self.fit.tier_name()
    }
}

/// The slot the theory crate fills: voicing, inversion, numeral, staff. Empty
/// until a later ticket adds the first annotation. `#[serde(default)]` keeps a
/// frame written before that ticket readable after it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Annotations {}

impl Annotations {
    /// True while no annotation has been added.
    pub fn is_empty(&self) -> bool {
        true
    }
}

/// One entry of the history strip: the name it settled under, its identity
/// for note lookups and export, the seconds since the entry before it
/// (`None` for the first entry this session), and the voice leading from
/// that entry (`None` likewise).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameHistoryEntry {
    /// The headline name it settled under, re-derived so it never drifts from
    /// what the engine says today.
    pub name: String,
    /// The pitch-class identity, for looking up its notes.
    pub key: ChordKey,
    /// When it settled, whole seconds since the Unix epoch — for export's
    /// time column. The strip itself only ever shows the gap.
    pub at_unix_seconds: u64,
    /// The gap since the entry before it, in whole seconds.
    pub seconds_since_previous: Option<u64>,
    /// How far the hand moved from the entry before it.
    pub voice_leading: Option<FrameVoiceLeading>,
}

/// Voice leading between one history entry and the one before it — the
/// assignment of smallest total semitone motion. `watchord-theory`'s
/// `VoiceLeading`, carried as plain data so `Frame` stays free of that
/// crate's own types on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameVoiceLeading {
    /// The sum of every semitone move.
    pub total_semitones: u32,
    /// How many notes moved by exactly zero semitones.
    pub common_tones_kept: usize,
    /// The largest single move.
    pub largest_move: u32,
}

/// While the display is stepped into history rather than live: which entry
/// (1-based, oldest first) out of the fixed `total` — the `HISTORY n/64`
/// plate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryStep {
    /// 1-based position of the stepped-to entry within history, oldest first.
    pub index: usize,
    /// The fixed denominator on the plate — `HISTORY_CAPACITY`, not how many
    /// entries are actually held.
    pub total: usize,
}

/// Everything the screen shows, as one plain value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Frame {
    /// Which screen is showing.
    pub screen: Screen,
    /// The standing line about the graph itself, when it is not the real one.
    pub banner: Option<String>,
    /// A line of plain words when something went wrong.
    pub status: Option<String>,
    /// What the INPUT plate reads.
    pub input: String,
    /// The attached inputs, by name.
    pub inputs: Vec<String>,
    /// What the STATE plate reads.
    pub state: FrameState,
    /// The big line. Never empty.
    pub headline_text: String,
    /// Rank 1, or `None` when the engine declined.
    pub headline: Option<FrameReading>,
    /// Why there is no headline, in words, when there is none.
    pub declined: Option<String>,
    /// The keys as note names: `C3  E3  G3  A3`. Empty when nothing sounds.
    pub keys: String,
    /// The sounding set's identity. Empty when nothing is displayed.
    pub key: ChordKey,
    /// The sounding MIDI notes.
    pub sounding: SoundingSet,
    /// Every reading below the headline, best first.
    pub alternates: Vec<FrameReading>,
    /// What the theory crate derives after ranking.
    #[serde(default)]
    pub annotations: Annotations,
    /// Notes on the displayed chord, newest first.
    pub notes: Vec<ChordNote>,
    /// Every note, grouped by chord — the All Notes screen.
    pub groups: Vec<NoteGroup>,
    /// Every note ever written, counted.
    pub notes_total: usize,
    /// The text in the note field.
    pub draft: String,
    /// The last up to 64 settled non-empty sounding sets, oldest first.
    pub history: Vec<FrameHistoryEntry>,
    /// `Some` while the display is stepped into `history` instead of live.
    pub history_step: Option<HistoryStep>,
}

impl Frame {
    /// Every label a renderer must emit, lowercase. A skin writes them in
    /// UPPERCASE, `--print` writes them as they are. A test greps each plain
    /// snapshot and the print output for each one.
    pub const LABELS: [&'static str; 28] = [
        "screen",
        "banner",
        "status",
        "input",
        "inputs",
        "state",
        "headline",
        "approximation",
        "spoken",
        "fit",
        "declined",
        "keys",
        "key",
        "sounding",
        "reading",
        "origin",
        "score",
        "root",
        "claimed",
        "alternate",
        "annotations",
        "history",
        "voice leading",
        "note",
        "notes total",
        "draft",
        "group",
        "written as",
    ];

    /// The labels the Now Playing screen carries: everything but the groups.
    pub const NOW_PLAYING_LABELS: [&'static str; 26] = [
        "screen",
        "banner",
        "status",
        "input",
        "inputs",
        "state",
        "headline",
        "approximation",
        "spoken",
        "fit",
        "declined",
        "keys",
        "key",
        "sounding",
        "reading",
        "origin",
        "score",
        "root",
        "claimed",
        "alternate",
        "annotations",
        "history",
        "voice leading",
        "note",
        "notes total",
        "draft",
    ];

    /// The labels the All Notes screen carries: the head block and the groups.
    pub const ALL_NOTES_LABELS: [&'static str; 18] = [
        "screen",
        "banner",
        "status",
        "input",
        "inputs",
        "state",
        "headline",
        "approximation",
        "spoken",
        "fit",
        "declined",
        "keys",
        "key",
        "sounding",
        "group",
        "note",
        "written as",
        "notes total",
    ];

    /// True when the keys are up and the display is holding the last chord.
    pub fn is_released(&self) -> bool {
        self.state == FrameState::Released
    }

    /// True before anything has been played.
    pub fn is_idle(&self) -> bool {
        self.state == FrameState::Idle
    }

    /// True when the plate is reporting an absence rather than a device.
    pub fn has_no_input(&self) -> bool {
        self.banner.is_none() && self.inputs.is_empty()
    }

    /// `≈` beside the headline, or `None`.
    pub fn headline_approximation(&self) -> Option<&str> {
        self.headline.as_ref()?.approximation.as_deref()
    }

    /// The headline said aloud, or `None`.
    pub fn headline_spoken(&self) -> Option<&str> {
        self.headline.as_ref().map(|h| h.spoken.as_str())
    }

    /// The `·no5 ·no11` line under the headline, or `None`.
    pub fn headline_fit_note(&self) -> Option<&str> {
        self.headline.as_ref()?.fit_detail.as_deref()
    }

    /// The chord a committed note would attach to, or `None`.
    pub fn note_target_key(&self) -> Option<&ChordKey> {
        if self.key.is_empty() {
            None
        } else {
            Some(&self.key)
        }
    }

    /// The headline and the alternates as one ranked list, rank 1 first.
    pub fn readings(&self) -> Vec<&FrameReading> {
        self.headline.iter().chain(self.alternates.iter()).collect()
    }

    /// The sounding notes as MIDI numbers: `60 64 67 69`.
    pub fn sounding_row(&self) -> String {
        self.sounding
            .midi_notes()
            .iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The history entry currently on screen: the stepped-to entry while
    /// `history_step` is set, otherwise the newest (live) one. `None` before
    /// anything has been played this session.
    pub fn displayed_history_entry(&self) -> Option<&FrameHistoryEntry> {
        match self.history_step {
            Some(step) => self.history.get(step.index - 1),
            None => self.history.last(),
        }
    }
}
