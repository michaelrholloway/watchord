//! Staff: which clef and row one sounding MIDI note draws on.
//!
//! Spec (#9, "Staff"): treble from middle C (60) up, bass below; nine rows per
//! staff — five line rows and four space rows, evenly spaced — plus ledger rows
//! as needed. Stepping by one natural letter always moves exactly one row and
//! flips line/space, which is what a real staff means by "adjacent letters
//! alternate line and space" — that fact is what the MIDI-note sweep checks.
//!
//! The row math works in a "diatonic index": `(midi_note / 12) * 7 +
//! letter_index`, which is monotonic non-decreasing in `midi_note` for either
//! alphabet (checked in the sweep test below), because within one octave both
//! [`crate::spelling::SHARP`] and `FLAT`'s letter indices are themselves
//! non-decreasing pitch-class to pitch-class.

use watchord_core::PitchClass;

use crate::spelling::{self, Spelled};

/// Which staff a note belongs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Clef {
    Treble,
    Bass,
}

/// One sounding note, placed on a staff.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StaffNote {
    pub midi_note: u8,
    pub clef: Clef,
    /// Row within the clef's own nine-row staff: `0` is the bottom line, `8`
    /// the top line. Negative or `>8` needs a ledger row.
    pub row: i32,
    pub spelled: Spelled,
}

/// Treble's bottom line: E, MIDI 64 (the standard treble-clef E4).
const TREBLE_REFERENCE_MIDI: u8 = 64;
/// Bass's bottom line: G, MIDI 43 (the standard bass-clef G2).
const BASS_REFERENCE_MIDI: u8 = 43;

fn diatonic_index(midi_note: u8, flat: bool) -> i32 {
    let pc = PitchClass::from_midi_note(midi_note);
    (midi_note as i32) / 12 * 7 + spelling::letter_index(pc, flat)
}

/// The clef and staff row for one sounding MIDI note. `flat` selects the
/// alphabet this note is spelled in — pass `true` only for a pitch class the
/// headline claims and writes flat; an unclaimed pitch class always spells
/// sharp per the spec, so callers pass `false` for those.
pub fn position_of(midi_note: u8, flat: bool) -> StaffNote {
    let clef = if midi_note >= 60 {
        Clef::Treble
    } else {
        Clef::Bass
    };
    let reference = match clef {
        Clef::Treble => TREBLE_REFERENCE_MIDI,
        Clef::Bass => BASS_REFERENCE_MIDI,
    };
    let row = diatonic_index(midi_note, flat) - diatonic_index(reference, false);
    StaffNote {
        midi_note,
        clef,
        row,
        spelled: spelling::spell(PitchClass::from_midi_note(midi_note), flat),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn treble_from_middle_c_up_bass_below() {
        assert_eq!(position_of(60, false).clef, Clef::Treble);
        assert_eq!(position_of(59, false).clef, Clef::Bass);
        assert_eq!(position_of(108, false).clef, Clef::Treble);
        assert_eq!(position_of(21, false).clef, Clef::Bass);
    }

    #[test]
    fn treble_bottom_line_e4_is_row_zero() {
        assert_eq!(position_of(TREBLE_REFERENCE_MIDI, false).row, 0);
    }

    #[test]
    fn treble_top_line_f5_is_row_eight() {
        assert_eq!(position_of(77, false).row, 8);
    }

    #[test]
    fn bass_bottom_line_g2_is_row_zero() {
        assert_eq!(position_of(BASS_REFERENCE_MIDI, false).row, 0);
    }

    #[test]
    fn bass_top_line_a3_is_row_eight() {
        assert_eq!(position_of(57, false).row, 8);
    }

    #[test]
    fn middle_c_sits_two_rows_below_the_treble_staff() {
        assert_eq!(position_of(60, false).row, -2);
    }

    #[test]
    fn a_sharp_shares_its_natural_letters_row() {
        let c = position_of(60, false);
        let c_sharp = position_of(61, false);
        assert_eq!(c.row, c_sharp.row);
        assert_eq!(c_sharp.spelled.letter, 'C');
    }

    #[test]
    fn flat_spelling_moves_the_same_physical_note_up_one_row() {
        // C# (sharp table) sits on C's row; Db (flat table) sits on D's row,
        // one row higher — two different spellings really do draw differently.
        let sharp = position_of(61, false);
        let flat = position_of(61, true);
        assert_eq!(flat.row, sharp.row + 1);
        assert_eq!(flat.spelled.letter, 'D');
    }

    /// A sweep over every MIDI note from 21 to 108 (AC): each note gets exactly
    /// one row, well-defined and total, and stepping to a note whose natural
    /// letter differs flips line/space (row parity), which is what "adjacent
    /// letters alternate line and space" means on a real staff.
    #[test]
    fn sweep_every_midi_note_one_row_each_letters_alternate() {
        let rows: Vec<(u8, i32, char)> = (21u8..=108)
            .map(|n| {
                let p = position_of(n, false);
                (n, p.row, p.spelled.letter)
            })
            .collect();
        assert_eq!(rows.len(), 88, "one staff row computed per note");

        for window in rows.windows(2) {
            let (n0, row0, letter0) = window[0];
            let (n1, row1, letter1) = window[1];
            assert_eq!(n1, n0 + 1);
            if letter0 != letter1 {
                assert_ne!(
                    row0.rem_euclid(2),
                    row1.rem_euclid(2),
                    "MIDI {n0}->{n1} ({letter0}->{letter1}): line/space did not alternate (rows {row0}->{row1})"
                );
            }
        }
    }

    /// Control that the alternation check above can actually fail: two notes a
    /// whole tone apart (different natural letters) must NOT land on the same
    /// parity. If this ever holds, the real sweep's assertion is checking
    /// nothing.
    #[test]
    fn control_two_different_letters_a_step_apart_do_land_on_different_parities() {
        let c = position_of(60, false);
        let d = position_of(62, false);
        assert_ne!(c.spelled.letter, d.spelled.letter);
        assert_ne!(c.row.rem_euclid(2), d.row.rem_euclid(2));
    }

    /// Every claimed pitch class must round-trip: the row math assumes a
    /// chromatic pitch class's letter sits in the SAME physical octave as the
    /// note (see module docs) — verified for every pitch class, both alphabets.
    #[test]
    fn diatonic_index_is_non_decreasing_across_the_full_sweep() {
        for flat in [false, true] {
            let mut previous = diatonic_index(21, flat);
            for n in 22u8..=108 {
                let current = diatonic_index(n, flat);
                assert!(
                    current >= previous,
                    "diatonic index went backwards at MIDI {n} (flat={flat}): {previous} -> {current}"
                );
                previous = current;
            }
        }
    }
}
