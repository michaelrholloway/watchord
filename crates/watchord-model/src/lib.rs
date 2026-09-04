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

pub mod fakes;
pub mod frame;

pub use frame::{Annotations, Frame, FrameReading, FrameState};

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use watchord_core::{
    ChordAnalysis, ChordKey, ChordNaming, ChordNote, DeclineReason, NoteName, NoteStoring,
    ReadingDisplay, SoundingSet, SoundingSetSource,
};

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
}

/// The two forwarding threads and the channel they feed, alive between
/// `start()` and `stop()`.
struct Listening {
    events: Receiver<ModelEvent>,
    threads: Vec<JoinHandle<()>>,
}

/// The maximum width of the input plate's copy of a device name.
pub const INPUT_LABEL_LIMIT: usize = 28;

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

    naming: Arc<dyn ChordNaming>,
    source: Box<dyn SoundingSetSource>,
    store: Arc<dyn NoteStoring>,
    listening: Option<Listening>,
}

impl AppModel {
    /// A model over the three seams, opening on Now Playing with every stored
    /// note already grouped. Does not start the input; call [`AppModel::start`].
    pub fn new(
        naming: Arc<dyn ChordNaming>,
        source: Box<dyn SoundingSetSource>,
        store: Arc<dyn NoteStoring>,
    ) -> Self {
        let mut model = AppModel {
            screen: Screen::NowPlaying,
            displayed: None,
            is_released: false,
            notes_for_displayed_chord: Vec::new(),
            note_groups: Vec::new(),
            draft_note_text: String::new(),
            status_message: None,
            banner: None,
            connected_inputs: Vec::new(),
            naming,
            source,
            store,
            listening: None,
        };
        model.reload_all_notes();
        model
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
        // Two forwarding threads, one per stream. Separate because the two are
        // genuinely independent: a keyboard can appear and disappear without a
        // note being played, and a note can arrive from a source that was
        // already connected at launch. Each thread ends when its sender side is
        // dropped, which `stop()` on every source does.
        let threads = vec![
            forward(sounding, tx.clone(), ModelEvent::Sounding),
            forward(inputs, tx, ModelEvent::Inputs),
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
    pub fn receive(&mut self, sounding: &SoundingSet) {
        if sounding.is_empty() {
            // Nothing has been played yet: there is no chord to hold, so leave
            // `is_released` false and let the skin show its waiting line.
            self.is_released = self.displayed.is_some();
            return;
        }
        let analysis = self.naming.analyze(sounding);
        let key_changed = self
            .displayed
            .as_ref()
            .is_none_or(|previous| previous.key != analysis.key);
        self.displayed = Some(analysis);
        self.is_released = false;
        if key_changed {
            self.reload_notes_for_displayed_chord();
        }
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

    /// The headline as the screen writes it — the name in slash form, the `≈`
    /// that goes beside it, and the `·no5` detail that goes under it. `None`
    /// when the engine declined to name; `headline_text` still says something.
    pub fn headline_reading(&self) -> Option<ReadingDisplay> {
        let displayed = self.displayed.as_ref()?;
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
        let Some(displayed) = self.displayed.as_ref() else {
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
        let displayed = self.displayed.as_ref()?;
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
        let Some(displayed) = self.displayed.as_ref() else {
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
        match self.displayed.as_ref() {
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
        let key = &self.displayed.as_ref()?.key;
        if key.is_empty() {
            None
        } else {
            Some(key.clone())
        }
    }

    /// True when Enter would save something: a chord to attach to and a
    /// draft that is not blank.
    pub fn can_commit_note(&self) -> bool {
        self.note_target_key().is_some() && !self.draft_note_text.trim().is_empty()
    }

    // MARK: - The frame

    /// Everything on screen, as one value. Both skins draw from it, `--print`
    /// prints it, and `--json` streams it, so none of them can disagree.
    pub fn frame(&self) -> Frame {
        let bass = self
            .displayed
            .as_ref()
            .and_then(|d| d.sounding.bass_pitch_class());
        let headline = self
            .displayed
            .as_ref()
            .and_then(|d| d.headline.as_ref())
            .map(|reading| FrameReading::new(reading, bass));
        let alternates = self
            .displayed
            .as_ref()
            .map(|d| {
                d.alternates
                    .iter()
                    .map(|reading| FrameReading::new(reading, bass))
                    .collect()
            })
            .unwrap_or_default();
        let state = if self.displayed.is_none() {
            FrameState::Idle
        } else if self.is_released {
            FrameState::Released
        } else {
            FrameState::Held
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
            key: self
                .displayed
                .as_ref()
                .map(|d| d.key.clone())
                .unwrap_or_default(),
            sounding: self
                .displayed
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
            groups: self.note_groups.clone(),
            notes_total: self.total_note_count(),
            draft: self.draft_note_text.clone(),
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
        let Some(key) = self.note_target_key() else {
            return;
        };
        if text.is_empty() {
            return;
        }
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
