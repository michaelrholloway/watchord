//! The one thing about `MidiSource` that can be proven without a keyboard:
//! its channels close on `stop()`, so a consumer's forwarding loop ends and a
//! quit does not hang on a join.
//!
//! Opens a real MIDI client. On a machine with no MIDI layer at all (a CI box
//! with no ALSA sequencer) `start()` refuses, and the test passes vacuously
//! rather than failing on the environment.

use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

use watchord_core::{SoundingSetSource, SourceError};
use watchord_midi::MidiSource;

#[test]
fn stop_closes_both_channels_so_a_consumer_loop_can_end() {
    let mut source = MidiSource::new(None);
    let sounding = source.sounding_sets();
    let inputs = source.connected_inputs();
    match source.start() {
        Ok(()) => {}
        Err(SourceError::CouldNotStart(_)) => return,
        Err(other) => panic!("unexpected start error: {other}"),
    }

    source.stop();

    // A closed channel disconnects at once. A channel still held open by the
    // source times out instead — which is exactly the hang this guards against.
    let grace = Duration::from_secs(3);
    let mut sounding_end = sounding.recv_timeout(grace);
    while sounding_end.is_ok() {
        sounding_end = sounding.recv_timeout(grace);
    }
    assert_eq!(sounding_end, Err(RecvTimeoutError::Disconnected));
    let mut inputs_end = inputs.recv_timeout(grace);
    while inputs_end.is_ok() {
        inputs_end = inputs.recv_timeout(grace);
    }
    assert_eq!(inputs_end, Err(RecvTimeoutError::Disconnected));
}
