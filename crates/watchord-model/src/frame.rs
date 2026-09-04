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
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use watchord_core::{
    ChordFit, ChordKey, ChordNote, ChordReading, NoteName, PitchClass, ReadingDisplay, SoundingSet,
    SpellingOrigin,
};

use crate::{NoteGroup, NotesSort, Screen};

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

/// Drill's own slot in the frame (spec #9, ticket #14): a mode drawn on top
/// of Now Playing, not a screen. `#[serde(default)]` keeps a frame written
/// before this ticket readable after it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DrillFrame {
    /// True while drill is the active mode.
    pub active: bool,
    /// The current target's name, or `None` when drill is off.
    pub target: Option<String>,
    /// The name of the target after this one — shown so the hand can
    /// prepare before the current one is graded.
    pub next_target: Option<String>,
    /// The last attempt's fit tier — `exact`, `missing`, `plus`, `nearest` —
    /// or `None` before anything has been played this session.
    pub grade: Option<String>,
    /// What the last attempt missed or added, named. `None` for an exact
    /// grade, or before anything has been played.
    pub grade_note: Option<String>,
    /// Every chord with drill history: attempts, exact count, last tried.
    pub stats: Vec<DrillStatRow>,
}

impl DrillFrame {
    /// The last grade as one line, or `None` before anything has been
    /// played. `exact` alone; otherwise just `grade_note`, which already
    /// says what happened (`missing G`, `extra B`) — the tier word is not
    /// repeated in front of it.
    pub fn grade_display(&self) -> Option<&str> {
        let tier = self.grade.as_deref()?;
        if tier == "exact" {
            Some(tier)
        } else {
            self.grade_note.as_deref().or(Some(tier))
        }
    }
}

/// One row of the drill stats view.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DrillStatRow {
    /// The chord's name, as the engine writes it.
    pub chord: String,
    /// Times this chord has been the target and graded.
    pub attempts: u32,
    /// How many of those attempts graded exact.
    pub exact: u32,
    /// When it was last attempted.
    pub last_at: Option<SystemTime>,
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
    /// The search query on All Notes, applied to `groups` live as typed.
    /// Spec #9 (ticket #15).
    pub search: String,
    /// Which of the three orders `groups` is sorted by.
    pub notes_sort: NotesSort,
    /// True when the note field is editing an existing note rather than
    /// drafting a new one.
    pub editing: bool,
    /// Drill's own state: target, next target, last grade, stats (spec #9,
    /// ticket #14). A mode drawn on Now Playing, not a separate screen.
    #[serde(default)]
    pub drill: DrillFrame,
}

impl Frame {
    /// Every label a renderer must emit, lowercase. A skin writes them in
    /// UPPERCASE, `--print` writes them as they are. A test greps each plain
    /// snapshot and the print output for each one.
    pub const LABELS: [&'static str; 35] = [
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
        "note",
        "notes total",
        "draft",
        "group",
        "written as",
        "editing",
        "search",
        "sort",
        "tags",
        "drill",
        "target",
        "next target",
        "grade",
        "drill stat",
    ];

    /// The labels the Now Playing screen carries: everything but the groups.
    pub const NOW_PLAYING_LABELS: [&'static str; 30] = [
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
        "note",
        "notes total",
        "draft",
        "editing",
        "drill",
        "target",
        "next target",
        "grade",
        "drill stat",
    ];

    /// The labels the All Notes screen carries: the head block and the groups.
    pub const ALL_NOTES_LABELS: [&'static str; 21] = [
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
        "search",
        "sort",
        "tags",
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
}
