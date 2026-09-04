//! Voice leading between two sounding sets.
//!
//! Ticket 12, spec 9's Implementation Decisions, "Voice leading": *"Between
//! two sounding sets, the assignment of previous notes to current notes with
//! the smallest total semitone motion, by exhaustive search. Sets are at most
//! seven notes. When sizes differ, the extra notes of the larger set count
//! their nearest neighbour. Reported as total semitones, common tones kept,
//! and the largest single move."*

use watchord_core::SoundingSet;

/// One pair of sounding sets, reduced to how far the hand moved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VoiceLeading {
    /// The sum of every semitone move: the matched pairs' distances plus each
    /// extra note's distance to its nearest neighbour.
    pub total_semitones: u32,
    /// How many matched pairs moved by exactly zero semitones — the same MIDI
    /// note held from one set to the other.
    pub common_tones_kept: usize,
    /// The largest single move among the matched pairs and the extra notes.
    pub largest_move: u32,
}

/// `None` when either set is empty — there is nothing to lead from or to.
///
/// The two sets need not be the same size: when they differ, the smaller set
/// is matched one-to-one into the larger by exhaustive search over every
/// assignment, minimising the summed absolute semitone distance; the larger
/// set's leftover notes each independently take the distance to their nearest
/// neighbour in the smaller set, and that distance is added to the total.
pub fn voice_leading(previous: &SoundingSet, current: &SoundingSet) -> Option<VoiceLeading> {
    let prev: Vec<i32> = previous.midi_notes().iter().map(|&n| n as i32).collect();
    let cur: Vec<i32> = current.midi_notes().iter().map(|&n| n as i32).collect();
    if prev.is_empty() || cur.is_empty() {
        return None;
    }
    Some(compute(&prev, &cur))
}

fn compute(prev: &[i32], cur: &[i32]) -> VoiceLeading {
    let (smaller, larger) = if prev.len() <= cur.len() {
        (prev, cur)
    } else {
        (cur, prev)
    };
    let assignment = best_assignment(smaller, larger);

    let mut matched_larger = vec![false; larger.len()];
    let mut total = 0u32;
    let mut largest_move = 0u32;
    let mut common_tones_kept = 0usize;
    for (i, &j) in assignment.iter().enumerate() {
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
    VoiceLeading {
        total_semitones: total,
        common_tones_kept,
        largest_move,
    }
}

/// The injective function from `smaller`'s indices into `larger`'s indices
/// that minimises the summed absolute semitone distance, found by exhaustive
/// backtracking search over every assignment. `smaller.len() <= larger.len()`
/// and both are at most seven, so this is a few thousand candidates at worst.
fn best_assignment(smaller: &[i32], larger: &[i32]) -> Vec<usize> {
    let mut used = vec![false; larger.len()];
    let mut assignment = vec![0usize; smaller.len()];
    let mut best: Option<(u32, Vec<usize>)> = None;
    search(smaller, larger, 0, 0, &mut used, &mut assignment, &mut best);
    best.map(|(_, assignment)| assignment).unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
fn search(
    smaller: &[i32],
    larger: &[i32],
    index: usize,
    running_cost: u32,
    used: &mut [bool],
    assignment: &mut [usize],
    best: &mut Option<(u32, Vec<usize>)>,
) {
    if let Some((best_cost, _)) = best
        && running_cost >= *best_cost
    {
        // Every remaining note adds at least zero more: this branch can only
        // tie the best found, never beat it. Prune it.
        return;
    }
    if index == smaller.len() {
        *best = Some((running_cost, assignment.to_vec()));
        return;
    }
    for candidate in 0..larger.len() {
        if used[candidate] {
            continue;
        }
        used[candidate] = true;
        assignment[index] = candidate;
        let cost = running_cost + smaller[index].abs_diff(larger[candidate]);
        search(smaller, larger, index + 1, cost, used, assignment, best);
        used[candidate] = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(notes: impl IntoIterator<Item = u8>) -> SoundingSet {
        SoundingSet::new(notes)
    }

    #[test]
    fn either_set_empty_is_no_voice_leading() {
        assert_eq!(voice_leading(&set([]), &set([60])), None);
        assert_eq!(voice_leading(&set([60]), &set([])), None);
        assert_eq!(voice_leading(&set([]), &set([])), None);
    }

    #[test]
    fn the_same_chord_held_moves_nothing() {
        let vl = voice_leading(&set([60, 64, 67]), &set([60, 64, 67])).unwrap();
        assert_eq!(vl.total_semitones, 0);
        assert_eq!(vl.common_tones_kept, 3);
        assert_eq!(vl.largest_move, 0);
    }
}
