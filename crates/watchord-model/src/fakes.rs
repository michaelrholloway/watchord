//! Fakes for the three seams. Ported from note-view's `Fakes.swift`.
//!
//! They live in the model crate rather than in a test module on purpose:
//! `watchord --fake` runs the real skin, drawing real data, on a machine with
//! nothing plugged in. They are also what the model tests drive, so the thing
//! the tests exercise is the thing the fake launch exercises.
//!
//! Each fake is a cheap handle over shared state (`Clone`), because the model
//! takes ownership of its source and a test still needs to drive it afterwards.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use watchord_core::tuning;
use watchord_core::{
    ChordAnalysis, ChordFit, ChordKey, ChordNaming, ChordNote, ChordReading, ChordRoot,
    DeclineReason, NoteStoring, PitchClass, SoundingSet, SoundingSetSource, SourceError,
    SpellingOrigin, StoreError,
};

/// What the real store says when asked to save against the empty chord key.
/// Mirrored here without importing the store crate — the model sees
/// `NoteStoring` and nothing else.
pub const UNUSABLE_CHORD_KEY: &str = "that chord cannot be saved against";

// MARK: - ScriptedSoundingSetSource

struct ScriptedInner {
    sounding_tx: Mutex<Option<Sender<SoundingSet>>>,
    sounding_rx: Mutex<Option<Receiver<SoundingSet>>>,
    inputs_tx: Mutex<Option<Sender<Vec<String>>>>,
    inputs_rx: Mutex<Option<Receiver<Vec<String>>>>,
    script: Vec<SoundingSet>,
    start_error: Option<String>,
    started: Mutex<bool>,
    stopped: Mutex<bool>,
}

/// A `SoundingSetSource` driven by hand.
///
/// No clock and no settling: the sets pushed in are already settled by
/// contract, so a test states exactly what the UI sees and when. The channels
/// buffer without bound, so the script is delivered whether or not anyone is
/// listening yet — which is what lets `watchord --fake` put a chord on screen
/// the moment the skin opens.
#[derive(Clone)]
pub struct ScriptedSoundingSetSource {
    inner: Arc<ScriptedInner>,
}

impl Default for ScriptedSoundingSetSource {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl ScriptedSoundingSetSource {
    /// A source that pushes `script` in order the moment it starts.
    pub fn new(script: Vec<SoundingSet>) -> Self {
        Self::build(script, None)
    }

    /// A source whose `start()` fails with `message`, to exercise the "input
    /// unavailable" path.
    pub fn failing_to_start(message: &str) -> Self {
        Self::build(Vec::new(), Some(message.to_string()))
    }

    fn build(script: Vec<SoundingSet>, start_error: Option<String>) -> Self {
        let (sounding_tx, sounding_rx) = mpsc::channel();
        let (inputs_tx, inputs_rx) = mpsc::channel();
        ScriptedSoundingSetSource {
            inner: Arc::new(ScriptedInner {
                sounding_tx: Mutex::new(Some(sounding_tx)),
                sounding_rx: Mutex::new(Some(sounding_rx)),
                inputs_tx: Mutex::new(Some(inputs_tx)),
                inputs_rx: Mutex::new(Some(inputs_rx)),
                script,
                start_error,
                started: Mutex::new(false),
                stopped: Mutex::new(false),
            }),
        }
    }

    /// Whether `start()` has been called.
    pub fn did_start(&self) -> bool {
        *lock(&self.inner.started)
    }

    /// Whether `stop()` has been called.
    pub fn did_stop(&self) -> bool {
        *lock(&self.inner.stopped)
    }

    /// Pushes one settled set at the app, as the real input would.
    pub fn send(&self, sounding: SoundingSet) {
        if let Some(tx) = lock(&self.inner.sounding_tx).as_ref() {
            let _ = tx.send(sounding);
        }
    }

    /// [`ScriptedSoundingSetSource::send`] from MIDI note numbers.
    pub fn send_midi_notes(&self, notes: impl IntoIterator<Item = u8>) {
        self.send(SoundingSet::new(notes));
    }

    /// Full release — every key up.
    pub fn release(&self) {
        self.send(SoundingSet::silent());
    }

    /// Publishes a device list, as a plug or an unplug would.
    pub fn attach(&self, names: &[&str]) {
        if let Some(tx) = lock(&self.inner.inputs_tx).as_ref() {
            let _ = tx.send(names.iter().map(|s| s.to_string()).collect());
        }
    }
}

impl SoundingSetSource for ScriptedSoundingSetSource {
    fn sounding_sets(&mut self) -> Receiver<SoundingSet> {
        lock(&self.inner.sounding_rx)
            .take()
            .unwrap_or_else(|| mpsc::channel().1)
    }

    fn connected_inputs(&mut self) -> Receiver<Vec<String>> {
        lock(&self.inner.inputs_rx)
            .take()
            .unwrap_or_else(|| mpsc::channel().1)
    }

    fn start(&mut self) -> Result<(), SourceError> {
        *lock(&self.inner.started) = true;
        if let Some(message) = &self.inner.start_error {
            return Err(SourceError::CouldNotStart(message.clone()));
        }
        for sounding in &self.inner.script {
            self.send(sounding.clone());
        }
        Ok(())
    }

    fn stop(&mut self) {
        *lock(&self.inner.stopped) = true;
        // Dropping the senders closes both channels, so a consumer's loop ends.
        lock(&self.inner.sounding_tx).take();
        lock(&self.inner.inputs_tx).take();
    }
}

// MARK: - InMemoryNoteStore

struct InMemoryInner {
    storage: Mutex<Vec<ChordNote>>,
    add_calls: Mutex<usize>,
    failure: Option<String>,
}

/// A `NoteStoring` that keeps everything in memory. Never touches the disk.
#[derive(Clone)]
pub struct InMemoryNoteStore {
    inner: Arc<InMemoryInner>,
}

impl Default for InMemoryNoteStore {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl InMemoryNoteStore {
    /// A store holding `seed`, in any order.
    pub fn new(seed: Vec<ChordNote>) -> Self {
        Self::build(seed, None)
    }

    /// A store whose every operation fails with `message`.
    pub fn failing(seed: Vec<ChordNote>, message: &str) -> Self {
        Self::build(seed, Some(message.to_string()))
    }

    fn build(seed: Vec<ChordNote>, failure: Option<String>) -> Self {
        InMemoryNoteStore {
            inner: Arc::new(InMemoryInner {
                storage: Mutex::new(seed),
                add_calls: Mutex::new(0),
                failure,
            }),
        }
    }

    /// How many times `add` was reached. The real store refuses a note against
    /// the empty chord key, so "no note was saved" is not enough to prove the
    /// UI guarded it — this counts the call itself.
    pub fn add_call_count(&self) -> usize {
        *lock(&self.inner.add_calls)
    }

    fn all(&self) -> Result<Vec<ChordNote>, StoreError> {
        if let Some(message) = &self.inner.failure {
            return Err(StoreError::Io(message.clone()));
        }
        let mut notes = lock(&self.inner.storage).clone();
        notes.sort_by_key(|n| std::cmp::Reverse(n.created_at));
        Ok(notes)
    }
}

impl NoteStoring for InMemoryNoteStore {
    fn notes(&self, key: &ChordKey) -> Result<Vec<ChordNote>, StoreError> {
        Ok(self
            .all()?
            .into_iter()
            .filter(|n| n.chord_key == *key)
            .collect())
    }

    fn all_notes(&self) -> Result<Vec<ChordNote>, StoreError> {
        self.all()
    }

    fn add(&self, text: &str, key: &ChordKey, spelling: &str) -> Result<ChordNote, StoreError> {
        *lock(&self.inner.add_calls) += 1;
        // What the real store does, so the UI's guard is tested against the
        // real rule rather than a permissive fake.
        if key.is_empty() {
            return Err(StoreError::Corrupt(UNUSABLE_CHORD_KEY.to_string()));
        }
        if let Some(message) = &self.inner.failure {
            return Err(StoreError::Io(message.clone()));
        }
        let note = ChordNote::new(
            uuid::Uuid::new_v4().to_string().to_uppercase(),
            key.clone(),
            spelling,
            text,
            SystemTime::now(),
        );
        lock(&self.inner.storage).push(note.clone());
        Ok(note)
    }

    fn update(&self, id: &str, text: &str) -> Result<ChordNote, StoreError> {
        if let Some(message) = &self.inner.failure {
            return Err(StoreError::Io(message.clone()));
        }
        let mut storage = lock(&self.inner.storage);
        let Some(existing) = storage.iter_mut().find(|n| n.id == id) else {
            return Err(StoreError::NoSuchNote(id.to_string()));
        };
        existing.text = text.to_string();
        Ok(existing.clone())
    }

    fn delete(&self, id: &str) -> Result<(), StoreError> {
        if let Some(message) = &self.inner.failure {
            return Err(StoreError::Io(message.clone()));
        }
        lock(&self.inner.storage).retain(|n| n.id != id);
        Ok(())
    }
}

// MARK: - StubChordNaming

/// A `ChordNaming` that answers from a table, and declines honestly otherwise.
///
/// It is **not** a second chord engine and must never grow into one. It exists
/// so the shell can be exercised before the engine lands, and so a test can
/// state "the engine said X" in one line.
#[derive(Clone, Default)]
pub struct StubChordNaming {
    table: Arc<Mutex<HashMap<ChordKey, ChordAnalysis>>>,
}

impl StubChordNaming {
    /// An empty table: every set is declined until [`StubChordNaming::stub`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an answer for the key of `sounding`.
    pub fn stub(&self, sounding: &SoundingSet, analysis: ChordAnalysis) {
        lock(&self.table).insert(sounding.key(), analysis);
    }

    /// The default answer: decline, with the reason the real engine would give
    /// for a set of this size. Reads the floor and ceiling out of `tuning`.
    pub fn decline(sounding: &SoundingSet) -> ChordAnalysis {
        let count = sounding.distinct_pitch_class_count();
        let reason = if count == 0 {
            DeclineReason::Silent
        } else if count < tuning::MINIMUM_PITCH_CLASSES {
            DeclineReason::SingleNote
        } else if count > tuning::MAXIMUM_PITCH_CLASSES {
            DeclineReason::TooManyPitchClasses
        } else {
            DeclineReason::NoHonestReading
        };
        ChordAnalysis::new(sounding.clone(), None, Vec::new(), Some(reason))
    }

    /// Builds an answer in one line: an exact headline over the sounding keys.
    pub fn naming(
        sounding: &SoundingSet,
        headline: &str,
        spoken: &str,
        alternates: Vec<StubAlternate>,
    ) -> ChordAnalysis {
        Self::naming_with_fit(
            sounding,
            headline,
            spoken,
            ChordFit::Exact,
            None,
            alternates,
        )
    }

    /// Builds an answer whose headline carries a fit other than exact.
    ///
    /// `claiming` is the pitch classes the headline claims; `None` means the keys
    /// that are sounding, which is what `Exact` means. The root is recovered from
    /// that set (see `ChordRoot`), which is why the difference is load-bearing.
    pub fn naming_with_fit(
        sounding: &SoundingSet,
        headline: &str,
        spoken: &str,
        fit: ChordFit,
        claiming: Option<&[i32]>,
        alternates: Vec<StubAlternate>,
    ) -> ChordAnalysis {
        let classes = sounding.pitch_classes();
        let claimed = |explicit: Option<&[i32]>| match explicit {
            Some(values) => values.iter().map(|&v| PitchClass::new(v)).collect(),
            None => classes.clone(),
        };
        let headline_classes = claimed(claiming);
        let headline_reading = ChordReading {
            display: headline.to_string(),
            spoken: spoken.to_string(),
            origin: SpellingOrigin::Headline,
            score: 100,
            root: ChordRoot::parse(headline, &headline_classes)
                .map(|r| r.pitch_class)
                .or_else(|| classes.iter().next().copied())
                .unwrap_or(PitchClass::new(0)),
            pitch_classes: headline_classes,
            fit,
        };
        let alternate_readings = alternates
            .into_iter()
            .enumerate()
            .map(|(index, alternate)| {
                let alt_classes = claimed(alternate.claiming.as_deref());
                ChordReading {
                    display: alternate.display.clone(),
                    spoken: alternate.spoken.clone(),
                    origin: alternate.origin,
                    score: 90 - index as i32,
                    root: ChordRoot::parse(&alternate.display, &alt_classes)
                        .map(|r| r.pitch_class)
                        .or_else(|| classes.iter().next().copied())
                        .unwrap_or(PitchClass::new(0)),
                    pitch_classes: alt_classes,
                    fit: alternate.fit,
                }
            })
            .collect();
        ChordAnalysis::new(
            sounding.clone(),
            Some(headline_reading),
            alternate_readings,
            None,
        )
    }
}

impl ChordNaming for StubChordNaming {
    fn analyze(&self, sounding: &SoundingSet) -> ChordAnalysis {
        let stored = lock(&self.table).get(&sounding.key()).cloned();
        match stored {
            // Re-seat the stored answer on the set actually played, so the bass
            // note and octaves on screen are the ones that were sent.
            Some(stored) => ChordAnalysis::new(
                sounding.clone(),
                stored.headline,
                stored.alternates,
                stored.declined_reason,
            ),
            None => Self::decline(sounding),
        }
    }
}

/// One alternate reading for `StubChordNaming::naming`.
#[derive(Clone, Debug)]
pub struct StubAlternate {
    /// The name as written, e.g. `Am7`.
    pub display: String,
    /// Which axis this reading sits on.
    pub origin: SpellingOrigin,
    /// The name said aloud.
    pub spoken: String,
    /// How well it fits the keys; `Exact` unless [`StubAlternate::with_fit`].
    pub fit: ChordFit,
    /// The pitch classes this reading claims; `None` means "the keys sounding".
    pub claiming: Option<Vec<i32>>,
}

impl StubAlternate {
    /// An exact alternate that claims the sounding keys.
    pub fn new(display: &str, origin: SpellingOrigin, spoken: &str) -> Self {
        StubAlternate {
            display: display.to_string(),
            origin,
            spoken: spoken.to_string(),
            fit: ChordFit::Exact,
            claiming: None,
        }
    }

    /// The same alternate with a fit other than exact, claiming `claiming`.
    pub fn with_fit(mut self, fit: ChordFit, claiming: &[i32]) -> Self {
        self.fit = fit;
        self.claiming = Some(claiming.to_vec());
        self
    }
}

/// A lock whose poisoning is not a reason to stop: the guarded state is plain
/// data and a panicking test elsewhere must not cascade.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
