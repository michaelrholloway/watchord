//! Ported case for case from note-view `Tests/NoteViewAppTests/AppModelTests.swift`.
//!
//! Everything the skin can get wrong, asserted against the model rather than a
//! frame. There is no terminal in this file, by design: if a rule can only be
//! checked by looking at cells then it is in the wrong place.
//!
//! The Swift `waitUntil` becomes `pump`: the model is polled until a condition
//! holds, on a deadline that is deliberately far longer than the thing waited
//! for, because the engine's suites run beside these on the same cores.

use std::sync::Arc;
use std::time::{Duration, Instant};

use watchord_core::{ChordKey, ChordNote, PedalKind, SoundingSet, SpellingOrigin};
use watchord_model::fakes::{
    InMemoryNoteStore, ScriptedSoundingSetSource, StubAlternate, StubChordNaming,
};
use watchord_model::{AppModel, NotesSort, Screen, seconds_ago};

// MARK: - Fixtures

/// C E G A, C in the bass — ticket 08's worked example.
fn c6() -> SoundingSet {
    SoundingSet::new([60, 64, 67, 69])
}
/// The same four pitch classes, A in the bass. Same `ChordKey`.
fn am7() -> SoundingSet {
    SoundingSet::new([57, 60, 64, 67])
}
/// A different chord entirely: F A C.
fn f_major() -> SoundingSet {
    SoundingSet::new([53, 57, 60])
}
/// Nine distinct pitch classes — one past `tuning::MAXIMUM_PITCH_CLASSES`.
fn too_many() -> SoundingSet {
    SoundingSet::new([60, 61, 62, 63, 64, 65, 66, 67, 68])
}

struct Fixture {
    naming: StubChordNaming,
    source: ScriptedSoundingSetSource,
    store: InMemoryNoteStore,
    model: AppModel,
}

impl Fixture {
    fn new() -> Self {
        Self::build(Vec::new(), None)
    }

    fn seeded(seed: Vec<ChordNote>) -> Self {
        Self::build(seed, None)
    }

    fn failing(message: &str) -> Self {
        Self::build(Vec::new(), Some(message))
    }

    fn build(seed: Vec<ChordNote>, failure: Option<&str>) -> Self {
        let naming = StubChordNaming::new();
        let source = ScriptedSoundingSetSource::default();
        let store = match failure {
            Some(message) => InMemoryNoteStore::failing(seed, message),
            None => InMemoryNoteStore::new(seed),
        };
        naming.stub(
            &c6(),
            StubChordNaming::naming(
                &c6(),
                "C6",
                "C major 6",
                vec![
                    StubAlternate::new("Am7", SpellingOrigin::ReRooted, "A minor 7"),
                    StubAlternate::new("CΔ6", SpellingOrigin::Enharmonic, "C major 6, major 7"),
                ],
            ),
        );
        naming.stub(
            &f_major(),
            StubChordNaming::naming(&f_major(), "F", "F major", vec![]),
        );
        let model = AppModel::new(
            Arc::new(naming.clone()),
            Box::new(source.clone()),
            Arc::new(store.clone()),
        );
        Fixture {
            naming,
            source,
            store,
            model,
        }
    }
}

fn note(text: &str, key: &SoundingSet, spelling: &str, minutes_ago: u64) -> ChordNote {
    ChordNote::new(
        uuid::Uuid::new_v4().to_string().to_uppercase(),
        key.key(),
        spelling,
        text,
        seconds_ago(60 * minutes_ago),
    )
}

/// Polls the model until `condition` holds, or fails after the deadline.
fn pump(model: &mut AppModel, condition: impl Fn(&AppModel) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        model.poll();
        if condition(model) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("condition never became true within 20s");
}

fn alternate_names(model: &AppModel) -> Vec<String> {
    model.alternates().into_iter().map(|a| a.name).collect()
}

fn note_texts(notes: &[ChordNote]) -> Vec<&str> {
    notes.iter().map(|n| n.text.as_str()).collect()
}

// MARK: - The display holds after release

mod release_tests {
    use super::*;

    #[test]
    fn a_chord_names_and_the_released_marker_is_not_showing_while_held() {
        let mut f = Fixture::new();
        f.model.receive(&c6());

        assert_eq!(f.model.headline_text(), "C6");
        assert!(!f.model.is_released());
        assert_eq!(f.model.decline_reason(), None);
    }

    #[test]
    fn lifting_the_keys_leaves_the_chord_on_screen_and_raises_the_marker() {
        let mut f = Fixture::new();
        f.model.receive(&c6());
        f.model.receive(&SoundingSet::silent());

        assert_eq!(
            f.model.headline_text(),
            "C6",
            "the chord must not blank when the hands lift"
        );
        assert!(f.model.is_released());
        // `Am7/C` — C is in the bass and A is not the root of that reading.
        // `CΔ6` keeps its plain name because its root *is* the bass.
        assert_eq!(alternate_names(&f.model), ["Am7/C", "CΔ6"]);
        assert_eq!(f.model.keys_row(), "C3  E3  G3  A3");
    }

    #[test]
    fn release_does_not_disturb_the_note_field_or_the_notes_list() {
        let mut f = Fixture::new();
        f.model.receive(&c6());
        f.model.draft_note_text = "already saved".into();
        f.model.commit_note();
        f.model.draft_note_text = "still typing".into();

        f.model.receive(&SoundingSet::silent());

        assert_eq!(f.model.draft_note_text, "still typing");
        assert_eq!(f.model.note_target_key(), Some(c6().key()));
        assert_eq!(f.model.notes_for_displayed_chord().len(), 1);
    }

    #[test]
    fn a_note_can_be_written_against_a_chord_that_has_already_been_released() {
        let mut f = Fixture::new();
        f.model.receive(&c6());
        f.model.receive(&SoundingSet::silent());

        assert!(f.model.is_released());
        f.model.draft_note_text = "sounds like the Rhodes on Voodoo".into();
        assert!(f.model.can_commit_note());
        f.model.commit_note();

        assert_eq!(
            note_texts(f.model.notes_for_displayed_chord()),
            ["sounds like the Rhodes on Voodoo"]
        );
        assert!(
            f.model.is_released(),
            "committing a note must not un-release the display"
        );
    }

    #[test]
    fn silence_before_anything_is_played_leaves_the_marker_down() {
        let mut f = Fixture::new();
        f.model.receive(&SoundingSet::silent());

        assert!(!f.model.is_released());
        assert!(f.model.displayed().is_none());
        assert_eq!(f.model.headline_text(), "—");
        assert_eq!(f.model.note_target_key(), None);
        assert!(!f.model.can_commit_note());
    }

    #[test]
    fn a_new_chord_clears_the_released_marker() {
        let mut f = Fixture::new();
        f.model.receive(&c6());
        f.model.receive(&SoundingSet::silent());
        f.model.receive(&f_major());

        assert!(!f.model.is_released());
        assert_eq!(f.model.headline_text(), "F");
    }
}

// MARK: - Declining is a normal state

mod decline_tests {
    use super::*;

    #[test]
    fn a_single_note_is_declined_and_still_shows_its_name() {
        let mut f = Fixture::new();
        f.model.receive(&SoundingSet::new([60]));

        assert_eq!(
            f.model.decline_reason().as_deref(),
            Some("one note — not enough to name a chord")
        );
        assert_eq!(
            f.model.headline_text(),
            "C",
            "a lone note still gets its name on screen"
        );
        assert!(!f.model.headline_text().is_empty());
        assert!(f.model.alternates().is_empty());
    }

    #[test]
    fn too_many_notes_are_declined_with_the_count_in_the_reason() {
        let mut f = Fixture::new();
        f.model.receive(&too_many());

        let reason = f.model.decline_reason().expect("a reason");
        assert!(reason.starts_with("too many notes"));
        assert!(reason.contains('9'));
        assert_eq!(f.model.headline_text(), "C  C#  D  D#  E  F  F#  G  G#");
    }

    #[test]
    fn a_set_with_no_honest_reading_is_declined_in_words() {
        let mut f = Fixture::new();
        // Two pitch classes: inside the floor and ceiling, so the stub falls
        // through to "no honest reading" rather than a count-based decline.
        f.model.receive(&SoundingSet::new([60, 61]));

        assert_eq!(
            f.model.decline_reason().as_deref(),
            Some("no honest reading — these keys do not spell a chord in the grammar")
        );
        assert_eq!(f.model.headline_text(), "C  C#");
    }

    #[test]
    fn a_declined_chord_still_holds_after_release_with_its_reason_intact() {
        let mut f = Fixture::new();
        f.model.receive(&too_many());
        f.model.receive(&SoundingSet::silent());

        assert!(f.model.is_released());
        assert!(
            f.model
                .decline_reason()
                .is_some_and(|r| r.starts_with("too many notes"))
        );
    }

    #[test]
    fn a_note_can_still_be_attached_to_a_chord_the_engine_declined_to_name() {
        let mut f = Fixture::new();
        f.model.receive(&too_many());
        f.model.draft_note_text = "whatever this is, it works".into();
        f.model.commit_note();

        assert_eq!(f.model.notes_for_displayed_chord().len(), 1);
        assert_eq!(
            f.model.notes_for_displayed_chord()[0].spelling_when_written,
            "C  C#  D  D#  E  F  F#  G  G#",
            "the record keeps what was actually on screen"
        );
    }
}

// MARK: - Writing notes

mod note_tests {
    use super::*;

    #[test]
    fn committing_shows_the_note_immediately_and_clears_the_field() {
        let mut f = Fixture::new();
        f.model.receive(&c6());
        f.model.draft_note_text = "try it with the 9 on top".into();
        f.model.commit_note();

        assert_eq!(f.model.draft_note_text, "");
        assert_eq!(
            note_texts(f.model.notes_for_displayed_chord()),
            ["try it with the 9 on top"]
        );
        assert_eq!(f.model.note_groups().len(), 1);
        assert_eq!(f.model.note_groups()[0].notes.len(), 1);
    }

    #[test]
    fn the_spelling_that_was_on_screen_is_what_gets_recorded() {
        let mut f = Fixture::new();
        f.model.receive(&c6());
        f.model.draft_note_text = "one".into();
        f.model.commit_note();

        assert_eq!(
            f.model.notes_for_displayed_chord()[0].spelling_when_written,
            "C6"
        );
        assert_eq!(f.model.notes_for_displayed_chord()[0].chord_key, c6().key());
    }

    #[test]
    fn notes_come_back_when_the_same_chord_is_played_in_another_voicing() {
        let mut f = Fixture::new();
        f.model.receive(&c6());
        f.model.draft_note_text = "the Voodoo one".into();
        f.model.commit_note();

        f.model.receive(&f_major());
        assert!(f.model.notes_for_displayed_chord().is_empty());

        // Same four pitch classes, A in the bass: a different name, same key.
        f.model.receive(&am7());
        assert_eq!(
            note_texts(f.model.notes_for_displayed_chord()),
            ["the Voodoo one"]
        );
    }

    #[test]
    fn whitespace_only_text_is_not_a_note() {
        let mut f = Fixture::new();
        f.model.receive(&c6());
        f.model.draft_note_text = "   \n ".into();
        assert!(!f.model.can_commit_note());
        f.model.commit_note();

        assert!(f.model.notes_for_displayed_chord().is_empty());
        assert_eq!(
            f.model.draft_note_text, "   \n ",
            "a refused commit does not eat the text"
        );
    }

    #[test]
    fn the_store_is_never_even_asked_to_save_against_the_empty_key() {
        // The real store refuses `ChordKey::empty()`, because such a note encodes
        // to `""` and is refused on the way back in. So "no note appeared" is
        // not enough: an unguarded call that failed and got swallowed looks the
        // same. Assert the call never happened.
        let mut f = Fixture::new();
        f.model.draft_note_text = "into the void".into();
        assert_eq!(f.model.note_target_key(), None);
        assert!(!f.model.can_commit_note());
        f.model.commit_note();

        assert_eq!(f.store.add_call_count(), 0);
        assert!(f.model.note_groups().is_empty());
        assert_eq!(f.model.status_message(), None, "a refusal is not an error");
    }

    #[test]
    fn released_silence_does_not_re_point_the_field_at_the_empty_key() {
        let mut f = Fixture::new();
        f.model.receive(&c6());
        f.model.receive(&SoundingSet::silent());
        f.model.draft_note_text = "after the fact".into();
        f.model.commit_note();

        assert_eq!(f.store.add_call_count(), 1);
        assert_eq!(
            note_texts(f.model.notes_for_displayed_chord()),
            ["after the fact"]
        );
        assert_eq!(f.model.status_message(), None);
    }

    #[test]
    fn deleting_removes_the_note_from_both_screens() {
        let mut f = Fixture::new();
        f.model.receive(&c6());
        f.model.draft_note_text = "one".into();
        f.model.commit_note();
        f.model.draft_note_text = "two".into();
        f.model.commit_note();

        let victim = f.model.notes_for_displayed_chord()[0].id.clone();
        f.model.delete_note(&victim);

        assert_eq!(f.model.notes_for_displayed_chord().len(), 1);
        let total: usize = f.model.note_groups().iter().map(|g| g.notes.len()).sum();
        assert_eq!(total, 1);
    }
}

// MARK: - Editing an existing note

mod edit_tests {
    use super::*;

    #[test]
    fn editing_loads_the_text_and_commit_updates_rather_than_adds() {
        let mut f = Fixture::new();
        f.model.receive(&c6());
        f.model.draft_note_text = "first draft".into();
        f.model.commit_note();
        let id = f.model.notes_for_displayed_chord()[0].id.clone();

        f.model.begin_edit(&id);
        assert_eq!(f.model.draft_note_text, "first draft");
        assert!(f.model.is_editing());

        f.model.draft_note_text = "first draft, revised".into();
        f.model.commit_note();

        assert!(!f.model.is_editing(), "commit leaves editing");
        assert_eq!(f.model.draft_note_text, "");
        assert_eq!(f.store.add_call_count(), 1, "no new note was added");
        assert_eq!(
            note_texts(f.model.notes_for_displayed_chord()),
            ["first draft, revised"]
        );
        assert_eq!(f.model.notes_for_displayed_chord()[0].id, id, "same note");
    }

    #[test]
    fn editing_needs_no_live_chord_to_commit() {
        let mut f = Fixture::new();
        f.model.receive(&c6());
        f.model.draft_note_text = "written while sounding".into();
        f.model.commit_note();
        let id = f.model.notes_for_displayed_chord()[0].id.clone();

        // Nothing is sounding now: an ordinary add would be refused.
        f.model.receive(&SoundingSet::silent());
        f.model.receive(&too_many());
        assert!(
            f.model.note_target_key().is_none() || f.model.notes_for_displayed_chord().is_empty()
        );

        f.model.begin_edit(&id);
        assert!(f.model.can_commit_note());
    }

    #[test]
    fn edit_replaces_text_and_keeps_id_created_at_and_chord_key() {
        let mut f = Fixture::new();
        f.model.receive(&c6());
        f.model.draft_note_text = "keep my identity".into();
        f.model.commit_note();
        let original = f.model.notes_for_displayed_chord()[0].clone();

        f.model.begin_edit(&original.id);
        f.model.draft_note_text = "identity kept, text changed".into();
        f.model.commit_note();

        let after = &f.model.notes_for_displayed_chord()[0];
        assert_eq!(after.id, original.id);
        assert_eq!(after.chord_key, original.chord_key);
        assert_eq!(after.created_at, original.created_at);
        assert_eq!(after.spelling_when_written, original.spelling_when_written);
        assert_eq!(after.text, "identity kept, text changed");
    }

    #[test]
    fn cancel_edit_clears_the_field_and_leaves_the_note_untouched() {
        let mut f = Fixture::new();
        f.model.receive(&c6());
        f.model.draft_note_text = "original".into();
        f.model.commit_note();
        let id = f.model.notes_for_displayed_chord()[0].id.clone();

        f.model.begin_edit(&id);
        f.model.draft_note_text = "changed my mind".into();
        f.model.cancel_edit();

        assert!(!f.model.is_editing());
        assert_eq!(f.model.draft_note_text, "");
        assert_eq!(
            note_texts(f.model.notes_for_displayed_chord()),
            ["original"]
        );
    }

    #[test]
    fn a_blank_edit_is_refused_like_a_blank_add() {
        let mut f = Fixture::new();
        f.model.receive(&c6());
        f.model.draft_note_text = "has text".into();
        f.model.commit_note();
        let id = f.model.notes_for_displayed_chord()[0].id.clone();

        f.model.begin_edit(&id);
        f.model.draft_note_text = "   ".into();
        assert!(!f.model.can_commit_note());
        f.model.commit_note();

        assert!(f.model.is_editing(), "the blank commit did not go through");
        assert_eq!(
            note_texts(f.model.notes_for_displayed_chord()),
            ["has text"]
        );
    }
}

// MARK: - Search, tags, and sort

mod search_and_sort_tests {
    use super::*;

    fn seeded_two_chords() -> Fixture {
        Fixture::seeded(vec![
            note("sounds like the Rhodes #electric", &c6(), "C6", 100),
            note("try the 9 on top #electric #voicing", &c6(), "Am7", 90),
            note("plain and useful", &f_major(), "F", 5),
        ])
    }

    #[test]
    fn search_filters_groups_by_note_text_live_as_typed() {
        let mut f = seeded_two_chords();
        f.model.screen = Screen::AllNotes;

        f.model.search_text = "rhodes".into();
        let headings: Vec<String> = f
            .model
            .frame()
            .groups
            .iter()
            .map(|g| g.heading.clone())
            .collect();
        assert_eq!(headings, ["C6"]);

        f.model.search_text.clear();
        assert_eq!(
            f.model.frame().groups.len(),
            2,
            "an empty query keeps everything"
        );
    }

    #[test]
    fn search_filters_by_tag() {
        let mut f = seeded_two_chords();
        f.model.search_text = "#voicing".into();
        assert_eq!(f.model.frame().groups.len(), 1);
        assert_eq!(f.model.frame().groups[0].heading, "C6");
    }

    #[test]
    fn search_filters_by_chord_name() {
        let mut f = seeded_two_chords();
        f.model.search_text = "F".into();
        let headings: Vec<String> = f
            .model
            .frame()
            .groups
            .iter()
            .map(|g| g.heading.clone())
            .collect();
        assert_eq!(headings, ["F"]);
    }

    #[test]
    fn each_sort_order_holds_on_a_seeded_set() {
        let mut f = seeded_two_chords();
        assert_eq!(f.model.notes_sort, NotesSort::Recent);
        let recent: Vec<String> = f
            .model
            .frame()
            .groups
            .iter()
            .map(|g| g.heading.clone())
            .collect();
        assert_eq!(recent, ["F", "C6"], "F's note is newest");

        f.model.cycle_notes_sort();
        assert_eq!(f.model.notes_sort, NotesSort::Chord);
        let by_chord: Vec<String> = f
            .model
            .frame()
            .groups
            .iter()
            .map(|g| g.heading.clone())
            .collect();
        assert_eq!(by_chord, ["C6", "F"], "alphabetical");

        f.model.cycle_notes_sort();
        assert_eq!(f.model.notes_sort, NotesSort::Count);
        let by_count: Vec<String> = f
            .model
            .frame()
            .groups
            .iter()
            .map(|g| g.heading.clone())
            .collect();
        assert_eq!(by_count, ["C6", "F"], "two notes beats one");

        f.model.cycle_notes_sort();
        assert_eq!(f.model.notes_sort, NotesSort::Recent, "cycles back");
    }

    #[test]
    fn the_frame_carries_the_live_search_text_and_sort_choice() {
        let mut f = seeded_two_chords();
        f.model.search_text = "rhodes".into();
        f.model.cycle_notes_sort();
        let frame = f.model.frame();
        assert_eq!(frame.search, "rhodes");
        assert_eq!(frame.notes_sort, NotesSort::Chord);
    }
}

// MARK: - All Notes

mod grouping_tests {
    use super::*;

    #[test]
    fn grouped_by_chord_newest_note_first_newest_group_first() {
        let f = Fixture::seeded(vec![
            note("old C6", &c6(), "C6", 100),
            note("new C6", &c6(), "Am7", 90),
            note("the F one", &f_major(), "F", 5),
        ]);

        assert_eq!(f.model.note_groups().len(), 2);
        assert_eq!(
            f.model.note_groups()[0].key,
            f_major().key(),
            "newest group leads"
        );
        assert_eq!(
            note_texts(&f.model.note_groups()[1].notes),
            ["new C6", "old C6"]
        );
    }

    #[test]
    fn the_heading_is_re_derived_through_the_engine_not_read_off_a_note() {
        // Every stored note on this chord says `Am7`. The heading must still be
        // whatever the engine names the key today.
        let f = Fixture::seeded(vec![
            note("a", &c6(), "Am7", 10),
            note("b", &c6(), "Am7", 5),
        ]);

        assert_eq!(f.model.note_groups()[0].heading, "C6");
        assert!(
            f.model.note_groups()[0]
                .notes
                .iter()
                .all(|n| n.spelling_when_written == "Am7")
        );
    }

    #[test]
    fn a_key_the_engine_cannot_name_still_gets_a_heading_and_its_notes() {
        let f = Fixture::seeded(vec![note("odd", &too_many(), "?", 1)]);

        assert_eq!(f.model.note_groups().len(), 1);
        assert_eq!(f.model.note_groups()[0].heading, too_many().key().raw());
        assert_eq!(f.model.note_groups()[0].notes.len(), 1);
    }
}

// MARK: - Screens

mod screen_tests {
    use super::*;

    #[test]
    fn switching_screens_and_back_preserves_every_bit_of_now_playing_state() {
        let mut f = Fixture::new();
        f.model.receive(&c6());
        f.model.draft_note_text = "half-typed".into();
        f.model.commit_note();
        f.model.draft_note_text = "half-typed again".into();
        f.model.receive(&SoundingSet::silent());

        f.model.screen = Screen::AllNotes;
        f.model.screen = Screen::NowPlaying;

        assert_eq!(f.model.headline_text(), "C6");
        assert!(f.model.is_released());
        assert_eq!(f.model.draft_note_text, "half-typed again");
        assert_eq!(f.model.notes_for_displayed_chord().len(), 1);
        assert_eq!(f.model.alternates().len(), 2);
    }

    #[test]
    fn now_playing_is_the_default_screen() {
        assert_eq!(Fixture::new().model.screen, Screen::NowPlaying);
        assert_eq!(Screen::ALL, [Screen::NowPlaying, Screen::AllNotes]);
    }
}

// MARK: - Failure surfaces

mod failure_tests {
    use super::*;

    #[test]
    fn an_input_that_will_not_start_says_so_and_the_app_carries_on() {
        let f = Fixture::new();
        let dead = ScriptedSoundingSetSource::failing_to_start("boom");
        let mut model = AppModel::new(
            Arc::new(f.naming.clone()),
            Box::new(dead),
            Arc::new(f.store.clone()),
        );
        model.start();

        assert!(
            model
                .status_message()
                .is_some_and(|m| m.contains("MIDI input unavailable"))
        );
        model.receive(&c6());
        assert_eq!(
            model.headline_text(),
            "C6",
            "the rest of the app still works"
        );
        model.stop();
    }

    #[test]
    fn a_store_that_fails_on_read_reports_it_and_shows_an_empty_list() {
        let f = Fixture::failing("boom");

        assert!(f.model.note_groups().is_empty());
        assert!(
            f.model
                .status_message()
                .is_some_and(|m| m.contains("Could not read notes"))
        );
    }

    #[test]
    fn a_store_that_fails_on_write_reports_it_and_keeps_the_text() {
        let mut f = Fixture::failing("boom");
        f.model.receive(&c6());
        f.model.draft_note_text = "will not save".into();
        f.model.commit_note();

        assert!(
            f.model
                .status_message()
                .is_some_and(|m| m.contains("Could not save the note"))
        );
        assert_eq!(
            f.model.draft_note_text, "will not save",
            "a failed save does not eat the text"
        );
    }

    #[test]
    fn a_successful_save_does_not_erase_the_composition_banner() {
        let mut f = Fixture::new();
        f.model.announce("running on fakes");
        f.model.receive(&c6());
        f.model.draft_note_text = "fine".into();
        f.model.commit_note();

        assert_eq!(f.model.banner(), Some("running on fakes"));
        assert_eq!(f.model.status_message(), None);
    }
}

// MARK: - The stream, end to end through the model

mod stream_tests {
    use super::*;

    #[test]
    fn start_consumes_the_source_and_the_display_follows_it() {
        let mut f = Fixture::new();
        f.model.start();
        assert!(f.source.did_start());

        f.source.send(c6());
        pump(&mut f.model, |m| m.headline_text() == "C6");
        assert!(!f.model.is_released());

        f.source.release();
        pump(&mut f.model, |m| m.is_released());
        assert_eq!(
            f.model.headline_text(),
            "C6",
            "still there with the hands off the keys"
        );

        f.model.stop();
        assert!(f.source.did_stop());
    }

    #[test]
    fn start_twice_attaches_one_listener_not_two() {
        let mut f = Fixture::new();
        f.model.start();
        f.model.start();
        f.source.send(c6());
        pump(&mut f.model, |m| m.headline_text() == "C6");
        f.model.stop();
    }
}

// MARK: - What is plugged in

/// The input plate used to read `MIDI` off `banner == nil` — a build flag. It
/// said `MIDI` with nothing plugged in. These assert the replacement: a live
/// device set, off the seam.
mod connected_input_tests {
    use super::*;

    #[test]
    fn nothing_attached_says_so_and_does_not_say_midi() {
        let f = Fixture::new();

        assert!(f.model.connected_inputs().is_empty());
        assert!(f.model.has_no_input());
        assert_eq!(f.model.input_label(), "none");
        assert_ne!(f.model.input_label(), "midi");
    }

    #[test]
    fn one_device_shows_its_own_name() {
        let mut f = Fixture::new();
        f.model.start();
        f.source.attach(&["Nord Stage 3"]);
        pump(&mut f.model, |m| !m.connected_inputs().is_empty());

        assert_eq!(f.model.input_label(), "Nord Stage 3");
        assert!(!f.model.has_no_input());
        f.model.stop();
    }

    #[test]
    fn several_devices_are_counted_rather_than_listed() {
        let mut f = Fixture::new();
        f.model.start();
        f.source
            .attach(&["Nord Stage 3", "IAC Driver Bus 1", "Launchkey"]);
        pump(&mut f.model, |m| m.connected_inputs().len() == 3);

        assert_eq!(f.model.input_label(), "3 devices");
        f.model.stop();
    }

    #[test]
    fn unplugging_the_last_device_goes_back_to_none() {
        let mut f = Fixture::new();
        f.model.start();
        f.source.attach(&["Nord Stage 3"]);
        pump(&mut f.model, |m| m.input_label() == "Nord Stage 3");

        f.source.attach(&[]);
        pump(&mut f.model, |m| m.connected_inputs().is_empty());

        assert_eq!(f.model.input_label(), "none");
        assert!(f.model.has_no_input());
        f.model.stop();
    }

    #[test]
    fn a_fake_graph_says_fake_whatever_is_attached() {
        let mut f = Fixture::new();
        f.model
            .announce("fake input — no MIDI hardware is being read");
        f.model.start();
        f.source.attach(&["Nord Stage 3"]);
        pump(&mut f.model, |m| !m.connected_inputs().is_empty());

        assert_eq!(
            f.model.input_label(),
            "fake",
            "a fake run must never look like a real one"
        );
        assert!(!f.model.has_no_input());
        f.model.stop();
    }

    #[test]
    fn playing_a_chord_does_not_change_what_the_plate_says() {
        // The discriminator: this is what would catch a plate quietly
        // re-derived from the sounding set.
        let mut f = Fixture::new();
        f.model.start();
        f.source.attach(&["Nord Stage 3"]);
        pump(&mut f.model, |m| m.input_label() == "Nord Stage 3");

        f.model.receive(&c6());
        assert_eq!(f.model.headline_text(), "C6");
        assert_eq!(f.model.input_label(), "Nord Stage 3");

        f.model.receive(&SoundingSet::silent());
        assert!(f.model.is_released());
        assert_eq!(
            f.model.input_label(),
            "Nord Stage 3",
            "releasing the keys unplugs nothing"
        );
        f.model.stop();
    }
}

// MARK: - The folio

mod folio_tests {
    use super::*;

    #[test]
    fn the_two_counts_are_genuinely_different_numbers() {
        let mut f = Fixture::seeded(vec![
            note("one", &c6(), "C6", 3),
            note("two", &f_major(), "F", 2),
            note("three", &f_major(), "F", 1),
        ]);
        f.model.receive(&c6());

        assert_eq!(f.model.notes_for_displayed_chord().len(), 1);
        assert_eq!(f.model.total_note_count(), 3);
    }

    #[test]
    fn the_total_counts_every_note_in_every_group() {
        let mut f = Fixture::new();
        assert_eq!(f.model.total_note_count(), 0);

        f.model.receive(&c6());
        f.model.draft_note_text = "first".into();
        f.model.commit_note();
        f.model.draft_note_text = "second".into();
        f.model.commit_note();
        f.model.receive(&f_major());
        f.model.draft_note_text = "third".into();
        f.model.commit_note();

        assert_eq!(f.model.total_note_count(), 3);
        assert_eq!(
            f.model.notes_for_displayed_chord().len(),
            1,
            "and the per-chord count is not it"
        );
    }
}

// MARK: - A device name is elided, never allowed to run away

mod input_label_width_tests {
    use super::*;

    #[test]
    fn a_short_name_is_left_exactly_as_the_device_gave_it() {
        let mut f = Fixture::new();
        f.model.start();
        f.source.attach(&["Nord Stage 3"]);
        pump(&mut f.model, |m| !m.connected_inputs().is_empty());

        assert_eq!(
            f.model.input_label(),
            "Nord Stage 3",
            "no ellipsis on a name that fits"
        );
        f.model.stop();
    }

    #[test]
    fn a_long_name_is_cut_to_the_limit_and_marked_as_cut() {
        let long = "Some Extremely Verbose Manufacturer Keyboard Controller Mk II";
        let mut f = Fixture::new();
        f.model.start();
        f.source.attach(&[long]);
        pump(&mut f.model, |m| !m.connected_inputs().is_empty());

        assert_eq!(f.model.input_label().chars().count(), 28);
        assert!(f.model.input_label().ends_with('…'));
        assert_ne!(f.model.input_label(), long);
        // The stored fact is untouched — only the plate's copy is shortened.
        assert_eq!(f.model.connected_inputs(), [long.to_string()]);
        f.model.stop();
    }

    #[test]
    fn the_boundary_exactly_at_the_limit_is_not_elided() {
        let exact = "x".repeat(28);
        assert_eq!(AppModel::elided(&exact, 28), exact);
        assert_eq!(
            AppModel::elided(&format!("{exact}x"), 28).chars().count(),
            28
        );
    }
}

// MARK: - Grouping, on its own

#[test]
fn group_ties_between_groups_break_on_the_key_text() {
    let when = seconds_ago(60);
    let notes = vec![
        ChordNote::new("A", ChordKey::parse("0.4.7.9").unwrap(), "C6", "a", when),
        ChordNote::new("B", ChordKey::parse("0.4.7").unwrap(), "C", "b", when),
    ];
    let groups = AppModel::group(notes, &StubChordNaming::new());
    let keys: Vec<&str> = groups.iter().map(|g| g.key.raw()).collect();
    assert_eq!(keys, ["0.4.7", "0.4.7.9"]);
}

// MARK: - History: filling, stepping, and voice leading (ticket 12)

mod history_tests {
    use std::sync::Mutex;
    use std::time::UNIX_EPOCH;

    use watchord_model::{Clock, FrameState, HistoryStep};

    use super::*;

    /// A model stubbed the same way [`Fixture`] is, but whose history is
    /// stamped from `seconds`, which the test advances by hand between
    /// pushes instead of racing the wall clock.
    fn model_with_controlled_clock() -> (AppModel, ScriptedSoundingSetSource, Arc<Mutex<u64>>) {
        let naming = StubChordNaming::new();
        naming.stub(
            &c6(),
            StubChordNaming::naming(
                &c6(),
                "C6",
                "C major 6",
                vec![StubAlternate::new(
                    "Am7",
                    SpellingOrigin::ReRooted,
                    "A minor 7",
                )],
            ),
        );
        naming.stub(
            &f_major(),
            StubChordNaming::naming(&f_major(), "F", "F major", vec![]),
        );
        let source = ScriptedSoundingSetSource::default();
        let store = InMemoryNoteStore::default();
        let seconds = Arc::new(Mutex::new(0u64));
        let for_clock = Arc::clone(&seconds);
        let clock: Clock =
            Box::new(move || UNIX_EPOCH + Duration::from_secs(*for_clock.lock().unwrap()));
        let mut model = AppModel::with_clock(
            Arc::new(naming),
            Box::new(source.clone()),
            Arc::new(store),
            clock,
        );
        model.start();
        (model, source, seconds)
    }

    #[test]
    fn history_fills_to_capacity_and_drops_the_oldest() {
        let (mut model, source, seconds) = model_with_controlled_clock();
        // 66 distinct, strictly ascending single notes: MIDI 21..=86.
        for offset in 0..66u64 {
            *seconds.lock().unwrap() = offset;
            let note = 21u8 + offset as u8;
            source.send_midi_notes([note]);
            let expected_key = SoundingSet::new([note]).key();
            pump(&mut model, |m| {
                m.frame()
                    .history
                    .last()
                    .is_some_and(|entry| entry.key == expected_key)
            });
        }
        assert_eq!(model.history_len(), 64);
        let frame = model.frame();
        assert_eq!(frame.history.len(), 64);
        // The first two pushes (MIDI 21, 22) were dropped; the oldest
        // survivor is the third push, MIDI 23.
        assert_eq!(frame.history[0].key, SoundingSet::new([23u8]).key());
        assert_eq!(
            frame.history[63].key,
            SoundingSet::new([21u8 + 65]).key(),
            "the newest push is still the last entry"
        );
    }

    #[test]
    fn stepping_back_and_forward_moves_through_history_and_a_new_set_returns_to_live() {
        let (mut model, source, seconds) = model_with_controlled_clock();
        *seconds.lock().unwrap() = 0;
        source.send_midi_notes([60, 64, 67, 69]); // C6
        pump(&mut model, |m| m.headline_text() == "C6");
        *seconds.lock().unwrap() = 5;
        source.send_midi_notes([53, 57, 60]); // F major
        pump(&mut model, |m| m.headline_text() == "F");
        *seconds.lock().unwrap() = 12;
        source.send_midi_notes([50]); // a third, declined entry
        pump(&mut model, |m| m.headline_text() == "D");

        assert_eq!(model.headline_text(), "D");
        assert_eq!(model.frame().history_step, None, "starts live");

        model.step_history_back();
        let frame = model.frame();
        assert_eq!(frame.headline_text, "F");
        assert_eq!(
            frame.state,
            FrameState::Released,
            "a stepped entry draws released"
        );
        assert_eq!(
            frame.history_step,
            Some(HistoryStep {
                index: 2,
                total: 64
            })
        );

        model.step_history_back();
        let frame = model.frame();
        assert_eq!(frame.headline_text, "C6");
        assert_eq!(
            frame.history_step,
            Some(HistoryStep {
                index: 1,
                total: 64
            })
        );

        // Already at the oldest entry: stepping back again is a no-op.
        model.step_history_back();
        assert_eq!(
            model.frame().history_step,
            Some(HistoryStep {
                index: 1,
                total: 64
            })
        );

        model.step_history_forward();
        assert_eq!(model.frame().headline_text, "F");
        model.step_history_forward();
        let frame = model.frame();
        assert_eq!(frame.headline_text, "D");
        assert_eq!(
            frame.history_step, None,
            "forward past the newest returns to live"
        );
        assert_eq!(frame.state, FrameState::Held);

        // Stepping away, then a new sounding set: the display returns to live.
        model.step_history_back();
        assert!(model.frame().history_step.is_some());
        *seconds.lock().unwrap() = 20;
        source.send_midi_notes([62, 65, 69]); // D minor-ish, not stubbed
        pump(&mut model, |m| m.frame().history.len() == 4);
        assert_eq!(model.frame().history_step, None);
    }

    #[test]
    fn voice_leading_rides_along_each_entry_against_the_one_before_it() {
        let (mut model, source, seconds) = model_with_controlled_clock();
        *seconds.lock().unwrap() = 0;
        source.send_midi_notes([60, 64, 67, 69]); // C6
        pump(&mut model, |m| m.headline_text() == "C6");
        *seconds.lock().unwrap() = 7;
        source.send_midi_notes([53, 57, 60]); // F major
        pump(&mut model, |m| m.headline_text() == "F");

        let frame = model.frame();
        assert_eq!(frame.history.len(), 2);
        assert_eq!(
            frame.history[0].voice_leading, None,
            "nothing came before it"
        );
        assert_eq!(frame.history[0].seconds_since_previous, None);
        assert_eq!(frame.history[1].seconds_since_previous, Some(7));
        let vl = frame.history[1]
            .voice_leading
            .expect("a previous entry to lead from");
        // The exact numbers are the theory crate's own tests; here we only
        // assert the model actually attaches one.
        assert!(vl.total_semitones > 0);
    }

    #[test]
    fn the_history_strip_reads_oldest_first_matching_context_md() {
        let (mut model, source, seconds) = model_with_controlled_clock();
        *seconds.lock().unwrap() = 0;
        source.send_midi_notes([60, 64, 67, 69]);
        pump(&mut model, |m| m.headline_text() == "C6");
        *seconds.lock().unwrap() = 1;
        source.send_midi_notes([53, 57, 60]);
        pump(&mut model, |m| m.headline_text() == "F");

        let frame = model.frame();
        let names: Vec<&str> = frame.history.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["C6", "F"], "oldest first, newest last");
    }
}

// MARK: - Pedals, settle, the input picker, and arpeggio (ticket 13)

mod pedal_tests {
    use super::*;

    #[test]
    fn each_pedal_plate_follows_its_own_control_event() {
        let mut f = Fixture::new();
        f.model.start();
        assert!(!f.model.is_sustain_down());
        assert!(!f.model.is_sostenuto_down());
        assert!(!f.model.is_soft_down());

        f.source.pedal(PedalKind::Sustain, true);
        pump(&mut f.model, |m| m.is_sustain_down());
        f.source.pedal(PedalKind::Soft, true);
        pump(&mut f.model, |m| m.is_soft_down());

        assert!(f.model.is_sustain_down());
        assert!(f.model.is_soft_down());
        // The control: a plate this test never touched stays down.
        assert!(!f.model.is_sostenuto_down());

        f.source.pedal(PedalKind::Sustain, false);
        pump(&mut f.model, |m| !m.is_sustain_down());
        assert!(
            f.model.is_soft_down(),
            "lifting sustain must not touch soft"
        );
        f.model.stop();
    }
}

mod settle_tests {
    use super::*;

    #[test]
    fn settle_moves_in_10ms_steps_and_clamps_at_10_and_500() {
        let mut f = Fixture::new();
        assert_eq!(
            f.model.settle_ms(),
            60,
            "tuning::SETTLE_INTERVAL's own value"
        );

        f.model.decrease_settle();
        assert_eq!(f.model.settle_ms(), 50);
        f.model.increase_settle();
        f.model.increase_settle();
        assert_eq!(f.model.settle_ms(), 70);

        for _ in 0..20 {
            f.model.decrease_settle();
        }
        assert_eq!(f.model.settle_ms(), 10, "clamped at the floor");
        f.model.decrease_settle();
        assert_eq!(f.model.settle_ms(), 10, "one more step still clamps");

        for _ in 0..60 {
            f.model.increase_settle();
        }
        assert_eq!(f.model.settle_ms(), 500, "clamped at the ceiling");
        f.model.increase_settle();
        assert_eq!(f.model.settle_ms(), 500, "one more step still clamps");

        // The control: the source actually heard the calls, not just the
        // model's own field.
        let calls = f.source.settle_calls();
        assert!(!calls.is_empty());
        assert_eq!(*calls.last().unwrap(), Duration::from_millis(500));
    }
}

mod input_picker_tests {
    use super::*;

    #[test]
    fn choosing_an_input_switches_the_source() {
        let mut f = Fixture::new();
        assert_eq!(f.source.last_selected_input(), None, "never called yet");

        f.model.select_input(Some("Nord Stage 3".to_string()));
        assert_eq!(
            f.source.last_selected_input(),
            Some(Some("Nord Stage 3".to_string()))
        );

        f.model.select_input(None);
        assert_eq!(
            f.source.last_selected_input(),
            Some(None),
            "clearing the restriction is a real call, not a no-op"
        );
    }
}

mod arpeggio_tests {
    use super::*;

    #[test]
    fn sostenuto_toggles_arpeggio_on_its_down_edge_only() {
        let mut f = Fixture::new();
        f.model.start();
        assert!(!f.model.is_arpeggio());

        f.source.pedal(PedalKind::Sostenuto, true);
        pump(&mut f.model, |m| m.is_arpeggio());

        f.source.pedal(PedalKind::Sostenuto, false);
        pump(&mut f.model, |m| !m.is_sostenuto_down());
        assert!(
            f.model.is_arpeggio(),
            "the release edge must not toggle again"
        );

        f.source.pedal(PedalKind::Sostenuto, true);
        pump(&mut f.model, |m| !m.is_arpeggio());
        f.model.stop();
    }

    /// A roll with overlap — 60 stays down while 64 joins, then 60 lifts while
    /// 67 joins — is how "one at a time" is actually played. Without arpeggio
    /// mode the third snapshot alone would show only 64 and 67.
    #[test]
    fn arpeggio_keeps_a_note_the_source_has_already_dropped() {
        let mut f = Fixture::new();
        f.model.start();
        f.source.pedal(PedalKind::Sostenuto, true);
        pump(&mut f.model, |m| m.is_arpeggio());

        f.model.receive(&SoundingSet::new([60]));
        f.model.receive(&SoundingSet::new([60, 64]));
        f.model.receive(&SoundingSet::new([64, 67])); // 60 released by the source
        assert_eq!(
            f.model.keys_row(),
            "C3  E3  G3",
            "60 must still show — a note-off does not remove it"
        );
        f.model.stop();
    }

    #[test]
    fn the_accumulator_settles_on_a_full_release_and_the_next_note_starts_fresh() {
        let mut f = Fixture::new();
        f.model.start();
        f.source.pedal(PedalKind::Sostenuto, true);
        pump(&mut f.model, |m| m.is_arpeggio());

        f.model.receive(&SoundingSet::new([60]));
        f.model.receive(&SoundingSet::new([60, 64]));
        f.model.receive(&SoundingSet::silent()); // every key up: "a key clears it"
        f.model.receive(&SoundingSet::new([72]));

        assert_eq!(
            f.model.keys_row(),
            "C4",
            "the full release cleared the accumulator"
        );
        f.model.stop();
    }

    #[test]
    fn lifting_sustain_also_settles_the_accumulator() {
        let mut f = Fixture::new();
        f.model.start();
        f.source.pedal(PedalKind::Sostenuto, true);
        pump(&mut f.model, |m| m.is_arpeggio());

        f.model.receive(&SoundingSet::new([60]));
        f.model.receive(&SoundingSet::new([60, 64]));

        f.source.pedal(PedalKind::Sustain, true);
        pump(&mut f.model, |m| m.is_sustain_down());
        f.source.pedal(PedalKind::Sustain, false);
        pump(&mut f.model, |m| !m.is_sustain_down());

        f.model.receive(&SoundingSet::new([72]));
        assert_eq!(
            f.model.keys_row(),
            "C4",
            "sustain lifting cleared the accumulator"
        );
        f.model.stop();
    }

    /// The control: without arpeggio mode, a note the source has dropped stays
    /// dropped, exactly as ticket 08's release tests already prove for the
    /// ordinary path — this just confirms arpeggio is what changes it.
    #[test]
    fn without_arpeggio_mode_a_dropped_note_stays_dropped() {
        let mut f = Fixture::new();
        assert!(!f.model.is_arpeggio());

        f.model.receive(&SoundingSet::new([60]));
        f.model.receive(&SoundingSet::new([60, 64]));
        f.model.receive(&SoundingSet::new([64, 67]));
        assert_eq!(f.model.keys_row(), "E3  G3");
    }
}

// MARK: - Key context (ticket #16)

mod key_context_tests {
    use watchord_core::PitchClass;
    use watchord_theory::key_context::{Key, Mode};

    use super::*;

    #[test]
    fn two_keys_set_and_step_the_key() {
        let mut f = Fixture::new();
        assert_eq!(f.model.key_context(), None, "unset before anything");

        f.model.cycle_key_tonic();
        assert_eq!(
            f.model.key_context(),
            Some(Key::new(PitchClass::new(0), Mode::Major)),
            "the first press of either key sets C major"
        );

        f.model.cycle_key_tonic();
        assert_eq!(
            f.model.key_context(),
            Some(Key::new(PitchClass::new(1), Mode::Major)),
            "the second press only steps the tonic"
        );

        f.model.toggle_key_mode();
        assert_eq!(
            f.model.key_context(),
            Some(Key::new(PitchClass::new(1), Mode::Minor)),
            "the mode key only flips the mode"
        );
    }

    #[test]
    fn the_soft_pedal_held_while_a_chord_settles_sets_the_key_from_the_headline() {
        let mut f = Fixture::new();
        f.model.start();

        // C6 (C E G A): a major triad on C — ticket #16's own worked example.
        f.source.pedal(PedalKind::Soft, true);
        pump(&mut f.model, |m| m.is_soft_down());
        f.model.receive(&c6());
        assert_eq!(
            f.model.key_context(),
            Some(Key::new(PitchClass::new(0), Mode::Major)),
            "root C, major triad quality"
        );

        // A fresh minor headline, settled with the pedal still down.
        let a_minor = SoundingSet::new([57, 60, 64]); // A C E
        f.naming.stub(
            &a_minor,
            StubChordNaming::naming(&a_minor, "Am", "A minor", vec![]),
        );
        f.model.receive(&a_minor);
        assert_eq!(
            f.model.key_context(),
            Some(Key::new(PitchClass::new(9), Mode::Minor)),
            "root A, minor triad quality"
        );

        // The control: with the pedal back up, a settle must not move it.
        f.source.pedal(PedalKind::Soft, false);
        pump(&mut f.model, |m| !m.is_soft_down());
        f.model.receive(&c6());
        assert_eq!(
            f.model.key_context(),
            Some(Key::new(PitchClass::new(9), Mode::Minor)),
            "soft pedal up: the settle must not move the key"
        );
        f.model.stop();
    }

    #[test]
    fn a_declined_settle_leaves_the_key_exactly_as_it_was() {
        let mut f = Fixture::new();
        f.model.start();
        f.source.pedal(PedalKind::Soft, true);
        pump(&mut f.model, |m| m.is_soft_down());
        f.model.receive(&c6());
        let before = f.model.key_context();
        assert!(before.is_some());

        // Past the naming ceiling: the engine declines, there is no headline.
        f.model.receive(&too_many());
        assert_eq!(
            f.model.key_context(),
            before,
            "a declined settle must not clear or move the key"
        );
        f.model.stop();
    }

    #[test]
    fn the_ranking_of_readings_is_identical_with_and_without_a_key() {
        let mut f = Fixture::new();
        f.model.receive(&c6());
        let without_key = alternate_names(&f.model);

        f.model.cycle_key_tonic(); // sets C major
        let with_key = alternate_names(&f.model);
        assert_eq!(
            without_key, with_key,
            "the alternates list, by display string, must not move"
        );

        // The control: a genuinely different chord's list compares unequal,
        // so the equality above is not a tautology of a broken comparison.
        f.model.receive(&f_major());
        let different_chord = alternate_names(&f.model);
        assert_ne!(with_key, different_chord);
    }
}
