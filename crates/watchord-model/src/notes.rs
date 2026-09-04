//! Search, tags, and sort for the All Notes screen. Spec #9 (ticket #15).
//!
//! Tags are computed from a note's text **at read time** and never stored —
//! the file schema does not change. Search and sort work on the groups
//! [`crate::AppModel`] already loaded; nothing here touches the store.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::NoteGroup;

/// Every `#word` in `text`: a `#` followed by one or more letters, digits, or
/// underscores. Lowercased and deduplicated; `#` alone or a run of punctuation
/// is not a tag.
pub fn tags_in(text: &str) -> BTreeSet<String> {
    let mut tags = BTreeSet::new();
    let mut current = String::new();
    let mut in_tag = false;
    for c in text.chars() {
        if in_tag {
            if c.is_alphanumeric() || c == '_' {
                current.push(c.to_ascii_lowercase());
                continue;
            }
            if !current.is_empty() {
                tags.insert(std::mem::take(&mut current));
            }
            in_tag = false;
        }
        if c == '#' {
            in_tag = true;
            current.clear();
        }
    }
    if in_tag && !current.is_empty() {
        tags.insert(current);
    }
    tags
}

/// Every tag across every note in `group`.
pub fn tags_of(group: &NoteGroup) -> BTreeSet<String> {
    group.notes.iter().flat_map(|n| tags_in(&n.text)).collect()
}

/// Keeps groups whose heading, stored key, a note's text, or a note's tag
/// contains `query`, case-insensitively. An empty or all-whitespace `query`
/// keeps everything.
pub fn filter_groups(groups: Vec<NoteGroup>, query: &str) -> Vec<NoteGroup> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return groups;
    }
    groups
        .into_iter()
        .filter(|group| group_matches(group, &needle))
        .collect()
}

fn group_matches(group: &NoteGroup, needle: &str) -> bool {
    if group.heading.to_lowercase().contains(needle)
        || group.key.raw().to_lowercase().contains(needle)
    {
        return true;
    }
    group.notes.iter().any(|note| {
        note.text.to_lowercase().contains(needle)
            || tags_in(&note.text).iter().any(|tag| tag.contains(needle))
    })
}

/// The three orders All Notes can read in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NotesSort {
    /// The group holding the newest note first — the load order
    /// [`crate::AppModel::group`] already produces.
    #[default]
    Recent,
    /// Alphabetical by the chord's current heading.
    Chord,
    /// Most notes first.
    Count,
}

impl NotesSort {
    /// Every order, in the sequence `s` cycles through.
    pub const ALL: [NotesSort; 3] = [NotesSort::Recent, NotesSort::Chord, NotesSort::Count];

    /// The word the plate and the pipe show: `recent`, `chord`, `count`.
    pub fn label(self) -> &'static str {
        match self {
            NotesSort::Recent => "recent",
            NotesSort::Chord => "chord",
            NotesSort::Count => "count",
        }
    }

    /// The next order in the cycle.
    pub fn next(self) -> Self {
        let index = Self::ALL.iter().position(|s| *s == self).unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }
}

/// Orders `groups` by `sort`. `Recent` is a no-op: the caller's load order
/// already is that order, and re-deriving it here would need the same key
/// twice.
pub fn sort_groups(mut groups: Vec<NoteGroup>, sort: NotesSort) -> Vec<NoteGroup> {
    match sort {
        NotesSort::Recent => {}
        NotesSort::Chord => groups.sort_by(|a, b| {
            a.heading
                .to_lowercase()
                .cmp(&b.heading.to_lowercase())
                .then_with(|| a.key.raw().cmp(b.key.raw()))
        }),
        NotesSort::Count => groups.sort_by(|a, b| {
            b.notes
                .len()
                .cmp(&a.notes.len())
                .then_with(|| a.key.raw().cmp(b.key.raw()))
        }),
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use watchord_core::{ChordKey, ChordNote};

    fn note(id: &str, text: &str) -> ChordNote {
        ChordNote::new(
            id,
            ChordKey::parse("0.4.7").unwrap(),
            "C",
            text,
            std::time::UNIX_EPOCH,
        )
    }

    fn group(heading: &str, key: &str, notes: Vec<ChordNote>) -> NoteGroup {
        NoteGroup {
            key: ChordKey::parse(key).unwrap(),
            heading: heading.to_string(),
            notes,
        }
    }

    #[test]
    fn tags_are_every_hash_word_lowercased_and_deduped() {
        let tags = tags_in("try the #Voicing again, #voicing #open-hand not #1");
        assert_eq!(
            tags,
            BTreeSet::from(["voicing".to_string(), "open".to_string(), "1".to_string()])
        );
    }

    #[test]
    fn a_bare_hash_or_trailing_hash_is_not_a_tag() {
        assert!(tags_in("just a # by itself").is_empty());
        assert!(tags_in("ends in a hash#").is_empty());
        // The control: a real tag in the same string is still found.
        assert_eq!(
            tags_in("a #tag then a bare # then #another"),
            BTreeSet::from(["tag".to_string(), "another".to_string()])
        );
    }

    #[test]
    fn filter_matches_note_text_tag_or_chord_name_case_insensitively() {
        let groups = vec![
            group(
                "C6",
                "0.4.7.9",
                vec![note("1", "sounds like Voodoo #rhodes")],
            ),
            group("F", "0.5.9", vec![note("2", "plain and useful")]),
        ];

        assert_eq!(
            filter_groups(groups.clone(), "VOODOO")
                .into_iter()
                .map(|g| g.heading)
                .collect::<Vec<_>>(),
            ["C6"]
        );
        assert_eq!(
            filter_groups(groups.clone(), "#rhodes")
                .into_iter()
                .map(|g| g.heading)
                .collect::<Vec<_>>(),
            ["C6"]
        );
        assert_eq!(
            filter_groups(groups.clone(), "f")
                .into_iter()
                .map(|g| g.heading)
                .collect::<Vec<_>>(),
            ["F"]
        );
        assert_eq!(
            filter_groups(groups.clone(), "  ").len(),
            2,
            "blank keeps all"
        );
        assert!(filter_groups(groups, "no such thing").is_empty());
    }

    #[test]
    fn sort_orders_hold_on_a_seeded_set() {
        let busy = group(
            "C6",
            "0.4.7.9",
            vec![note("1", "one"), note("2", "two"), note("3", "three")],
        );
        let quiet = group("F", "0.5.9", vec![note("4", "four")]);
        let groups = vec![quiet.clone(), busy.clone()];

        let by_chord = sort_groups(groups.clone(), NotesSort::Chord);
        assert_eq!(
            by_chord
                .iter()
                .map(|g| g.heading.as_str())
                .collect::<Vec<_>>(),
            ["C6", "F"]
        );

        let by_count = sort_groups(groups.clone(), NotesSort::Count);
        assert_eq!(
            by_count
                .iter()
                .map(|g| g.heading.as_str())
                .collect::<Vec<_>>(),
            ["C6", "F"]
        );

        // Recent is a no-op: the input order is preserved exactly.
        let by_recent = sort_groups(groups.clone(), NotesSort::Recent);
        assert_eq!(
            by_recent
                .iter()
                .map(|g| g.heading.as_str())
                .collect::<Vec<_>>(),
            ["F", "C6"]
        );
    }

    #[test]
    fn sort_cycles_through_all_three_and_back() {
        assert_eq!(NotesSort::Recent.next(), NotesSort::Chord);
        assert_eq!(NotesSort::Chord.next(), NotesSort::Count);
        assert_eq!(NotesSort::Count.next(), NotesSort::Recent);
    }
}
