//! MIDI 1.0 bytes in, `MidiEvent`s out. Pure — no `midir` types appear here,
//! so the whole wire format is testable from an array of integers.
//!
//! A byte stream is walked by status: a status byte (`0x80..=0xFF`) names a
//! message and its data length, data bytes (`0x00..=0x7F`) follow, and a data
//! byte with no status before it reuses the last channel status — running
//! status, which `midir` normally resolves for us but which costs nothing to
//! honour. System real-time bytes (`0xF8..=0xFF`) may arrive between any two
//! bytes and are skipped in place; SysEx (`0xF0`) is skipped to its `0xF7`.
//!
//! Getting the *lengths* right matters more than the messages do: get one wrong
//! and every subsequent message in the buffer is parsed from the wrong offset.

use crate::MidiEvent;

const NOTE_OFF: u8 = 0x8;
const NOTE_ON: u8 = 0x9;
const CONTROL_CHANGE: u8 = 0xB;

/// Data bytes that follow a status nibble, for channel-voice messages.
fn data_length(status_nibble: u8) -> usize {
    match status_nibble {
        0xC | 0xD => 1, // program change, channel pressure
        _ => 2,         // note off/on, poly pressure, control change, pitch bend
    }
}

/// Data bytes that follow a system-common status byte.
fn system_common_length(status: u8) -> usize {
    match status {
        0xF1 | 0xF3 => 1, // MTC quarter frame, song select
        0xF2 => 2,        // song position
        _ => 0,           // tune request, undefined, and 0xF7
    }
}

/// Decodes a run of MIDI 1.0 bytes. A trailing partial message — fewer data
/// bytes than the status declares — ends the walk rather than being guessed at.
pub fn decode(bytes: &[u8]) -> Vec<MidiEvent> {
    let mut events = Vec::new();
    let mut running_status: Option<u8> = None;
    let mut index = 0;

    while index < bytes.len() {
        let byte = bytes[index];
        let status = if byte & 0x80 != 0 {
            index += 1;
            if byte >= 0xF8 {
                // Real-time: one byte, may interleave anything, does not
                // touch running status.
                continue;
            }
            if byte == 0xF0 {
                // SysEx: skip to end-of-exclusive. Cancels running status.
                running_status = None;
                while index < bytes.len() && bytes[index] != 0xF7 {
                    index += 1;
                }
                index += 1;
                continue;
            }
            if byte >= 0xF0 {
                // System common: fixed length, cancels running status.
                running_status = None;
                index += system_common_length(byte);
                continue;
            }
            running_status = Some(byte);
            byte
        } else {
            // A data byte with no status in front of it: running status.
            let Some(status) = running_status else {
                index += 1;
                continue;
            };
            status
        };

        let nibble = status >> 4;
        let channel = status & 0x0F;
        let length = data_length(nibble);
        // Collect the data bytes, stepping over any real-time byte that has
        // been interleaved between them.
        let mut data = [0u8; 2];
        let mut collected = 0;
        while collected < length {
            match bytes.get(index) {
                Some(&byte) if byte >= 0xF8 => index += 1,
                Some(&byte) if byte & 0x80 == 0 => {
                    data[collected] = byte;
                    collected += 1;
                    index += 1;
                }
                // A new status byte, or the end of the buffer: this message
                // is truncated. Leave the status for the outer loop.
                _ => break,
            }
        }
        if collected < length {
            if index >= bytes.len() {
                break;
            }
            continue;
        }
        let (data1, data2) = (data[0], data[1]);

        let event = match nibble {
            NOTE_ON => MidiEvent::NoteOn {
                note: data1,
                velocity: data2,
                channel,
            },
            NOTE_OFF => MidiEvent::NoteOff {
                note: data1,
                channel,
            },
            CONTROL_CHANGE => MidiEvent::ControlChange {
                controller: data1,
                value: data2,
                channel,
            },
            _ => continue,
        };
        events.push(event);
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_on_note_off_and_control_change_decode() {
        assert_eq!(
            decode(&[0x90, 60, 100]),
            [MidiEvent::NoteOn {
                note: 60,
                velocity: 100,
                channel: 0
            }]
        );
        assert_eq!(
            decode(&[0x89, 64, 0]),
            [MidiEvent::NoteOff {
                note: 64,
                channel: 9
            }]
        );
        assert_eq!(
            decode(&[0xB5, 64, 127]),
            [MidiEvent::ControlChange {
                controller: 64,
                value: 127,
                channel: 5
            }]
        );
    }

    #[test]
    fn cc64_cc66_and_cc67_decode_on_and_off_at_the_threshold() {
        // 64 = sustain, 66 = sostenuto, 67 = soft (ticket 13); threshold 64.
        for controller in [64u8, 66, 67] {
            assert_eq!(
                decode(&[0xB0, controller, 127]),
                [MidiEvent::ControlChange {
                    controller,
                    value: 127,
                    channel: 0
                }],
                "controller {controller} on"
            );
            assert_eq!(
                decode(&[0xB0, controller, 0]),
                [MidiEvent::ControlChange {
                    controller,
                    value: 0,
                    channel: 0
                }],
                "controller {controller} off"
            );
        }
    }

    #[test]
    fn velocity_zero_stays_a_note_on() {
        // The tracker owns the velocity-0 rule, not the decoder.
        assert_eq!(
            decode(&[0x90, 60, 0]),
            [MidiEvent::NoteOn {
                note: 60,
                velocity: 0,
                channel: 0
            }]
        );
    }

    #[test]
    fn several_messages_in_one_buffer_decode_in_order() {
        let events = decode(&[0x90, 60, 100, 0x90, 64, 100, 0x80, 60, 0]);
        assert_eq!(events.len(), 3);
        assert!(matches!(events[2], MidiEvent::NoteOff { note: 60, .. }));
    }

    #[test]
    fn running_status_reuses_the_last_status() {
        let events = decode(&[0x90, 60, 100, 64, 100, 67, 100]);
        assert_eq!(events.len(), 3);
        assert!(matches!(events[2], MidiEvent::NoteOn { note: 67, .. }));
    }

    #[test]
    fn unmodelled_channel_messages_are_skipped_by_their_own_length() {
        // Program change (1 byte), pitch bend (2 bytes), then a note-on that
        // must still parse from the right offset.
        let events = decode(&[0xC0, 5, 0xE0, 0x00, 0x40, 0x90, 60, 100]);
        assert_eq!(
            events,
            [MidiEvent::NoteOn {
                note: 60,
                velocity: 100,
                channel: 0
            }]
        );
    }

    #[test]
    fn real_time_bytes_interleave_without_breaking_a_message() {
        let events = decode(&[0x90, 0xF8, 60, 0xFE, 100]);
        assert_eq!(
            events,
            [MidiEvent::NoteOn {
                note: 60,
                velocity: 100,
                channel: 0
            }]
        );
    }

    #[test]
    fn sysex_is_skipped_to_its_end() {
        let events = decode(&[0xF0, 0x7E, 0x7F, 0x09, 0x01, 0xF7, 0x80, 60, 0]);
        assert_eq!(
            events,
            [MidiEvent::NoteOff {
                note: 60,
                channel: 0
            }]
        );
    }

    #[test]
    fn a_trailing_partial_message_ends_the_walk() {
        assert_eq!(decode(&[0x90, 60, 100, 0x90, 64]), decode(&[0x90, 60, 100]));
        assert!(decode(&[0x90]).is_empty());
        assert!(decode(&[]).is_empty());
    }

    #[test]
    fn a_stray_data_byte_with_no_status_is_dropped() {
        assert!(decode(&[60, 100]).is_empty());
    }
}
