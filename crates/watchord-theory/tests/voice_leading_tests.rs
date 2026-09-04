//! Hand-computed pairs, then a brute-force oracle over 200 seeded random
//! pairs of sets sized 2..7 — an independent enumeration (every permutation of
//! the larger set, not the production search's pruned backtracking), so a bug
//! in one is unlikely to be a bug in the other too.

use watchord_core::SoundingSet;
use watchord_theory::voice_leading::{VoiceLeading, voice_leading};

fn set(notes: impl IntoIterator<Item = u8>) -> SoundingSet {
    SoundingSet::new(notes)
}

// MARK: - Hand-computed pairs

#[test]
fn same_chord_held_is_zero_motion() {
    let vl = voice_leading(&set([60, 64, 67]), &set([60, 64, 67])).unwrap();
    assert_eq!(
        vl,
        VoiceLeading {
            total_semitones: 0,
            common_tones_kept: 3,
            largest_move: 0,
        }
    );
}

#[test]
fn parallel_motion_up_a_semitone() {
    let vl = voice_leading(&set([60, 64, 67]), &set([61, 65, 68])).unwrap();
    assert_eq!(
        vl,
        VoiceLeading {
            total_semitones: 3,
            common_tones_kept: 0,
            largest_move: 1,
        }
    );
}

#[test]
fn one_common_tone_kept_two_notes_move() {
    // C E G -> C F A: C stays, E->F (1), G->A (2).
    let vl = voice_leading(&set([60, 64, 67]), &set([60, 65, 69])).unwrap();
    assert_eq!(
        vl,
        VoiceLeading {
            total_semitones: 3,
            common_tones_kept: 1,
            largest_move: 2,
        }
    );
}

#[test]
fn a_note_added_an_octave_up_is_an_extra_note() {
    let vl = voice_leading(&set([60, 64, 67]), &set([60, 64, 67, 72])).unwrap();
    assert_eq!(
        vl,
        VoiceLeading {
            total_semitones: 5, // the added 72's nearest neighbour, 67, is 5 away
            common_tones_kept: 3,
            largest_move: 5,
        }
    );
}

#[test]
fn a_note_dropped_is_an_extra_note_on_the_other_side() {
    let vl = voice_leading(&set([60, 64, 67, 72]), &set([60, 64, 67])).unwrap();
    assert_eq!(
        vl,
        VoiceLeading {
            total_semitones: 5,
            common_tones_kept: 3,
            largest_move: 5,
        }
    );
}

#[test]
fn two_notes_no_common_tone() {
    let vl = voice_leading(&set([60, 64]), &set([58, 61])).unwrap();
    assert_eq!(
        vl,
        VoiceLeading {
            total_semitones: 5,
            common_tones_kept: 0,
            largest_move: 3,
        }
    );
}

#[test]
fn a_single_note_moves_a_tritone() {
    let vl = voice_leading(&set([60]), &set([66])).unwrap();
    assert_eq!(
        vl,
        VoiceLeading {
            total_semitones: 6,
            common_tones_kept: 0,
            largest_move: 6,
        }
    );
}

#[test]
fn one_note_held_two_move_by_a_step() {
    // Cm (C Eb G) -> a chord that keeps the Eb and steps the outer two down.
    let vl = voice_leading(&set([60, 63, 67]), &set([59, 63, 66])).unwrap();
    assert_eq!(
        vl,
        VoiceLeading {
            total_semitones: 2,
            common_tones_kept: 1,
            largest_move: 1,
        }
    );
}

#[test]
fn either_set_empty_has_no_voice_leading() {
    assert_eq!(voice_leading(&set([]), &set([60])), None);
    assert_eq!(voice_leading(&set([60]), &set([])), None);
}

// MARK: - Brute-force oracle over 200 seeded random pairs

/// xorshift64*, seeded once per test run so a failure reproduces.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A value in `lo..=hi`.
    fn range(&mut self, lo: u32, hi: u32) -> u32 {
        lo + (self.next_u64() % u64::from(hi - lo + 1)) as u32
    }
}

fn random_set(rng: &mut Rng) -> SoundingSet {
    let count = rng.range(2, 7);
    let notes: Vec<u8> = (0..count).map(|_| rng.range(21, 108) as u8).collect();
    SoundingSet::new(notes)
}

/// Every permutation of `items[k..]`, calling `visit` on the whole slice at
/// each leaf. Independent of the production search's pruned backtracking —
/// this one just enumerates everything.
fn permute(items: &mut [usize], k: usize, visit: &mut impl FnMut(&[usize])) {
    if k == items.len() {
        visit(items);
        return;
    }
    for i in k..items.len() {
        items.swap(k, i);
        permute(items, k + 1, visit);
        items.swap(k, i);
    }
}

/// The reference implementation: try every permutation of the larger set's
/// indices, take the assignment from its first `n` positions, and keep the
/// cheapest — a different search strategy over the same problem.
fn oracle(previous: &SoundingSet, current: &SoundingSet) -> Option<VoiceLeading> {
    let prev: Vec<i32> = previous.midi_notes().iter().map(|&n| n as i32).collect();
    let cur: Vec<i32> = current.midi_notes().iter().map(|&n| n as i32).collect();
    if prev.is_empty() || cur.is_empty() {
        return None;
    }
    let (smaller, larger) = if prev.len() <= cur.len() {
        (&prev, &cur)
    } else {
        (&cur, &prev)
    };
    let n = smaller.len();
    let mut indices: Vec<usize> = (0..larger.len()).collect();
    let mut best_cost = u32::MAX;
    let mut best_assignment: Vec<usize> = indices[..n].to_vec();
    permute(&mut indices, 0, &mut |perm| {
        let assignment = &perm[..n];
        let cost: u32 = smaller
            .iter()
            .zip(assignment)
            .map(|(&s, &j)| s.abs_diff(larger[j]))
            .sum();
        if cost < best_cost {
            best_cost = cost;
            best_assignment = assignment.to_vec();
        }
    });
    let mut matched_larger = vec![false; larger.len()];
    let mut total = 0u32;
    let mut largest_move = 0u32;
    let mut common_tones_kept = 0usize;
    for (i, &j) in best_assignment.iter().enumerate() {
        let distance = smaller[i].abs_diff(larger[j]);
        total += distance;
        largest_move = largest_move.max(distance);
        if distance == 0 {
            common_tones_kept += 1;
        }
        matched_larger[j] = true;
    }
    for (j, &note) in larger.iter().enumerate() {
        if matched_larger[j] {
            continue;
        }
        let distance = smaller.iter().map(|&s| note.abs_diff(s)).min().unwrap_or(0);
        total += distance;
        largest_move = largest_move.max(distance);
    }
    Some(VoiceLeading {
        total_semitones: total,
        common_tones_kept,
        largest_move,
    })
}

#[test]
fn matches_a_brute_force_oracle_on_two_hundred_random_pairs() {
    let mut rng = Rng(0x5EED_C0FF_EE12_3456);
    for trial in 0..200 {
        let previous = random_set(&mut rng);
        let current = random_set(&mut rng);
        let actual = voice_leading(&previous, &current);
        let expected = oracle(&previous, &current);
        assert_eq!(
            actual,
            expected,
            "trial {trial}: previous={:?} current={:?}",
            previous.midi_notes(),
            current.midi_notes()
        );
    }
}

#[test]
fn the_oracle_itself_can_disagree_with_a_wrong_answer() {
    // The control: an assignment that is deliberately not the minimum must
    // not equal what the oracle finds, or the oracle proves nothing.
    let previous = set([60, 64, 67]);
    let current = set([61, 65, 68]);
    let wrong = VoiceLeading {
        total_semitones: 99,
        common_tones_kept: 0,
        largest_move: 1,
    };
    assert_ne!(oracle(&previous, &current).unwrap(), wrong);
}
