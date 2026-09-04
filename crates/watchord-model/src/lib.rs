//! watchord-model: everything on screen, as one plain value.
//!
//! Ported from note-view's `Sources/NoteViewApp/AppModel.swift`. The skin is
//! dumb: it reads properties off [`AppModel`] and draws them. Every rule that
//! could be wrong — the display holding after release, a decline rendering as
//! words, a committed note appearing immediately, state surviving a screen
//! switch — lives here and is reachable from a plain unit test, with no terminal.
//!
//! Swift's `@Observable` + `Task` consumption of two `AsyncStream`s becomes one
//! [`ModelEvent`] channel that [`AppModel::poll`] and [`AppModel::wait`] drain.
//! There is no async runtime in the workspace; a skin's event loop calls `poll`
//! on its tick, or `wait` when it has nothing else to do.

pub mod drill;
pub mod fakes;
pub mod frame;
pub mod notes;

pub use drill::{DrillFit, Xorshift64};
pub use frame::{
    Annotations, DrillFrame, DrillStatRow, Frame, FrameHistoryEntry, FrameReading, FrameState,
    FrameVoiceLeading, HistoryStep,
};
pub use notes::NotesSort;

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use watchord_core::tuning;
use watchord_core::{
    ChordAnalysis, ChordKey, ChordNaming, ChordNote, ChordVocabulary, ControlEvent, DeclineReason,
    DrillStoring, DrillTarget, NoteName, NoteStoring, PedalKind, PitchClass, ReadingDisplay,
    SoundingSet, SoundingSetSource,
};
use watchord_theory::key_context::{self, Key, Mode};
use watchord_theory::voice_leading::{self, VoiceLeading};

/// Which screen is showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Screen {
    /// The live chord, its alternates, its notes, and the note field.
    NowPlaying,
    /// Every note ever written, grouped by chord.
    AllNotes,
}

impl Screen {
    /// Every screen, in the order the skin cycles them.
    pub const ALL: [Screen; 2] = [Screen::NowPlaying, Screen::AllNotes];

    /// The tab's label.
    pub fn title(self) -> &'static str {
        match self {
            Screen::NowPlaying => "Now Playing",
            Screen::AllNotes => "All Notes",
        }
    }

    /// The Swift raw value: `nowPlaying`, `allNotes`.
    pub fn raw_value(self) -> &'static str {
        match self {
            Screen::NowPlaying => "nowPlaying",
            Screen::AllNotes => "allNotes",
        }
    }
}

/// A chord's notes on the All Notes screen.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteGroup {
    /// The identity the notes are stored against.
    pub key: ChordKey,
    /// The chord's headline **as the engine names it today** — re-derived from
    /// the key rather than read off any note. `spelling_when_written` is shown
    /// per row instead, where it belongs.
    pub heading: String,
    /// Newest first.
    pub notes: Vec<ChordNote>,
}

impl NoteGroup {
    /// A stable identity for the group: the key's text.
    pub fn id(&self) -> &str {
        self.key.raw()
    }
}

/// One thing the input told the model. Both streams from the source are merged
/// onto one channel so a skin has exactly one thing to wait on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelEvent {
    /// A settled sounding set, including the empty one on full release.
    Sounding(SoundingSet),
    /// The whole current set of attached input names.
    Inputs(Vec<String>),
    /// A pedal moved up or down.
    Control(ControlEvent),
}

/// The two forwarding threads and the channel they feed, alive between
/// `start()` and `stop()`.
struct Listening {
    events: Receiver<ModelEvent>,
    threads: Vec<JoinHandle<()>>,
}

/// The maximum width of the input plate's copy of a device name.
pub const INPUT_LABEL_LIMIT: usize = 28;

/// How many settled non-empty sounding sets [`AppModel::history`] keeps. The
/// oldest drops once a 65th arrives. Also the denominator on the `HISTORY
/// n/64` plate, ticket 12.
pub const HISTORY_CAPACITY: usize = 64;

/// Stamps a new history entry. Real time by default; [`AppModel::with_clock`]
/// injects one so a test can state the gaps between chords.
pub type Clock = Box<dyn Fn() -> SystemTime + Send + Sync>;

/// One settled non-empty sounding set, as history keeps it: when it settled,
/// how long since the one before it, and the voice leading from that one —
/// all computed once, at the moment it is pushed, against the entry that was
/// then the newest.
#[derive(Clone, Debug, PartialEq, Eq)]
struct HistoryEntry {
    sounding: SoundingSet,
    at: SystemTime,
    seconds_since_previous: Option<u64>,
    voice_leading: Option<VoiceLeading>,
}

/// The settle window's floor, in milliseconds.
pub const SETTLE_MIN_MS: u64 = tuning::SETTLE_MIN_MS;
/// The settle window's ceiling, in milliseconds.
pub const SETTLE_MAX_MS: u64 = tuning::SETTLE_MAX_MS;
/// The settle window's step, in milliseconds.
pub const SETTLE_STEP_MS: u64 = tuning::SETTLE_STEP_MS;

/// Everything on screen.
pub struct AppModel {
    /// Which screen is showing. Switching does not disturb anything else on this
    /// object, which is what makes "state survives a screen switch" true by
    /// construction rather than by care.
    pub screen: Screen,

    /// The analysis currently on screen, or `None` before anything has been
    /// played. **This is not "what is sounding".** It is the last thing worth
    /// looking at, which is a different thing the moment the hands come up.
    displayed: Option<ChordAnalysis>,

    /// True when the keys are up but `displayed` is still showing the chord that
    /// was just played. The display holds after release.
    is_released: bool,

    /// Notes saved against `displayed`'s chord key, newest first.
    notes_for_displayed_chord: Vec<ChordNote>,

    /// Every note, grouped by chord — the All Notes screen.
    note_groups: Vec<NoteGroup>,

    /// The text in the always-present note field.
    pub draft_note_text: String,

    /// The id of the note `draft_note_text` is editing, or `None` when it is
    /// drafting a new one. Set by [`AppModel::begin_edit`].
    editing_note_id: Option<String>,

    /// The search query on All Notes, typed live. Filters `note_groups` by
    /// text, tag, or chord name (spec #9).
    pub search_text: String,

    /// Which of the three orders All Notes reads in.
    pub notes_sort: NotesSort,

    /// A line of plain words when something outside the model's control went
    /// wrong — no MIDI available, the notes file could not be read. Never a
    /// crash, never a blank screen.
    status_message: Option<String>,

    /// A standing line about the graph itself, set once by the composition root
    /// and never cleared. Kept apart from `status_message` so that a successful
    /// save does not quietly erase "these are fakes".
    banner: Option<String>,

    /// The MIDI devices currently attached, by display name. Empty means nothing
    /// is plugged in — a normal state the screen has to be able to say.
    connected_inputs: Vec<String>,

    /// The sustain pedal, as the source last reported it.
    sustain: bool,
    /// The sostenuto pedal, as the source last reported it.
    sostenuto: bool,
    /// The soft pedal, as the source last reported it.
    soft: bool,
    /// The settle window, adjusted live by two keys.
    settle: Duration,
    /// True while arpeggio mode is on.
    arpeggio: bool,
    /// The notes accumulated so far this arpeggio, while `arpeggio` is on.
    /// Empty otherwise. A key going fully up, or the sustain pedal lifting,
    /// clears it — the next note-on starts a fresh chord.
    arpeggio_notes: SoundingSet,

    /// The tonic and mode every reading's numeral is read against. Hand-set
    /// with `cycle_key_tonic`/`toggle_key_mode`, or set from the headline
    /// while the soft pedal is held (ticket #16). `None` until set; session
    /// only — never saved.
    key_context: Option<Key>,

    naming: Arc<dyn ChordNaming>,
    source: Box<dyn SoundingSetSource>,
    store: Arc<dyn NoteStoring>,
    listening: Option<Listening>,

    /// The last [`HISTORY_CAPACITY`] settled non-empty sounding sets, oldest
    /// first — "newest last" (`CONTEXT.md`).
    history: VecDeque<HistoryEntry>,
    /// `Some(index)` while the display is stepped into `history` (0-based
    /// from the front). `None` when it is live. Any new non-empty sounding
    /// set clears it back to `None`.
    history_cursor: Option<usize>,
    /// Stamps new history entries.
    clock: Clock,

    /// The engine's naming vocabulary, for drawing drill targets. `None`
    /// until [`AppModel::with_drill`] — a model with no vocabulary simply
    /// never offers to enter drill, rather than every existing call site
    /// (tests, other fakes) having to supply one it does not care about.
    vocabulary: Option<Arc<dyn ChordVocabulary>>,
    /// Where drill stats are read and recorded. `None` alongside `vocabulary`.
    drill_store: Option<Arc<dyn DrillStoring>>,
    /// The drill mode's own state, kept apart from the rest so entering and
    /// leaving it disturbs nothing else — the same discipline `screen`
    /// already keeps.
    drill: DrillState,
}

/// Drill's state on [`AppModel`]. A mode, not a screen (Michael's ruling,
/// 2026-09-03): it draws on top of Now Playing rather than replacing it.
#[derive(Default)]
struct DrillState {
    active: bool,
    target: Option<DrillTarget>,
    next_target: Option<DrillTarget>,
    last_grade: Option<(DrillTarget, DrillFit)>,
    rng: Xorshift64,
}

impl AppModel {
    /// A model over the three seams, opening on Now Playing with every stored
    /// note already grouped. Does not start the input; call [`AppModel::start`].
    pub fn new(
        naming: Arc<dyn ChordNaming>,
        source: Box<dyn SoundingSetSource>,
        store: Arc<dyn NoteStoring>,
    ) -> Self {
        Self::with_clock(naming, source, store, Box::new(SystemTime::now))
    }

    /// Same as [`AppModel::new`], with history entries stamped by `clock`
    /// instead of the wall clock — for a test that wants known gaps between
    /// chords.
    pub fn with_clock(
        naming: Arc<dyn ChordNaming>,
        source: Box<dyn SoundingSetSource>,
        store: Arc<dyn NoteStoring>,
        clock: Clock,
    ) -> Self {
        let mut model = AppModel {
            screen: Screen::NowPlaying,
            displayed: None,
            is_released: false,
            notes_for_displayed_chord: Vec::new(),
            note_groups: Vec::new(),
            draft_note_text: String::new(),
            editing_note_id: None,
            search_text: String::new(),
            notes_sort: NotesSort::default(),
            status_message: None,
            banner: None,
            connected_inputs: Vec::new(),
            sustain: false,
            sostenuto: false,
            soft: false,
            settle: tuning::SETTLE_INTERVAL,
            arpeggio: false,
            arpeggio_notes: SoundingSet::silent(),
            key_context: None,
            naming,
            source,
            store,
            listening: None,
            history: VecDeque::new(),
            history_cursor: None,
            clock,
            vocabulary: None,
            drill_store: None,
            drill: DrillState::default(),
        };
        model.reload_all_notes();
        model
    }

    /// Enables drill: without this, [`AppModel::enter_drill`] is a no-op and
    /// `frame().drill.active` never turns true. Kept as a builder step,
    /// separate from the three required seams, so no existing call site —
    /// fakes, tests, the other graphs — needs to name a vocabulary or a
    /// drill store it does not use.
    pub fn with_drill(
        mut self,
        vocabulary: Arc<dyn ChordVocabulary>,
        drill_store: Arc<dyn DrillStoring>,
    ) -> Self {
        self.vocabulary = Some(vocabulary);
        self.drill_store = Some(drill_store);
        self
    }

    /// Seeds the drill's target draws, for a test that needs an exact
    /// sequence rather than merely a deterministic one.
    pub fn seed_drill_rng(&mut self, seed: u64) {
        self.drill.rng = Xorshift64::new(seed);
    }

    // MARK: - Lifecycle

    /// Opens the input and begins consuming settled sounding sets. Idempotent.
    ///
    /// A failure to start is reported as a line of plain words and the app
    /// carries on: with no keyboard plugged in there is still a screen, still
    /// All Notes, and still every note ever written. With no hardware attached
    /// `start()` does not fail at all — it succeeds and the stream stays quiet.
    pub fn start(&mut self) {
        if self.listening.is_some() {
            return;
        }
        if let Err(error) = self.source.start() {
            self.status_message = Some(format!("MIDI input unavailable — {error}"));
        }
        let (tx, events) = mpsc::channel();
        let sounding = self.source.sounding_sets();
        let inputs = self.source.connected_inputs();
        let controls = self.source.controls();
        // Three forwarding threads, one per stream. Separate because they are
        // genuinely independent: a keyboard can appear and disappear without a
        // note being played, a pedal can move with no note sounding, and a
        // note can arrive from a source that was already connected at launch.
        // Each thread ends when its sender side is dropped, which `stop()` on
        // every source does.
        let threads = vec![
            forward(sounding, tx.clone(), ModelEvent::Sounding),
            forward(inputs, tx.clone(), ModelEvent::Inputs),
            forward(controls, tx, ModelEvent::Control),
        ];
        self.listening = Some(Listening { events, threads });
    }

    /// Puts a standing line of plain words on the screen. The composition root
    /// uses it to say when the graph is not the real one, so a fake run cannot be
    /// mistaken for a real one by looking at it.
    pub fn announce(&mut self, message: impl Into<String>) {
        self.banner = Some(message.into());
    }

    /// Closes the input and joins the forwarding threads. Idempotent. The
    /// display is left as it was, so the last chord stays readable.
    pub fn stop(&mut self) {
        self.source.stop();
        if let Some(listening) = self.listening.take() {
            // The source has dropped its senders, so both threads are finishing.
            drop(listening.events);
            for thread in listening.threads {
                let _ = thread.join();
            }
        }
        self.connected_inputs.clear();
    }

    /// Applies every event that has arrived since the last call, without
    /// blocking. Returns true when anything was applied.
    pub fn poll(&mut self) -> bool {
        let mut applied = false;
        loop {
            let next = match &self.listening {
                Some(listening) => listening.events.try_recv().ok(),
                None => None,
            };
            let Some(event) = next else { break };
            self.apply(event);
            applied = true;
        }
        applied
    }

    /// Blocks up to `timeout` for one event, then drains the rest. Returns true
    /// when anything was applied. For a loop with nothing else to do.
    pub fn wait(&mut self, timeout: Duration) -> bool {
        let first = match &self.listening {
            Some(listening) => listening.events.recv_timeout(timeout).ok(),
            None => None,
        };
        let Some(event) = first else { return false };
        self.apply(event);
        self.poll();
        true
    }

    /// True while the input's channels are still open. False once every sender
    /// has hung up, which is how a scripted source says it is finished.
    pub fn is_listening(&self) -> bool {
        self.listening
            .as_ref()
            .is_some_and(|listening| listening.threads.iter().any(|thread| !thread.is_finished()))
    }

    fn apply(&mut self, event: ModelEvent) {
        match event {
            ModelEvent::Sounding(sounding) => self.receive(&sounding),
            ModelEvent::Inputs(names) => self.connected_inputs = names,
            ModelEvent::Control(control) => self.apply_control(control),
        }
    }

    /// Applies one pedal moving up or down. Sostenuto toggles arpeggio mode on
    /// its down edge only, so a press-and-release toggles once, not twice.
    /// Sustain lifting while arpeggio is on settles the accumulated set — the
    /// next note-on starts a fresh chord.
    fn apply_control(&mut self, control: ControlEvent) {
        match control.pedal {
            PedalKind::Sustain => {
                self.sustain = control.down;
                if !control.down && self.arpeggio {
                    self.arpeggio_notes = SoundingSet::silent();
                }
            }
            PedalKind::Sostenuto => {
                self.sostenuto = control.down;
                if control.down {
                    self.arpeggio = !self.arpeggio;
                    if !self.arpeggio {
                        self.arpeggio_notes = SoundingSet::silent();
                    }
                }
            }
            PedalKind::Soft => self.soft = control.down,
        }
    }

    // MARK: - Input

    /// One settled sounding set from the input.
    ///
    /// The whole of the hold-after-release rule is these few lines. An **empty**
    /// set marks the display released and changes nothing else — it does not
    /// clear the headline, the alternates, the notes list, or the draft text, so
    /// the field stays pointed at the chord Michael just played and he can write
    /// about it with his hands off the keys. Every non-empty set replaces the
    /// display, including one the engine declines to name: a decline is a normal
    /// answer and gets shown, in words.
    ///
    /// While arpeggio mode is on, `sounding` is unioned onto the notes already
    /// accumulated this arpeggio rather than replacing the display outright —
    /// that is the whole of "a note-on adds and a note-off does not remove
    /// it," since a key coming up would otherwise shrink what the source
    /// reports. A key clearing (every key up) empties the accumulator here,
    /// via the early return below, so the next note-on starts fresh.
    pub fn receive(&mut self, sounding: &SoundingSet) {
        if sounding.is_empty() {
            // Nothing has been played yet: there is no chord to hold, so leave
            // `is_released` false and let the skin show its waiting line.
            self.is_released = self.displayed.is_some();
            if self.arpeggio {
                self.arpeggio_notes = SoundingSet::silent();
            }
            return;
        }
        let effective = if self.arpeggio {
            self.arpeggio_notes = SoundingSet::new(
                self.arpeggio_notes
                    .midi_notes()
                    .iter()
                    .chain(sounding.midi_notes())
                    .copied(),
            );
            self.arpeggio_notes.clone()
        } else {
            sounding.clone()
        };
        let analysis = self.naming.analyze(&effective);
        // Ticket #16: the soft pedal held while a chord settles sets the key
        // context from the headline — root becomes tonic, triad quality
        // picks the mode. A settle that declines to name anything leaves the
        // key context exactly as it was.
        if self.soft
            && let Some(headline) = &analysis.headline
        {
            self.key_context = Some(key_context::key_from_headline(
                headline.root,
                &headline.pitch_classes,
            ));
        }
        let key_changed = self
            .displayed
            .as_ref()
            .is_none_or(|previous| previous.key != analysis.key);
        self.displayed = Some(analysis);
        self.is_released = false;
        if key_changed {
            self.reload_notes_for_displayed_chord();
        }
        // The settled chord is `effective` (accumulated while arpeggio is on),
        // not the raw `sounding` delta this call received — history records
        // what was actually displayed and named.
        self.push_history(effective);
        // Any new sounding set returns the display to live.
        self.history_cursor = None;
        if self.drill.active {
            self.grade_drill_attempt(sounding);
        }
    }

    // MARK: - History

    /// Records one settled non-empty sounding set, computing its gap and
    /// voice leading against whatever was then the newest entry, and dropping
    /// the oldest once there are more than [`HISTORY_CAPACITY`].
    fn push_history(&mut self, sounding: SoundingSet) {
        let at = (self.clock)();
        let previous = self.history.back();
        let seconds_since_previous =
            previous.map(|p| at.duration_since(p.at).unwrap_or_default().as_secs());
        let voice_leading =
            previous.and_then(|p| voice_leading::voice_leading(&p.sounding, &sounding));
        self.history.push_back(HistoryEntry {
            sounding,
            at,
            seconds_since_previous,
            voice_leading,
        });
        while self.history.len() > HISTORY_CAPACITY {
            self.history.pop_front();
        }
    }

    /// How many entries `history` currently holds, at most [`HISTORY_CAPACITY`].
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Steps the display one entry further into the past. A no-op with fewer
    /// than two entries, or already at the oldest one.
    pub fn step_history_back(&mut self) {
        if self.history.len() < 2 {
            return;
        }
        let last = self.history.len() - 1;
        self.history_cursor = Some(match self.history_cursor {
            None => last - 1,
            Some(0) => 0,
            Some(i) => i - 1,
        });
    }

    /// Steps the display one entry back toward now. Stepping forward past the
    /// newest stepped entry returns the display to live.
    pub fn step_history_forward(&mut self) {
        let Some(i) = self.history_cursor else {
            return;
        };
        let last = self.history.len().saturating_sub(1);
        self.history_cursor = if i + 1 >= last { None } else { Some(i + 1) };
    }

    /// The analysis actually on screen: a re-analysis of the stepped history
    /// entry while stepped (`history_cursor` is `Some`), otherwise the live
    /// `displayed`.
    fn effective_analysis(&self) -> Option<ChordAnalysis> {
        match self.history_cursor.and_then(|i| self.history.get(i)) {
            Some(entry) => Some(self.naming.analyze(&entry.sounding)),
            None => self.displayed.clone(),
        }
    }

    /// True when the effective display draws as released: really released,
    /// or showing a stepped history entry, which is always drawn released.
    fn effective_released(&self) -> bool {
        self.history_cursor.is_some() || self.is_released
    }

    /// The headline name for one history entry, independent of what is
    /// currently displayed — for the strip and for export.
    fn entry_headline_text(&self, sounding: &SoundingSet) -> String {
        let analysis = self.naming.analyze(sounding);
        let bass = sounding.bass_pitch_class();
        match analysis.headline.as_ref() {
            Some(reading) => ReadingDisplay::new(reading, bass).name,
            None => NoteName::pitch_classes_row(sounding),
        }
    }

    // MARK: - Drill

    /// True once [`AppModel::with_drill`] has supplied both seams — the
    /// gate `enter_drill` and the plain skin's key handler read before
    /// offering the mode at all.
    pub fn can_drill(&self) -> bool {
        self.vocabulary.is_some() && self.drill_store.is_some()
    }

    /// True while drill is the active mode.
    pub fn is_drilling(&self) -> bool {
        self.drill.active
    }

    /// Enters drill: draws a target and the target after it. A no-op when
    /// there is no vocabulary and store, or drill is already active.
    pub fn enter_drill(&mut self) {
        if !self.can_drill() || self.drill.active {
            return;
        }
        self.drill.active = true;
        self.drill.last_grade = None;
        self.drill.target = self.draw_target();
        self.drill.next_target = self.draw_target();
    }

    /// Leaves drill. The last grade and the drawn targets are cleared, so
    /// re-entering starts a fresh draw rather than resuming a stale one.
    pub fn exit_drill(&mut self) {
        self.drill.active = false;
        self.drill.target = None;
        self.drill.next_target = None;
        self.drill.last_grade = None;
    }

    /// Enters drill if it is off, leaves it if it is on. What the one bound
    /// key calls.
    pub fn toggle_drill(&mut self) {
        if self.drill.active {
            self.exit_drill();
        } else {
            self.enter_drill();
        }
    }

    /// Draws one target, weighted toward chords whose `attempts - exact` is
    /// highest (design note, spec #9): a chord with no history draws like a
    /// chord with a clean record, and a chord missed more than it has been
    /// nailed draws more.
    fn draw_target(&mut self) -> Option<DrillTarget> {
        let vocabulary = self.vocabulary.as_ref()?;
        let targets = vocabulary.targets();
        if targets.is_empty() {
            return None;
        }
        let stats = self
            .drill_store
            .as_ref()
            .and_then(|store| store.stats().ok())
            .unwrap_or_default();
        let weights: Vec<u32> = targets
            .iter()
            .map(|target| {
                let stat = stats.iter().find(|s| s.chord_key == target.key);
                drill::weight_of(stat)
            })
            .collect();
        drill::draw_weighted(&targets, &weights, &mut self.drill.rng).cloned()
    }

    /// Grades `sounding` against the current target, records the attempt,
    /// and advances: the next target becomes current, and a fresh next
    /// target is drawn.
    fn grade_drill_attempt(&mut self, sounding: &SoundingSet) {
        let Some(target) = self.drill.target.clone() else {
            return;
        };
        let claimed = target.key.pitch_classes();
        let played = sounding.pitch_classes();
        let fit = DrillFit::measure(&claimed, &played);
        let exact = fit.is_exact();
        if let Some(store) = &self.drill_store
            && let Err(error) = store.record(&target.key, exact, SystemTime::now())
        {
            self.status_message = Some(format!("Could not save drill stats — {error}"));
        }
        self.drill.last_grade = Some((target, fit));
        self.drill.target = self.drill.next_target.take();
        self.drill.next_target = self.draw_target();
    }

    /// Every chord with drill history, newest-attempts first, ties broken by
    /// name — the stats view (spec #9: "attempts, exact count, and last
    /// tried per chord").
    fn drill_stat_rows(&self) -> Vec<DrillStatRow> {
        let Some(store) = &self.drill_store else {
            return Vec::new();
        };
        let Ok(stats) = store.stats() else {
            return Vec::new();
        };
        let targets = self.vocabulary.as_ref().map(|v| v.targets());
        let display_of = |key: &ChordKey| -> String {
            targets
                .as_ref()
                .and_then(|ts| ts.iter().find(|t| &t.key == key))
                .map(|t| t.display.clone())
                .unwrap_or_else(|| key.raw().to_string())
        };
        let mut rows: Vec<DrillStatRow> = stats
            .iter()
            .filter(|s| s.attempts > 0)
            .map(|s| DrillStatRow {
                chord: display_of(&s.chord_key),
                attempts: s.attempts,
                exact: s.exact,
                last_at: s.last_at,
            })
            .collect();
        rows.sort_by(|a, b| {
            b.attempts
                .cmp(&a.attempts)
                .then_with(|| a.chord.cmp(&b.chord))
        });
        rows
    }

    // MARK: - Reading the display

    /// The analysis on screen, or `None` before anything has been played.
    pub fn displayed(&self) -> Option<&ChordAnalysis> {
        self.displayed.as_ref()
    }

    /// True when the keys are up and the display is holding the last chord.
    pub fn is_released(&self) -> bool {
        self.is_released
    }

    /// Notes saved against the displayed chord, newest first.
    pub fn notes_for_displayed_chord(&self) -> &[ChordNote] {
        &self.notes_for_displayed_chord
    }

    /// Every note, grouped by chord — the All Notes screen's rows.
    pub fn note_groups(&self) -> &[NoteGroup] {
        &self.note_groups
    }

    /// Every note ever written, counted.
    pub fn total_note_count(&self) -> usize {
        self.note_groups.iter().map(|g| g.notes.len()).sum()
    }

    /// The error row's text, or `None` when nothing has gone wrong.
    pub fn status_message(&self) -> Option<&str> {
        self.status_message.as_deref()
    }

    /// Sets the error row's text directly — for a caller outside the model
    /// that has its own outcome to report, such as a session export.
    pub fn set_status(&mut self, message: Option<String>) {
        self.status_message = message;
    }

    /// The standing line set by [`AppModel::announce`], if any.
    pub fn banner(&self) -> Option<&str> {
        self.banner.as_deref()
    }

    /// The attached MIDI inputs, by display name.
    pub fn connected_inputs(&self) -> &[String] {
        &self.connected_inputs
    }

    /// What the running head's input plate says. The device's own name when
    /// there is exactly one, a count when there are several, and `none` when
    /// there is nothing — which is the whole reason this reads a live set rather
    /// than a build flag.
    pub fn input_label(&self) -> String {
        if self.banner.is_some() {
            return "fake".to_string();
        }
        match self.connected_inputs.as_slice() {
            [] => "none".to_string(),
            // Elided rather than wrapped or clipped: the plate sizes to its own
            // content, and an unbounded name would push the next plate off the
            // band.
            [one] => Self::elided(one, INPUT_LABEL_LIMIT),
            many => format!("{} devices", many.len()),
        }
    }

    /// Cuts `name` to `limit` characters, the last one an ellipsis, when it is
    /// longer than `limit`. Counts characters, not bytes.
    pub fn elided(name: &str, limit: usize) -> String {
        if name.chars().count() <= limit {
            return name.to_string();
        }
        let mut cut: String = name.chars().take(limit.saturating_sub(1)).collect();
        cut.push('…');
        cut
    }

    /// True when the plate is reporting an absence rather than a device. Not a
    /// failure — nothing plugged in is an ordinary way to open this app.
    pub fn has_no_input(&self) -> bool {
        self.banner.is_none() && self.connected_inputs.is_empty()
    }

    /// Restricts the source to one named input, or clears the restriction to
    /// listen to everything. Live — no restart. The picker calls this with a
    /// name off `connected_inputs`.
    pub fn select_input(&mut self, name: Option<String>) {
        self.source.select_input(name);
    }

    // MARK: - Pedals, settle, arpeggio

    /// The sustain pedal, as the source last reported it.
    pub fn is_sustain_down(&self) -> bool {
        self.sustain
    }

    /// The sostenuto pedal, as the source last reported it.
    pub fn is_sostenuto_down(&self) -> bool {
        self.sostenuto
    }

    /// The soft pedal, as the source last reported it.
    pub fn is_soft_down(&self) -> bool {
        self.soft
    }

    /// True while arpeggio mode is on.
    pub fn is_arpeggio(&self) -> bool {
        self.arpeggio
    }

    /// The settle window, in milliseconds — what the SETTLE plate reads.
    pub fn settle_ms(&self) -> u64 {
        self.settle.as_millis() as u64
    }

    /// Widens the settle window by one step, clamped at `SETTLE_MAX_MS`.
    pub fn increase_settle(&mut self) {
        self.set_settle_ms((self.settle_ms() + SETTLE_STEP_MS).min(SETTLE_MAX_MS));
    }

    /// Narrows the settle window by one step, clamped at `SETTLE_MIN_MS`.
    pub fn decrease_settle(&mut self) {
        self.set_settle_ms(
            self.settle_ms()
                .saturating_sub(SETTLE_STEP_MS)
                .max(SETTLE_MIN_MS),
        );
    }

    fn set_settle_ms(&mut self, ms: u64) {
        self.settle = Duration::from_millis(ms);
        self.source.set_settle(self.settle);
    }

    // MARK: - Key context (ticket #16)

    /// The tonic and mode every reading's numeral is read against, or `None`
    /// before anything sets it.
    pub fn key_context(&self) -> Option<Key> {
        self.key_context
    }

    /// `k`: steps the tonic up a semitone. Sets `C major` on the first press
    /// when no key is set yet — one of the "two keys [that] set it" (spec
    /// #9); every later press only moves the tonic.
    pub fn cycle_key_tonic(&mut self) {
        self.key_context = Some(match self.key_context {
            Some(key) => key.cycle_tonic(),
            None => Key::new(PitchClass::new(0), Mode::Major),
        });
    }

    /// `m`: flips major/minor. Sets `C major` on the first press when no key
    /// is set yet, the same way `cycle_key_tonic` does.
    pub fn toggle_key_mode(&mut self) {
        self.key_context = Some(match self.key_context {
            Some(key) => key.toggle_mode(),
            None => Key::new(PitchClass::new(0), Mode::Major),
        });
    }

    /// The headline as the screen writes it — the name in slash form, the `≈`
    /// that goes beside it, and the `·no5` detail that goes under it. `None`
    /// when the engine declined to name; `headline_text` still says something.
    pub fn headline_reading(&self) -> Option<ReadingDisplay> {
        let displayed = self.effective_analysis()?;
        let headline = displayed.headline.as_ref()?;
        Some(ReadingDisplay::new(
            headline,
            displayed.sounding.bass_pitch_class(),
        ))
    }

    /// The big line at the top of Now Playing. Never empty, never a crash.
    ///
    /// When the engine declined this is a plain statement of what is sounding —
    /// a lone note's name, or the pitch classes — and `decline_reason` carries
    /// the words explaining why there is no chord name beside it.
    ///
    /// It carries the slash but **not** the `≈`: this string is also what
    /// `commit_note` records as `spelling_when_written`.
    pub fn headline_text(&self) -> String {
        let Some(displayed) = self.effective_analysis() else {
            return "—".to_string();
        };
        if let Some(reading) = self.headline_reading() {
            return reading.name;
        }
        match displayed.declined_reason {
            Some(
                DeclineReason::SingleNote
                | DeclineReason::TooManyPitchClasses
                | DeclineReason::NoHonestReading,
            ) => NoteName::pitch_classes_row(&displayed.sounding),
            Some(DeclineReason::Silent) | None => "—".to_string(),
        }
    }

    /// The reading said aloud, shown small beside the headline, or `None`.
    pub fn headline_spoken(&self) -> Option<String> {
        self.headline_reading().map(|r| r.spoken)
    }

    /// The `≈` that goes **beside** the headline, or `None`.
    pub fn headline_approximation(&self) -> Option<String> {
        self.headline_reading().and_then(|r| r.approximation)
    }

    /// The `·no5 ·no11` line that goes **under** the headline, or `None`.
    pub fn headline_fit_note(&self) -> Option<String> {
        self.headline_reading().and_then(|r| r.fit_detail)
    }

    /// Why the engine declined, in words — `None` when it named the chord.
    pub fn decline_reason(&self) -> Option<String> {
        let displayed = self.effective_analysis()?;
        if displayed.headline.is_some() {
            return None;
        }
        Some(match displayed.declined_reason {
            Some(DeclineReason::SingleNote) => "one note — not enough to name a chord".to_string(),
            Some(DeclineReason::TooManyPitchClasses) => format!(
                "too many notes — {} different pitch classes, past the naming ceiling",
                displayed.sounding.distinct_pitch_class_count()
            ),
            Some(DeclineReason::NoHonestReading) => {
                "no honest reading — these keys do not spell a chord in the grammar".to_string()
            }
            Some(DeclineReason::Silent) => "nothing sounding".to_string(),
            None => "no reading".to_string(),
        })
    }

    /// Every reading below the headline, each tagged with its axis, and each
    /// written the same way the headline is — slash included, because an
    /// alternate written bare beside `C6/E` would claim a bass it does not have.
    pub fn alternates(&self) -> Vec<ReadingDisplay> {
        let Some(displayed) = self.effective_analysis() else {
            return Vec::new();
        };
        let bass = displayed.sounding.bass_pitch_class();
        displayed
            .alternates
            .iter()
            .map(|reading| ReadingDisplay::new(reading, bass))
            .collect()
    }

    /// The sounding keys as note names: `"C3  E3  G3  A3"`.
    pub fn keys_row(&self) -> String {
        match self.effective_analysis() {
            Some(displayed) if !displayed.sounding.is_empty() => {
                NoteName::keys_row(&displayed.sounding)
            }
            _ => String::new(),
        }
    }

    /// The chord a committed note would attach to, or `None` when there is
    /// nothing to attach to. Silence has a key and it is empty; nothing is ever
    /// saved against it.
    pub fn note_target_key(&self) -> Option<ChordKey> {
        let key = self.effective_analysis()?.key;
        if key.is_empty() { None } else { Some(key) }
    }

    /// True when Enter would save something: a non-blank draft, and — unless
    /// it is editing an existing note, which needs no live chord — a chord to
    /// attach it to.
    pub fn can_commit_note(&self) -> bool {
        if self.draft_note_text.trim().is_empty() {
            return false;
        }
        self.editing_note_id.is_some() || self.note_target_key().is_some()
    }

    // MARK: - Editing

    /// True while the field is editing an existing note rather than drafting
    /// a new one.
    pub fn is_editing(&self) -> bool {
        self.editing_note_id.is_some()
    }

    /// Loads `id`'s text into the field for editing. Looks across both the
    /// notes on the displayed chord and every group, so a note found on
    /// either screen can be edited. A no-op when `id` is not found.
    pub fn begin_edit(&mut self, id: &str) {
        let found = self
            .notes_for_displayed_chord
            .iter()
            .chain(self.note_groups.iter().flat_map(|g| g.notes.iter()))
            .find(|n| n.id == id)
            .cloned();
        if let Some(note) = found {
            self.draft_note_text = note.text;
            self.editing_note_id = Some(note.id);
        }
    }

    /// Leaves editing and clears the field. A no-op when not editing.
    pub fn cancel_edit(&mut self) {
        if self.editing_note_id.take().is_some() {
            self.draft_note_text.clear();
        }
    }

    /// Steps to the next of the three sort orders.
    pub fn cycle_notes_sort(&mut self) {
        self.notes_sort = self.notes_sort.next();
    }

    // MARK: - The frame

    /// Everything on screen, as one value. Both skins draw from it, `--print`
    /// prints it, and `--json` streams it, so none of them can disagree.
    pub fn frame(&self) -> Frame {
        let effective = self.effective_analysis();
        let bass = effective
            .as_ref()
            .and_then(|d| d.sounding.bass_pitch_class());
        let headline = effective
            .as_ref()
            .and_then(|d| d.headline.as_ref())
            .map(|reading| {
                FrameReading::new(reading, bass).with_key_context(self.key_context, reading)
            });
        let alternates = effective
            .as_ref()
            .map(|d| {
                d.alternates
                    .iter()
                    .map(|reading| {
                        FrameReading::new(reading, bass).with_key_context(self.key_context, reading)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let state = if effective.is_none() {
            FrameState::Idle
        } else if self.effective_released() {
            FrameState::Released
        } else {
            FrameState::Held
        };
        let history = self
            .history
            .iter()
            .map(|entry| FrameHistoryEntry {
                name: self.entry_headline_text(&entry.sounding),
                key: entry.sounding.key(),
                at_unix_seconds: entry
                    .at
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                seconds_since_previous: entry.seconds_since_previous,
                voice_leading: entry.voice_leading.map(|vl| FrameVoiceLeading {
                    total_semitones: vl.total_semitones,
                    common_tones_kept: vl.common_tones_kept,
                    largest_move: vl.largest_move,
                }),
            })
            .collect();
        let history_step = self.history_cursor.map(|i| HistoryStep {
            index: i + 1,
            total: HISTORY_CAPACITY,
        });
        let groups = notes::filter_groups(self.note_groups.clone(), &self.search_text);
        let groups = notes::sort_groups(groups, self.notes_sort);
        let drill = DrillFrame {
            active: self.drill.active,
            target: self.drill.target.as_ref().map(|t| t.display.clone()),
            next_target: self.drill.next_target.as_ref().map(|t| t.display.clone()),
            grade: self
                .drill
                .last_grade
                .as_ref()
                .map(|(_, fit)| fit.tier_name().to_string()),
            grade_note: self.drill.last_grade.as_ref().and_then(|(_, fit)| {
                let note = fit.note();
                if note.is_empty() { None } else { Some(note) }
            }),
            stats: self.drill_stat_rows(),
        };
        Frame {
            screen: self.screen,
            banner: self.banner.clone(),
            status: self.status_message.clone(),
            input: self.input_label(),
            inputs: self.connected_inputs.clone(),
            state,
            headline_text: self.headline_text(),
            headline,
            declined: self.decline_reason(),
            keys: self.keys_row(),
            key: effective
                .as_ref()
                .map(|d| d.key.clone())
                .unwrap_or_default(),
            sounding: effective
                .as_ref()
                .map(|d| d.sounding.clone())
                .unwrap_or_default(),
            alternates,
            annotations: self
                .displayed
                .as_ref()
                .map(watchord_theory::annotate::annotate)
                .map(Annotations::from)
                .unwrap_or_default(),
            notes: self.notes_for_displayed_chord.clone(),
            groups,
            notes_total: self.total_note_count(),
            draft: self.draft_note_text.clone(),
            history,
            history_step,
            search: self.search_text.clone(),
            notes_sort: self.notes_sort,
            editing: self.editing_note_id.is_some(),
            drill,
            sustain: self.sustain,
            sostenuto: self.sostenuto,
            soft: self.soft,
            settle_ms: self.settle_ms(),
            arpeggio: self.arpeggio,
            key_context: self.key_context,
        }
    }

    // MARK: - Notes

    /// Saves the draft against the displayed chord — including a released one.
    ///
    /// `spelling_when_written` records the big line that was actually on screen,
    /// so a note written while the display read `C6` still says `C6` when it
    /// later resurfaces under `Am7`.
    pub fn commit_note(&mut self) {
        let text = self.draft_note_text.trim().to_string();
        if text.is_empty() {
            return;
        }
        if let Some(id) = self.editing_note_id.clone() {
            match self.store.update(&id, &text) {
                Ok(_) => {
                    self.draft_note_text.clear();
                    self.editing_note_id = None;
                    self.status_message = None;
                    self.reload_notes_for_displayed_chord();
                    self.reload_all_notes();
                }
                Err(error) => {
                    self.status_message = Some(format!("Could not save the note — {error}"));
                }
            }
            return;
        }
        let Some(key) = self.note_target_key() else {
            return;
        };
        let spelling = self.headline_text();
        match self.store.add(&text, &key, &spelling) {
            Ok(_) => {
                self.draft_note_text.clear();
                self.status_message = None;
                self.reload_notes_for_displayed_chord();
                self.reload_all_notes();
            }
            Err(error) => {
                self.status_message = Some(format!("Could not save the note — {error}"));
            }
        }
    }

    /// Removes one note by id and refreshes both screens. A failure is
    /// reported on the error row; deleting an absent id is a no-op.
    pub fn delete_note(&mut self, id: &str) {
        match self.store.delete(id) {
            Ok(()) => {
                self.status_message = None;
                self.reload_notes_for_displayed_chord();
                self.reload_all_notes();
            }
            Err(error) => {
                self.status_message = Some(format!("Could not delete the note — {error}"));
            }
        }
    }

    fn reload_notes_for_displayed_chord(&mut self) {
        let Some(key) = self.note_target_key() else {
            self.notes_for_displayed_chord.clear();
            return;
        };
        match self.store.notes(&key) {
            Ok(notes) => self.notes_for_displayed_chord = notes,
            Err(error) => {
                self.notes_for_displayed_chord.clear();
                self.status_message = Some(format!("Could not read notes — {error}"));
            }
        }
    }

    fn reload_all_notes(&mut self) {
        match self.store.all_notes() {
            Ok(notes) => self.note_groups = Self::group(notes, self.naming.as_ref()),
            Err(error) => {
                self.note_groups.clear();
                self.status_message = Some(format!("Could not read notes — {error}"));
            }
        }
    }

    // MARK: - Grouping

    /// Groups notes by chord, newest note first inside a group and the group
    /// with the newest note first overall. Ties between groups break on the
    /// key's text, so the order is the same on every reload.
    ///
    /// A free function over its collaborator so the grouping rule can be tested
    /// on its own, without an input source or a store.
    pub fn group(notes: Vec<ChordNote>, naming: &dyn ChordNaming) -> Vec<NoteGroup> {
        let mut by_key: Vec<(ChordKey, Vec<ChordNote>)> = Vec::new();
        for note in notes {
            match by_key.iter_mut().find(|(key, _)| *key == note.chord_key) {
                Some((_, group)) => group.push(note),
                None => by_key.push((note.chord_key.clone(), vec![note])),
            }
        }
        let mut groups: Vec<NoteGroup> = by_key
            .into_iter()
            .map(|(key, mut notes)| {
                notes.sort_by_key(|n| std::cmp::Reverse(n.created_at));
                NoteGroup {
                    heading: Self::heading(&key, naming),
                    key,
                    notes,
                }
            })
            .collect();
        groups.sort_by(|left, right| {
            let left_date = left
                .notes
                .first()
                .map(|n| n.created_at)
                .unwrap_or(UNIX_EPOCH);
            let right_date = right
                .notes
                .first()
                .map(|n| n.created_at)
                .unwrap_or(UNIX_EPOCH);
            right_date
                .cmp(&left_date)
                .then_with(|| left.key.raw().cmp(right.key.raw()))
        });
        groups
    }

    /// The current headline for a stored key.
    ///
    /// A `ChordKey` is voicing-blind, so there is no bass note to hand the
    /// engine. We ask about one canonical voicing — the pitch classes stacked
    /// upward from middle C — which is deterministic, so the same key always
    /// heads the same way. **No slash here**: the bass of that stacking is an
    /// artefact of how this method builds its question, not a fact about
    /// anything Michael played. The `≈` *is* kept.
    pub fn heading(key: &ChordKey, naming: &dyn ChordNaming) -> String {
        let midi_notes: Vec<u8> = key
            .pitch_classes()
            .iter()
            .map(|pc| 60 + pc.value())
            .collect();
        if midi_notes.is_empty() {
            return key.raw().to_string();
        }
        let analysis = naming.analyze(&SoundingSet::new(midi_notes));
        let Some(headline) = analysis.headline.as_ref() else {
            return key.raw().to_string();
        };
        let display = ReadingDisplay::new(headline, None);
        match display.approximation {
            Some(approximation) => format!("{} {}", display.name, approximation),
            None => display.name,
        }
    }
}

/// Moves everything off one receiver onto the model's channel, until either
/// side hangs up.
fn forward<T: Send + 'static>(
    from: Receiver<T>,
    to: Sender<ModelEvent>,
    wrap: fn(T) -> ModelEvent,
) -> JoinHandle<()> {
    thread::spawn(move || {
        for value in from {
            if to.send(wrap(value)).is_err() {
                break;
            }
        }
    })
}

/// `SystemTime` for `seconds` before now, clamped at the epoch. For seeds.
pub fn seconds_ago(seconds: u64) -> SystemTime {
    SystemTime::now()
        .checked_sub(Duration::from_secs(seconds))
        .unwrap_or(UNIX_EPOCH)
}
