//! Voicing: close, open, drop 2, drop 3, rootless, span, doublings.
//!
//! Spec (#9, Implementation Decisions, "Voicing"): close is every sounding note
//! within one octave of the bass, open is not. Drop 2/3 are recognised by
//! reconstructing the close-position stack of the headline's claimed tones over
//! the actual bass, then checking whether moving its 2nd- or 3rd-from-top note
//! down an octave reproduces exactly what is sounding.

use std::collections::BTreeMap;

use watchord_core::{ChordReading, PitchClass, SoundingSet};

/// How the sounding notes are spread.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoicingShape {
    Close,
    Open,
    Drop2,
    Drop3,
}

impl VoicingShape {
    /// `close`, `open`, `drop 2`, `drop 3` — the plain word for the screen.
    pub fn label(self) -> &'static str {
        match self {
            VoicingShape::Close => "close",
            VoicingShape::Open => "open",
            VoicingShape::Drop2 => "drop 2",
            VoicingShape::Drop3 => "drop 3",
        }
    }
}

/// One pitch class sounding more than once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Doubling {
    pub pitch_class: PitchClass,
    pub count: u8,
}

/// Everything [`voicing_of`] derives from one sounding set against its headline.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Voicing {
    pub shape: VoicingShape,
    /// Highest sounding note minus the bass, in semitones.
    pub span: u8,
    /// True when the headline's root pitch class is not sounding at all.
    pub rootless: bool,
    /// Pitch classes sounding more than once, with their counts. Ascending.
    pub doublings: Vec<Doubling>,
}

/// The root-position close-position stack of `reading`'s claimed pitch
/// classes, built downward from `top` (the actual sounding top note) through
/// the claimed tones in root-relative order (root, then ascending interval
/// from the root). Anchored at the top rather than the bass because dropping
/// an interior note down an octave — the whole point of drop 2/3 — never
/// moves the top note, so the top is the one fixed point a drop construction
/// can be built from without already knowing the answer.
fn close_stack_from_top(reading: &ChordReading, top: i32) -> Vec<i32> {
    let mut ordered: Vec<PitchClass> = reading.pitch_classes.iter().copied().collect();
    ordered.sort_by_key(|&pc| reading.root.interval_to(pc));
    let mut stack = vec![top];
    let mut current = top;
    for &pc in ordered.iter().rev().skip(1) {
        let current_pc = PitchClass::from_midi_note(current as u8);
        let step_down = pc.interval_to(current_pc);
        current -= step_down as i32;
        stack.push(current);
    }
    stack.reverse();
    stack
}

/// Drop 2 or drop 3, or `None` when neither construction reproduces the
/// sounding notes exactly (a doubled or partial voicing never matches).
fn drop_shape(reading: &ChordReading, notes: &[u8]) -> Option<VoicingShape> {
    if notes.len() < 3 {
        return None;
    }
    let top = *notes.last()? as i32;
    let close = close_stack_from_top(reading, top);
    if close.len() != notes.len() {
        return None;
    }
    let sounding: Vec<i32> = notes.iter().map(|&n| n as i32).collect();
    for (from_top, shape) in [(2usize, VoicingShape::Drop2), (3usize, VoicingShape::Drop3)] {
        if close.len() < from_top {
            continue;
        }
        let index = close.len() - from_top;
        let mut candidate = close.clone();
        candidate[index] -= 12;
        if candidate.iter().any(|&n| !(0..=127).contains(&n)) {
            continue;
        }
        candidate.sort_unstable();
        if candidate == sounding {
            return Some(shape);
        }
    }
    None
}

/// Everything the spec's "Voicing" section derives, or `None` when nothing is
/// sounding. Operates on the actual sounding MIDI notes — voicing describes a
/// physical spread, not the pitch-class identity `ChordKey` carries.
pub fn voicing_of(reading: &ChordReading, sounding: &SoundingSet) -> Option<Voicing> {
    let notes = sounding.midi_notes();
    let bass = *notes.first()?;
    let top = *notes.last()?;
    let span = top - bass;
    let shape = if span <= 11 {
        VoicingShape::Close
    } else {
        drop_shape(reading, notes).unwrap_or(VoicingShape::Open)
    };
    let rootless = !sounding.pitch_classes().contains(&reading.root);
    let mut counts: BTreeMap<PitchClass, u8> = BTreeMap::new();
    for &n in notes {
        *counts.entry(PitchClass::from_midi_note(n)).or_insert(0) += 1;
    }
    let doublings = counts
        .into_iter()
        .filter(|&(_, count)| count > 1)
        .map(|(pitch_class, count)| Doubling { pitch_class, count })
        .collect();
    Some(Voicing {
        shape,
        span,
        rootless,
        doublings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use watchord_core::{ChordFit, SpellingOrigin};

    fn reading(root: u8, claimed: &[u8]) -> ChordReading {
        ChordReading {
            display: "X".to_string(),
            spoken: "x".to_string(),
            origin: SpellingOrigin::Headline,
            score: 0,
            pitch_classes: claimed.iter().map(|&v| PitchClass::new(v as i32)).collect(),
            fit: ChordFit::Exact,
            root: PitchClass::new(root as i32),
        }
    }

    #[test]
    fn triad_within_an_octave_is_close() {
        let r = reading(0, &[0, 4, 7]);
        let s = SoundingSet::new([60, 64, 67]);
        let v = voicing_of(&r, &s).unwrap();
        assert_eq!(v.shape, VoicingShape::Close);
        assert_eq!(v.span, 7);
        assert!(!v.rootless);
        assert!(v.doublings.is_empty());
    }

    #[test]
    fn spread_beyond_an_octave_with_no_drop_shape_is_open() {
        // C E G spread over two octaves, not a drop-2/3 rebuild of a close stack.
        let r = reading(0, &[0, 4, 7]);
        let s = SoundingSet::new([48, 64, 79]);
        let v = voicing_of(&r, &s).unwrap();
        assert_eq!(v.shape, VoicingShape::Open);
    }

    #[test]
    fn moving_the_second_from_top_note_down_an_octave_is_drop_2() {
        // Close stack of C E G A over bass 60: 60,64,67,69. Drop the 2nd from
        // top (67, G) down an octave: 55,60,64,69.
        let r = reading(0, &[0, 4, 7, 9]);
        let s = SoundingSet::new([55, 60, 64, 69]);
        let v = voicing_of(&r, &s).unwrap();
        assert_eq!(v.shape, VoicingShape::Drop2);
    }

    #[test]
    fn moving_the_third_from_top_note_down_an_octave_is_drop_3() {
        // Close stack of C E G A over bass 60: 60,64,67,69. Drop the 3rd from
        // top (64, E) down an octave: 52,60,67,69.
        let r = reading(0, &[0, 4, 7, 9]);
        let s = SoundingSet::new([52, 60, 67, 69]);
        let v = voicing_of(&r, &s).unwrap();
        assert_eq!(v.shape, VoicingShape::Drop3);
    }

    #[test]
    fn absent_root_is_rootless() {
        let r = reading(0, &[0, 4, 7, 10]);
        let s = SoundingSet::new([64, 67, 70]); // E G Bb, no C
        let v = voicing_of(&r, &s).unwrap();
        assert!(v.rootless);
    }

    #[test]
    fn an_octave_doubled_root_is_a_doubling() {
        let r = reading(0, &[0, 4, 7]);
        let s = SoundingSet::new([48, 60, 64, 67]);
        let v = voicing_of(&r, &s).unwrap();
        assert_eq!(
            v.doublings,
            vec![Doubling {
                pitch_class: PitchClass::new(0),
                count: 2
            }]
        );
    }

    #[test]
    fn no_sounding_notes_is_none() {
        let r = reading(0, &[0, 4, 7]);
        let s = SoundingSet::silent();
        assert!(voicing_of(&r, &s).is_none());
    }
}
