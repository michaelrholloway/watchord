//! `SoundingSetSource` over `midir`: connects to every MIDI input on the
//! machine (or the one named), decodes what arrives, and yields settled
//! `SoundingSet`s. Deliberately thin — the wire format, the state machine and
//! the debounce all live in files that never touch a device.
//!
//! ## Threads
//!
//! - `midir` calls back on its own thread, once per message. The callback
//!   decodes and pushes a batch onto an unbounded channel and returns; it never
//!   blocks and never drops, because a dropped note-off is a stuck note.
//! - A **relay thread** drains that channel into a [`SettleRelay`] in arrival
//!   order — so a note-off can never overtake its note-on — and emits each
//!   settled set.
//! - A **watch thread** re-enumerates the inputs every [`HOT_PLUG_INTERVAL`],
//!   connects what appeared, drops what vanished, and publishes the current
//!   names whenever they change. Polling rather than a platform notification
//!   because it is the one mechanism that behaves the same on all three OSes.
//!
//! `stop()` closes a channel the watch thread waits on, so it ends within one
//! poll and takes every connection with it; the relay thread then sees its
//! channel close and ends too. `stop()` then drops its own senders, so the
//! sounding-set and input-name receivers disconnect and a consumer's loop ends.

use std::collections::BTreeMap;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use midir::{Ignore, MidiInput, MidiInputConnection};
use watchord_core::tuning;
use watchord_core::{ControlEvent, SoundingSet, SoundingSetSource, SourceError};

use crate::{MidiEvent, SettleRelay, decode};

/// A lock whose poisoning is not a reason to stop: the guarded state is plain
/// data and a panicking thread elsewhere must not cascade.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// How often the inputs are re-enumerated for hot-plug.
pub const HOT_PLUG_INTERVAL: Duration = Duration::from_secs(2);

/// The threads and channels alive between `start()` and `stop()`.
struct Running {
    /// Dropping this is the stop signal for the watch thread.
    stop: Sender<()>,
    watch: JoinHandle<()>,
    relay: JoinHandle<()>,
}

/// The real input: every MIDI port on the machine, or the ones whose name
/// contains `--input`'s text. See the module docs for the threads it runs.
pub struct MidiSource {
    client_name: String,
    /// `--input <name>`: connect only to inputs whose name contains this,
    /// case-insensitively. `None` connects to every input. The construction-time
    /// default; `select_input` overrides it live via `live_filter`.
    filter: Option<String>,
    /// The picker's live choice, read by the watch thread every
    /// `HOT_PLUG_INTERVAL`. `None` here falls back to `filter`.
    live_filter: Arc<Mutex<Option<String>>>,
    /// The settle window, read by the relay thread on every loop turn so
    /// `set_settle` takes effect without a restart.
    settle: Arc<Mutex<Duration>>,
    /// The sounding-set channel. The sender is the one the relay thread clones;
    /// the receiver is handed out once by `sounding_sets()`. `stop()` replaces
    /// the pair, which is what closes the consumer's end.
    sounding: (Sender<SoundingSet>, Option<Receiver<SoundingSet>>),
    /// The connected-inputs channel, on the same terms.
    inputs: (Sender<Vec<String>>, Option<Receiver<Vec<String>>>),
    /// The pedal-control channel, on the same terms.
    controls: (Sender<ControlEvent>, Option<Receiver<ControlEvent>>),
    running: Option<Running>,
}

impl Default for MidiSource {
    fn default() -> Self {
        Self::new(None)
    }
}

impl MidiSource {
    /// A source over every input, or only those matching `filter`.
    pub fn new(filter: Option<String>) -> Self {
        Self::with_settle(filter, tuning::SETTLE_INTERVAL)
    }

    /// The same, with the settle window chosen by the caller rather than
    /// `tuning::SETTLE_INTERVAL`. For tests that cannot wait 60 ms per case.
    pub fn with_settle(filter: Option<String>, settle: Duration) -> Self {
        let sounding = mpsc::channel();
        let inputs = mpsc::channel();
        let controls = mpsc::channel();
        MidiSource {
            client_name: "watchord".to_string(),
            live_filter: Arc::new(Mutex::new(filter.clone())),
            filter,
            settle: Arc::new(Mutex::new(settle)),
            sounding: (sounding.0, Some(sounding.1)),
            inputs: (inputs.0, Some(inputs.1)),
            controls: (controls.0, Some(controls.1)),
            running: None,
        }
    }

    /// The names of the inputs on this machine right now, filtered. A probe
    /// for the composition root and the `--print` check; opens no connection.
    pub fn available_inputs(&self) -> Result<Vec<String>, SourceError> {
        let midi = MidiInput::new(&self.client_name)
            .map_err(|e| SourceError::CouldNotStart(e.to_string()))?;
        Ok(midi
            .ports()
            .iter()
            .filter_map(|port| midi.port_name(port).ok())
            .filter(|name| accepts(self.filter.as_deref(), name))
            .collect())
    }
}

/// The filter rule, in one place: a case-insensitive substring match.
fn accepts(filter: Option<&str>, name: &str) -> bool {
    match filter {
        None => true,
        Some(wanted) => name.to_lowercase().contains(&wanted.to_lowercase()),
    }
}

impl SoundingSetSource for MidiSource {
    fn sounding_sets(&mut self) -> Receiver<SoundingSet> {
        self.sounding.1.take().unwrap_or_else(|| mpsc::channel().1)
    }

    fn connected_inputs(&mut self) -> Receiver<Vec<String>> {
        self.inputs.1.take().unwrap_or_else(|| mpsc::channel().1)
    }

    fn controls(&mut self) -> Receiver<ControlEvent> {
        self.controls.1.take().unwrap_or_else(|| mpsc::channel().1)
    }

    fn start(&mut self) -> Result<(), SourceError> {
        if self.running.is_some() {
            return Ok(());
        }
        // Prove the MIDI layer opens before spawning anything.
        MidiInput::new(&self.client_name).map_err(|e| SourceError::CouldNotStart(e.to_string()))?;

        let (events_tx, events_rx) = mpsc::channel::<Vec<MidiEvent>>();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();

        let relay = {
            let sounding_tx = self.sounding.0.clone();
            let controls_tx = self.controls.0.clone();
            let settle = Arc::clone(&self.settle);
            thread::spawn(move || run_relay(events_rx, sounding_tx, controls_tx, settle))
        };
        let watch = {
            let client_name = self.client_name.clone();
            let filter = self.filter.clone();
            let live_filter = Arc::clone(&self.live_filter);
            let inputs_tx = self.inputs.0.clone();
            thread::spawn(move || {
                run_watch(client_name, filter, live_filter, events_tx, inputs_tx, stop_rx)
            })
        };
        self.running = Some(Running {
            stop: stop_tx,
            watch,
            relay,
        });

        // A named input that is not here is reported, not fatal: the watch
        // thread keeps looking, so plugging it in later still works.
        if let Some(wanted) = &self.filter
            && self.available_inputs()?.is_empty()
        {
            return Err(SourceError::NoSuchInput(wanted.clone()));
        }
        Ok(())
    }

    fn stop(&mut self) {
        let Some(running) = self.running.take() else {
            return;
        };
        drop(running.stop);
        let _ = running.watch.join();
        let _ = running.relay.join();
        // The threads held clones of the three senders and have now dropped
        // them. Dropping the originals is what disconnects the receivers a
        // consumer is holding — without it, a forwarding loop on the other end
        // never sees the channel close and a quit that joins it hangs. A fresh
        // set takes their place so a later `start()` has somewhere to send.
        self.sounding = {
            let (tx, rx) = mpsc::channel();
            (tx, Some(rx))
        };
        self.inputs = {
            let (tx, rx) = mpsc::channel();
            (tx, Some(rx))
        };
        self.controls = {
            let (tx, rx) = mpsc::channel();
            (tx, Some(rx))
        };
    }

    fn set_settle(&mut self, settle: Duration) {
        *lock(&self.settle) = settle;
    }

    fn select_input(&mut self, name: Option<String>) {
        *lock(&self.live_filter) = name;
    }
}

impl Drop for MidiSource {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Drains event batches into the relay and emits each settled set and every
/// pedal change. Reads `settle` on every turn so `set_settle` takes effect on
/// the next batch, live. Ends when every sender of `events` is gone, which is
/// when the watch thread ends.
fn run_relay(
    events: Receiver<Vec<MidiEvent>>,
    out: Sender<SoundingSet>,
    controls_out: Sender<ControlEvent>,
    settle: Arc<Mutex<Duration>>,
) {
    let mut relay = SettleRelay::new(*lock(&settle));
    loop {
        let live = *lock(&settle);
        if live != relay.settle() {
            relay.set_settle(live);
        }
        let wait = relay
            .due_at()
            .map(|due| due.saturating_duration_since(Instant::now()))
            .unwrap_or(Duration::from_secs(3600));
        match events.recv_timeout(wait) {
            Ok(batch) => {
                for control in relay.receive(batch, Instant::now()) {
                    if controls_out.send(control).is_err() {
                        return;
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if let Some(sounding) = relay.fire(Instant::now())
            && out.send(sounding).is_err()
        {
            break;
        }
    }
}

/// Connects to the inputs, re-enumerating every `HOT_PLUG_INTERVAL` until
/// `stop` closes. Owns every connection; they drop when it returns. Reads
/// `live_filter` every pass, falling back to `filter`, so `select_input`
/// switches the source without a restart.
fn run_watch(
    client_name: String,
    filter: Option<String>,
    live_filter: Arc<Mutex<Option<String>>>,
    events: Sender<Vec<MidiEvent>>,
    inputs: Sender<Vec<String>>,
    stop: Receiver<()>,
) {
    let mut connections: BTreeMap<String, MidiInputConnection<()>> = BTreeMap::new();
    let mut published: Option<Vec<String>> = None;
    loop {
        let effective = lock(&live_filter).clone().or_else(|| filter.clone());
        let present = enumerate(&client_name, effective.as_deref());

        // Drop what vanished.
        connections.retain(|name, _| present.contains(name));

        // Connect what appeared. Each connection needs its own client: `midir`
        // consumes the `MidiInput` on connect.
        for name in &present {
            if connections.contains_key(name) {
                continue;
            }
            if let Some(connection) = connect(&client_name, name, events.clone()) {
                connections.insert(name.clone(), connection);
            }
        }

        // Publish on the first pass and on every change — a whole set, so a
        // late consumer gets the current answer and never a history.
        let names: Vec<String> = connections.keys().cloned().collect();
        if published.as_ref() != Some(&names) {
            if inputs.send(names.clone()).is_err() {
                break;
            }
            published = Some(names);
        }

        match stop.recv_timeout(HOT_PLUG_INTERVAL) {
            Err(RecvTimeoutError::Timeout) => continue,
            Ok(()) | Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

/// The names of the inputs that pass the filter, in port order.
fn enumerate(client_name: &str, filter: Option<&str>) -> Vec<String> {
    let Ok(midi) = MidiInput::new(client_name) else {
        return Vec::new();
    };
    midi.ports()
        .iter()
        .filter_map(|port| midi.port_name(port).ok())
        .filter(|name| accepts(filter, name))
        .collect()
}

/// Opens one input by name. `None` if it went away between enumerating and
/// connecting, which the next pass will notice.
fn connect(
    client_name: &str,
    name: &str,
    events: Sender<Vec<MidiEvent>>,
) -> Option<MidiInputConnection<()>> {
    let mut midi = MidiInput::new(client_name).ok()?;
    midi.ignore(Ignore::All);
    let port = midi
        .ports()
        .into_iter()
        .find(|port| midi.port_name(port).ok().as_deref() == Some(name))?;
    midi.connect(
        &port,
        "watchord in",
        move |_timestamp, bytes, ()| {
            let decoded = decode(bytes);
            if !decoded.is_empty() {
                let _ = events.send(decoded);
            }
        },
        (),
    )
    .ok()
}

#[cfg(test)]
mod tests {
    use super::accepts;

    #[test]
    fn the_filter_is_a_case_insensitive_substring() {
        assert!(accepts(None, "OP-1 field"));
        assert!(accepts(Some("op-1"), "OP-1 field"));
        assert!(accepts(Some("field"), "OP-1 field"));
        assert!(!accepts(Some("Nord"), "OP-1 field"));
    }
}
