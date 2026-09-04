//! Ported case for case from note-view `Tests/NotesStoreTests/JSONNotesStoreTests.swift`.
//!
//! Every store in this file is pointed at a fresh temp directory. No test may
//! address the real store — see `real_store_location` for the assertion that
//! holds us to it.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use watchord_core::{ChordKey, ChordNote, NoteStoring, StoreError};
use watchord_store::{Clock, JsonNotesStore, default_path};

fn c_major() -> ChordKey {
    ChordKey::parse("0.4.7").unwrap()
}
fn c_six() -> ChordKey {
    ChordKey::parse("0.4.7.9").unwrap()
}
fn uuid() -> String {
    uuid::Uuid::new_v4().to_string().to_uppercase()
}

/// A unique directory under the OS temp dir, removed when dropped.
struct TempDirectory {
    path: PathBuf,
}

impl TempDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join("watchord-tests").join(uuid());
        fs::create_dir_all(&path).unwrap();
        TempDirectory { path }
    }

    fn notes_file(&self) -> PathBuf {
        self.path.join("notes.json")
    }

    /// Names of everything directly inside, sorted. Includes dotfiles, which
    /// matters — the store's temp files are hidden.
    fn contents(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(&self.path)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    fn damaged_files(&self) -> Vec<String> {
        self.contents()
            .into_iter()
            .filter(|n| n.contains(".damaged-"))
            .collect()
    }

    fn leftover_temp_files(&self) -> Vec<String> {
        self.contents()
            .into_iter()
            .filter(|n| n.contains(".tmp-"))
            .collect()
    }

    fn write_raw_store(&self, text: &str) {
        fs::write(self.notes_file(), text).unwrap();
    }

    fn raw_store_text(&self) -> String {
        fs::read_to_string(self.notes_file()).unwrap()
    }

    fn text_of(&self, name: &str) -> String {
        fs::read_to_string(self.path.join(name)).unwrap()
    }

    /// Makes the directory unwritable, so the next attempt to create a file in
    /// it fails the way a full or read-only volume would.
    #[cfg(unix)]
    fn deny_writes(&self) {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&self.path, fs::Permissions::from_mode(0o500)).unwrap();
    }

    #[cfg(unix)]
    fn allow_writes(&self) {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&self.path, fs::Permissions::from_mode(0o700));
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        #[cfg(unix)]
        self.allow_writes();
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// A clock the test drives by hand, so "newest first" is a fact about the
/// notes rather than a bet on the machine.
#[derive(Clone)]
struct TestClock {
    current: Arc<Mutex<SystemTime>>,
}

impl TestClock {
    fn new() -> Self {
        Self::starting_at(UNIX_EPOCH + Duration::from_secs(1_760_000_000))
    }

    fn starting_at(start: SystemTime) -> Self {
        TestClock {
            current: Arc::new(Mutex::new(start)),
        }
    }

    fn now(&self) -> Clock {
        let current = Arc::clone(&self.current);
        Box::new(move || *current.lock().unwrap())
    }

    fn advance(&self, seconds: u64) {
        let mut current = self.current.lock().unwrap();
        *current += Duration::from_secs(seconds);
    }
}

fn ids(notes: &[ChordNote]) -> Vec<&str> {
    notes.iter().map(|n| n.id.as_str()).collect()
}

fn texts(notes: &[ChordNote]) -> Vec<&str> {
    notes.iter().map(|n| n.text.as_str()).collect()
}

fn entry(id: &str, key: &str, text: &str, spelling: &str, created: &str) -> String {
    format!(
        r#"{{ "id": "{id}", "chordKey": "{key}", "text": "{text}", "spellingWhenWritten": "{spelling}", "createdAt": "{created}" }}"#
    )
}

// MARK: - Reading

mod reading_an_empty_store {
    use super::*;

    #[test]
    fn an_absent_file_is_an_empty_store_not_an_error() {
        let dir = TempDirectory::new();
        let store = JsonNotesStore::at(dir.notes_file());

        assert!(store.all_notes().unwrap().is_empty());
        assert!(store.notes(&c_major()).unwrap().is_empty());
    }

    #[test]
    fn reading_an_absent_store_creates_nothing() {
        let dir = TempDirectory::new();
        let store = JsonNotesStore::at(dir.notes_file());

        store.all_notes().unwrap();

        // A read is a read. Creating a file here would mean the app leaves
        // droppings just for having launched.
        assert!(dir.contents().is_empty());
    }
}

// MARK: - Round trip

mod round_trip {
    use super::*;

    #[test]
    fn three_notes_across_two_chords_come_back_grouped_and_newest_first() {
        let dir = TempDirectory::new();
        let clock = TestClock::new();

        let writing = JsonNotesStore::with_clock(dir.notes_file(), clock.now());
        let first = writing.add("the Rhodes one", &c_six(), "C6").unwrap();
        clock.advance(60);
        let second = writing.add("try the 9 on top", &c_six(), "Am7").unwrap();
        clock.advance(60);
        let third = writing.add("plain and useful", &c_major(), "C").unwrap();

        // A second instance, so this reads the file rather than the cache.
        let reading = JsonNotesStore::at(dir.notes_file());

        let sixes = reading.notes(&c_six()).unwrap();
        assert_eq!(ids(&sixes), [second.id.as_str(), first.id.as_str()]);
        assert_eq!(texts(&sixes), ["try the 9 on top", "the Rhodes one"]);

        let majors = reading.notes(&c_major()).unwrap();
        assert_eq!(ids(&majors), [third.id.as_str()]);

        assert_eq!(
            ids(&reading.all_notes().unwrap()),
            [third.id.as_str(), second.id.as_str(), first.id.as_str()]
        );
    }

    #[test]
    fn every_field_survives_the_round_trip() {
        let dir = TempDirectory::new();
        let clock = TestClock::new();

        let written = JsonNotesStore::with_clock(dir.notes_file(), clock.now())
            .add("sounds like Voodoo", &c_six(), "C6")
            .unwrap();

        let all = JsonNotesStore::at(dir.notes_file()).all_notes().unwrap();
        let reloaded = all.first().expect("one note");

        assert_eq!(*reloaded, written);
        assert_eq!(reloaded.id, written.id);
        assert_eq!(reloaded.chord_key, c_six());
        assert_eq!(reloaded.spelling_when_written, "C6");
        assert_eq!(reloaded.text, "sounds like Voodoo");
        assert_eq!(reloaded.created_at, written.created_at);
    }

    #[test]
    fn lookup_is_exact_and_not_a_subset_match() {
        let dir = TempDirectory::new();
        let store = JsonNotesStore::at(dir.notes_file());

        store.add("on the six", &c_six(), "C6").unwrap();

        // 0.4.7 is a subset of 0.4.7.9. Identity is the whole set.
        assert!(store.notes(&c_major()).unwrap().is_empty());
        assert_eq!(store.notes(&c_six()).unwrap().len(), 1);
    }

    #[test]
    fn delete_removes_one_note_and_persists() {
        let dir = TempDirectory::new();
        let clock = TestClock::new();
        let store = JsonNotesStore::with_clock(dir.notes_file(), clock.now());

        let doomed = store.add("wrong", &c_six(), "C6").unwrap();
        clock.advance(1);
        let keeper = store.add("right", &c_six(), "C6").unwrap();

        store.delete(&doomed.id).unwrap();

        assert_eq!(ids(&store.all_notes().unwrap()), [keeper.id.as_str()]);
        assert_eq!(
            ids(&JsonNotesStore::at(dir.notes_file()).all_notes().unwrap()),
            [keeper.id.as_str()]
        );
    }

    #[test]
    fn deleting_something_absent_is_a_no_op() {
        let dir = TempDirectory::new();
        let store = JsonNotesStore::at(dir.notes_file());
        let kept = store.add("keep me", &c_six(), "C6").unwrap();

        store.delete(&uuid()).unwrap();

        assert_eq!(ids(&store.all_notes().unwrap()), [kept.id.as_str()]);
    }

    /// `created_at` alone is not a total order, and two notes written in the
    /// same instant is what a fast machine does with two quick returns. The
    /// order must be a function of the notes themselves, not of file position.
    #[test]
    fn notes_sharing_a_timestamp_load_in_an_order_independent_of_file_position() {
        let ids: Vec<String> = (0..8).map(|_| uuid()).collect();
        let store_text = |order: &[String]| {
            let entries: Vec<String> = order
                .iter()
                .map(|id| entry(id, "0.4.7", id, "C", "2026-08-10T14:00:00Z"))
                .collect();
            format!("{{ \"version\": 1, \"notes\": [{}] }}", entries.join(","))
        };

        let forward = TempDirectory::new();
        forward.write_raw_store(&store_text(&ids));
        let reversed = TempDirectory::new();
        let mut backwards = ids.clone();
        backwards.reverse();
        reversed.write_raw_store(&store_text(&backwards));

        let from_forward: Vec<String> = JsonNotesStore::at(forward.notes_file())
            .all_notes()
            .unwrap()
            .into_iter()
            .map(|n| n.id)
            .collect();
        let from_reversed: Vec<String> = JsonNotesStore::at(reversed.notes_file())
            .all_notes()
            .unwrap()
            .into_iter()
            .map(|n| n.id)
            .collect();

        assert_eq!(from_forward.len(), ids.len());
        assert_eq!(
            from_forward, from_reversed,
            "the order followed the file, not the notes"
        );
    }

    #[test]
    fn many_notes_can_share_one_chord() {
        let dir = TempDirectory::new();
        let clock = TestClock::new();
        let store = JsonNotesStore::with_clock(dir.notes_file(), clock.now());

        for index in 0..10 {
            clock.advance(1);
            store.add(&format!("note {index}"), &c_six(), "C6").unwrap();
        }

        assert_eq!(store.notes(&c_six()).unwrap().len(), 10);
        assert_eq!(
            JsonNotesStore::at(dir.notes_file())
                .notes(&c_six())
                .unwrap()
                .len(),
            10
        );
    }
}

// MARK: - The written shape

mod on_disk_shape {
    use super::*;

    /// Asserted against the real bytes rather than by round-tripping, because a
    /// round trip passes for any self-consistent format.
    #[test]
    fn matches_the_specified_json() {
        let dir = TempDirectory::new();
        let clock = TestClock::starting_at(UNIX_EPOCH + Duration::from_secs(1_770_000_000));
        let store = JsonNotesStore::with_clock(dir.notes_file(), clock.now());

        store.add("hello", &c_six(), "C6").unwrap();

        let root: serde_json::Value =
            serde_json::from_str(&dir.raw_store_text()).expect("valid JSON");
        assert_eq!(root["version"], 1);

        let notes = root["notes"].as_array().expect("a notes array");
        assert_eq!(notes.len(), 1);

        let note = &notes[0];
        assert_eq!(note["chordKey"], "0.4.7.9");
        assert_eq!(note["spellingWhenWritten"], "C6");
        assert_eq!(note["text"], "hello");
        assert!(uuid::Uuid::parse_str(note["id"].as_str().unwrap()).is_ok());
        assert_eq!(note["createdAt"], "2026-02-02T02:40:00Z");
    }

    /// Byte for byte what note-view writes: two-space indent, sorted keys, and
    /// Foundation's ` : ` between key and value, so a file written by either
    /// app diffs empty against the other.
    #[test]
    fn matches_note_views_bytes() {
        let dir = TempDirectory::new();
        let clock = TestClock::starting_at(UNIX_EPOCH + Duration::from_secs(1_770_000_000));
        let store = JsonNotesStore::with_clock(dir.notes_file(), clock.now());
        let note = store.add("hello", &c_six(), "CΔ6").unwrap();

        let expected = format!(
            "{{\n  \"notes\" : [\n    {{\n      \"chordKey\" : \"0.4.7.9\",\n      \"createdAt\" : \"2026-02-02T02:40:00Z\",\n      \"id\" : \"{}\",\n      \"spellingWhenWritten\" : \"CΔ6\",\n      \"text\" : \"hello\"\n    }}\n  ],\n  \"version\" : 1\n}}",
            note.id
        );
        assert_eq!(dir.raw_store_text(), expected);
    }

    #[test]
    fn version_is_present_from_the_first_write() {
        let dir = TempDirectory::new();
        let store = JsonNotesStore::at(dir.notes_file());

        store.add("hello", &c_major(), "C").unwrap();

        assert!(dir.raw_store_text().contains("\"version\""));
    }

    #[test]
    fn a_store_built_from_a_directory_puts_notes_json_inside_it() {
        let dir = TempDirectory::new();
        let nested = dir.path.join("does-not-exist-yet");

        let store = JsonNotesStore::in_directory(&nested).unwrap();
        assert_eq!(store.location(), nested.join("notes.json"));

        let added = store.add("hello", &c_six(), "C6").unwrap();
        assert_eq!(
            ids(&JsonNotesStore::in_directory(&nested)
                .unwrap()
                .all_notes()
                .unwrap()),
            [added.id.as_str()]
        );
    }

    #[test]
    fn the_directory_is_created_on_first_write() {
        let dir = TempDirectory::new();
        let nested = dir
            .path
            .join("Application Support")
            .join("note-view")
            .join("notes.json");

        let store = JsonNotesStore::at(&nested);
        store.add("hello", &c_major(), "C").unwrap();

        assert!(nested.exists());
    }
}

// MARK: - Surviving a damaged store

mod damaged_store {
    use super::*;

    #[test]
    fn truncated_json_yields_an_empty_store_moves_the_file_aside_and_the_next_add_succeeds() {
        let dir = TempDirectory::new();
        dir.write_raw_store(r#"{"version": 1, "notes": [{"id": "AB"#);

        let store = JsonNotesStore::at(dir.notes_file());

        assert!(store.all_notes().unwrap().is_empty());
        assert_eq!(dir.damaged_files().len(), 1);

        // And the store is usable afterwards, rather than wedged.
        let added = store.add("after the damage", &c_six(), "C6").unwrap();
        assert_eq!(ids(&store.all_notes().unwrap()), [added.id.as_str()]);
        assert_eq!(
            ids(&JsonNotesStore::at(dir.notes_file()).all_notes().unwrap()),
            [added.id.as_str()]
        );
    }

    #[test]
    fn the_moved_aside_file_still_holds_the_original_bytes() {
        let dir = TempDirectory::new();
        let original = r#"{"version": 1, "notes": [{"id": "not a uuid", "text": "precious"#;
        dir.write_raw_store(original);

        let store = JsonNotesStore::at(dir.notes_file());
        store.all_notes().unwrap();

        let quarantined = dir.damaged_files();
        let quarantined = quarantined.first().expect("a quarantined file");
        assert_eq!(dir.text_of(quarantined), original);
    }

    #[test]
    fn not_json_at_all_is_survived() {
        let dir = TempDirectory::new();
        dir.write_raw_store("this is not json, it is a sandwich");

        let store = JsonNotesStore::at(dir.notes_file());

        assert!(store.all_notes().unwrap().is_empty());
        assert_eq!(dir.damaged_files().len(), 1);
    }

    #[test]
    fn an_empty_file_is_survived() {
        let dir = TempDirectory::new();
        dir.write_raw_store("");

        let store = JsonNotesStore::at(dir.notes_file());

        assert!(store.all_notes().unwrap().is_empty());
        assert_eq!(
            store.add("recovered", &c_major(), "C").unwrap().text,
            "recovered"
        );
    }

    #[test]
    fn a_non_canonical_chord_key_is_skipped_and_its_neighbours_still_load() {
        let dir = TempDirectory::new();
        dir.write_raw_store(&format!(
            "{{ \"version\": 1, \"notes\": [{}, {}, {}] }}",
            entry(&uuid(), "0.4.7", "good one", "C", "2026-08-10T14:00:00Z"),
            entry(
                &uuid(),
                "7.4.0",
                "unsorted key",
                "C",
                "2026-08-10T14:01:00Z"
            ),
            entry(
                &uuid(),
                "0.4.7.9",
                "another good one",
                "C6",
                "2026-08-10T14:02:00Z"
            ),
        ));

        let loaded = JsonNotesStore::at(dir.notes_file()).all_notes().unwrap();

        assert_eq!(texts(&loaded), ["another good one", "good one"]);
        assert!(!loaded.iter().any(|n| n.text == "unsorted key"));
    }

    #[test]
    fn each_kind_of_unreadable_key_is_skipped_rather_than_stored() {
        for bad_key in ["7.4.0", "0.4.7.", "0.4.4", "12.4", "", "0,4,7", "0.4.7.x"] {
            let dir = TempDirectory::new();
            dir.write_raw_store(&format!(
                "{{ \"version\": 1, \"notes\": [{}, {}] }}",
                entry(&uuid(), bad_key, "bad", "?", "2026-08-10T14:00:00Z"),
                entry(&uuid(), "0.4.7", "good", "C", "2026-08-10T14:01:00Z"),
            ));

            let loaded = JsonNotesStore::at(dir.notes_file()).all_notes().unwrap();
            assert_eq!(
                texts(&loaded),
                ["good"],
                "key '{bad_key}' should have been skipped"
            );
        }
    }

    #[test]
    fn an_entry_missing_a_field_is_skipped_and_its_neighbours_still_load() {
        let dir = TempDirectory::new();
        dir.write_raw_store(&format!(
            "{{ \"version\": 1, \"notes\": [{{ \"id\": \"{}\", \"chordKey\": \"0.4.7\", \"createdAt\": \"2026-08-10T14:00:00Z\" }}, {}] }}",
            uuid(),
            entry(&uuid(), "0.4.7", "intact", "C", "2026-08-10T14:01:00Z"),
        ));

        assert_eq!(
            texts(&JsonNotesStore::at(dir.notes_file()).all_notes().unwrap()),
            ["intact"]
        );
    }

    /// The quiet loss. The file reads fine, so nothing looks wrong — and the
    /// very next `add` rewrites it with the unreadable entries dropped.
    #[test]
    fn rewriting_a_partially_readable_file_preserves_the_original_first() {
        let dir = TempDirectory::new();
        dir.write_raw_store(&format!(
            "{{ \"version\": 1, \"notes\": [{}, {}] }}",
            entry(
                &uuid(),
                "7.4.0",
                "only copy of this",
                "C",
                "2026-08-10T14:00:00Z"
            ),
            entry(&uuid(), "0.4.7", "fine", "C", "2026-08-10T14:01:00Z"),
        ));

        let store = JsonNotesStore::at(dir.notes_file());
        store.add("new note", &c_six(), "C6").unwrap();

        let preserved = dir.damaged_files();
        let preserved = preserved.first().expect("the original was not preserved");
        assert!(dir.text_of(preserved).contains("only copy of this"));

        // And the live store moved on correctly.
        assert!(dir.raw_store_text().contains("new note"));
        assert_eq!(store.all_notes().unwrap().len(), 2);
    }

    #[test]
    fn a_clean_file_is_never_moved_aside() {
        let dir = TempDirectory::new();
        let store = JsonNotesStore::at(dir.notes_file());

        store.add("one", &c_major(), "C").unwrap();
        store.add("two", &c_six(), "C6").unwrap();
        JsonNotesStore::at(dir.notes_file()).all_notes().unwrap();

        assert!(dir.damaged_files().is_empty());
        assert_eq!(dir.contents(), ["notes.json"]);
    }

    #[test]
    fn two_damaged_generations_become_two_files() {
        let dir = TempDirectory::new();

        dir.write_raw_store("garbage one");
        JsonNotesStore::at(dir.notes_file()).all_notes().unwrap();

        dir.write_raw_store("garbage two");
        JsonNotesStore::at(dir.notes_file()).all_notes().unwrap();

        assert_eq!(dir.damaged_files().len(), 2);
        let mut recovered: Vec<String> =
            dir.damaged_files().iter().map(|n| dir.text_of(n)).collect();
        recovered.sort();
        assert_eq!(recovered, ["garbage one", "garbage two"]);
    }
}

// MARK: - Versioning

mod versioning {
    use super::*;

    #[test]
    fn an_unknown_version_does_not_crash() {
        let dir = TempDirectory::new();
        dir.write_raw_store(r#"{"version": 2, "notes": []}"#);

        assert!(
            JsonNotesStore::at(dir.notes_file())
                .all_notes()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn an_unknown_version_is_preserved_rather_than_overwritten() {
        let dir = TempDirectory::new();
        let future = r#"{"version": 2, "notes": [], "somethingNewer": true}"#;
        dir.write_raw_store(future);

        JsonNotesStore::at(dir.notes_file()).all_notes().unwrap();

        let preserved = dir.damaged_files();
        let preserved = preserved.first().expect("preserved");
        assert_eq!(dir.text_of(preserved), future);
    }

    #[test]
    fn readable_notes_in_an_unknown_version_are_still_recovered() {
        let dir = TempDirectory::new();
        dir.write_raw_store(&format!(
            "{{ \"version\": 2, \"notes\": [{}] }}",
            entry(
                &uuid(),
                "0.4.7",
                "written by a newer build",
                "C",
                "2026-08-10T14:00:00Z"
            ),
        ));

        assert_eq!(
            texts(&JsonNotesStore::at(dir.notes_file()).all_notes().unwrap()),
            ["written by a newer build"]
        );
    }

    /// Recovering into memory is not enough: the file they came from has been
    /// moved aside, so unless they are written forward they are visible exactly
    /// once and gone after the next launch.
    #[test]
    fn notes_recovered_from_an_unknown_version_are_still_there_on_the_next_launch() {
        let dir = TempDirectory::new();
        dir.write_raw_store(&format!(
            "{{ \"version\": 2, \"notes\": [{}] }}",
            entry(
                &uuid(),
                "0.4.7",
                "written by a newer build",
                "C",
                "2026-08-10T14:00:00Z"
            ),
        ));

        JsonNotesStore::at(dir.notes_file()).all_notes().unwrap();

        let relaunched = JsonNotesStore::at(dir.notes_file()).all_notes().unwrap();
        assert_eq!(texts(&relaunched), ["written by a newer build"]);

        // And the original is still on disk, exactly once.
        assert_eq!(dir.damaged_files().len(), 1);
        assert!(
            dir.text_of(&dir.damaged_files()[0])
                .contains("\"version\": 2")
        );

        // Re-reading a migrated store must not keep producing damaged copies.
        JsonNotesStore::at(dir.notes_file()).all_notes().unwrap();
        assert_eq!(dir.damaged_files().len(), 1);
    }

    #[test]
    fn a_missing_version_is_treated_as_damaged_not_as_current() {
        let dir = TempDirectory::new();
        dir.write_raw_store(r#"{"notes": []}"#);

        let store = JsonNotesStore::at(dir.notes_file());

        assert!(store.all_notes().unwrap().is_empty());
        assert_eq!(dir.damaged_files().len(), 1);
    }

    #[test]
    fn a_store_with_no_notes_array_is_empty_not_damaged() {
        let dir = TempDirectory::new();
        dir.write_raw_store(r#"{"version": 1}"#);

        let store = JsonNotesStore::at(dir.notes_file());

        assert!(store.all_notes().unwrap().is_empty());
        assert!(dir.damaged_files().is_empty());
    }
}

// MARK: - Atomic writes

#[cfg(unix)]
mod atomic_writes {
    use super::*;

    #[test]
    fn a_failed_write_leaves_the_previous_good_file_byte_identical() {
        let dir = TempDirectory::new();
        let store = JsonNotesStore::at(dir.notes_file());
        let survivor = store.add("must survive", &c_six(), "C6").unwrap();

        let before = fs::read(dir.notes_file()).unwrap();

        dir.deny_writes();
        let result = store.add("cannot be written", &c_major(), "C");
        dir.allow_writes();
        assert!(matches!(result, Err(StoreError::Io(_))));

        // Not "still parses" — byte-identical.
        assert_eq!(fs::read(dir.notes_file()).unwrap(), before);
        assert_eq!(
            ids(&JsonNotesStore::at(dir.notes_file()).all_notes().unwrap()),
            [survivor.id.as_str()]
        );
    }

    /// Distinguishes temp-in-the-same-directory from temp-in-/tmp.
    #[test]
    fn the_temp_file_is_created_in_the_target_directory() {
        let dir = TempDirectory::new();
        let store = JsonNotesStore::at(dir.notes_file());
        store.add("first", &c_six(), "C6").unwrap();

        dir.deny_writes();
        let result = store.add("second", &c_major(), "C");
        dir.allow_writes();

        let Err(StoreError::Io(reason)) = result else {
            panic!("expected a write failure, got {result:?}");
        };
        assert!(reason.contains("temporary file"), "{reason}");
        assert!(reason.contains(dir.path.to_str().unwrap()), "{reason}");
    }

    #[test]
    fn a_failed_write_leaves_the_in_memory_view_matching_the_file() {
        let dir = TempDirectory::new();
        let store = JsonNotesStore::at(dir.notes_file());
        let survivor = store.add("on disk", &c_six(), "C6").unwrap();

        dir.deny_writes();
        let result = store.add("never landed", &c_major(), "C");
        dir.allow_writes();
        assert!(result.is_err());

        assert_eq!(ids(&store.all_notes().unwrap()), [survivor.id.as_str()]);
    }

    #[test]
    fn no_temp_files_are_left_behind_after_a_successful_write() {
        let dir = TempDirectory::new();
        let store = JsonNotesStore::at(dir.notes_file());

        for index in 0..5 {
            store.add(&format!("note {index}"), &c_six(), "C6").unwrap();
        }

        assert!(dir.leftover_temp_files().is_empty());
        assert_eq!(dir.contents(), ["notes.json"]);
    }

    #[test]
    fn no_temp_files_are_left_behind_after_a_failed_write() {
        let dir = TempDirectory::new();
        let store = JsonNotesStore::at(dir.notes_file());
        store.add("first", &c_six(), "C6").unwrap();

        dir.deny_writes();
        let result = store.add("second", &c_major(), "C");
        dir.allow_writes();
        assert!(result.is_err());

        assert!(dir.leftover_temp_files().is_empty());
    }

    /// Rewriting repeatedly while another thread watches: the path must resolve
    /// to a complete, parseable store at every instant.
    #[test]
    fn the_store_is_never_absent_or_partial_during_a_write() {
        let dir = TempDirectory::new();
        let path = dir.notes_file();
        let store = Arc::new(JsonNotesStore::at(&path));
        store.add("first", &c_six(), "C6").unwrap();

        let writing_is_done = Arc::new(Mutex::new(false));
        let watcher_is_running = Arc::new(Mutex::new(false));
        let watcher = {
            let path = path.clone();
            let done = Arc::clone(&writing_is_done);
            let running = Arc::clone(&watcher_is_running);
            thread::spawn(move || {
                let (mut reads, mut absent, mut unparseable) = (0, 0, 0);
                loop {
                    match fs::read(&path) {
                        Ok(bytes) => {
                            reads += 1;
                            if serde_json::from_slice::<serde_json::Value>(&bytes).is_err() {
                                unparseable += 1;
                            }
                        }
                        Err(_) => absent += 1,
                    }
                    *running.lock().unwrap() = true;
                    if *done.lock().unwrap() {
                        break;
                    }
                }
                (reads, absent, unparseable)
            })
        };

        // Wait until the watcher is genuinely running before writing anything,
        // or on a busy machine all the writes finish before it ever reads and
        // `reads > 0` fails — which reads as "the store is broken" when it
        // means "the instrument never ran".
        while !*watcher_is_running.lock().unwrap() {
            thread::yield_now();
        }

        for index in 0..60 {
            store.add(&format!("note {index}"), &c_six(), "C6").unwrap();
        }
        *writing_is_done.lock().unwrap() = true;

        let (reads, absent, unparseable) = watcher.join().unwrap();
        assert_eq!(unparseable, 0, "the store was observed mid-write");
        assert_eq!(absent, 0, "the store briefly did not exist");
        assert!(reads > 0, "the watcher never read the store at all");
    }
}

// MARK: - Updating a note's text

mod updating {
    use super::*;

    #[test]
    fn update_replaces_the_text_and_persists() {
        let dir = TempDirectory::new();
        let store = JsonNotesStore::at(dir.notes_file());
        let written = store.add("the Rhodes one", &c_six(), "C6").unwrap();

        let updated = store.update(&written.id, "the Rhodes one, muted").unwrap();
        assert_eq!(updated.text, "the Rhodes one, muted");
        assert_eq!(updated.id, written.id);
        assert_eq!(updated.chord_key, written.chord_key);
        assert_eq!(updated.spelling_when_written, written.spelling_when_written);
        assert_eq!(updated.created_at, written.created_at);

        // A second instance, so this reads the file rather than the cache.
        let reloaded = JsonNotesStore::at(dir.notes_file()).all_notes().unwrap();
        assert_eq!(reloaded.len(), 1);
        assert_eq!(reloaded[0].text, "the Rhodes one, muted");
        assert_eq!(reloaded[0].id, written.id);
    }

    #[test]
    fn updating_an_absent_id_is_an_error_and_nothing_on_disk_changes() {
        let dir = TempDirectory::new();
        let store = JsonNotesStore::at(dir.notes_file());
        store.add("keep", &c_six(), "C6").unwrap();
        let before = fs::read(dir.notes_file()).unwrap();

        let result = store.update(&uuid(), "new text");
        assert!(matches!(result, Err(StoreError::NoSuchNote(_))));
        assert_eq!(fs::read(dir.notes_file()).unwrap(), before);
    }

    /// The byte-compatibility claim from #15: update keeps the file shape
    /// note-view writes — same fields, same format, only the text differs.
    /// Asserted against the real bytes, as `matches_note_views_bytes` is,
    /// because a round trip would pass for any self-consistent format.
    #[test]
    fn update_keeps_the_file_byte_compatible_with_note_views_schema() {
        let dir = TempDirectory::new();
        let clock = TestClock::starting_at(UNIX_EPOCH + Duration::from_secs(1_770_000_000));
        let store = JsonNotesStore::with_clock(dir.notes_file(), clock.now());
        let note = store.add("hello", &c_six(), "CΔ6").unwrap();

        store.update(&note.id, "hello, edited").unwrap();

        let expected = format!(
            "{{\n  \"notes\" : [\n    {{\n      \"chordKey\" : \"0.4.7.9\",\n      \"createdAt\" : \"2026-02-02T02:40:00Z\",\n      \"id\" : \"{}\",\n      \"spellingWhenWritten\" : \"CΔ6\",\n      \"text\" : \"hello, edited\"\n    }}\n  ],\n  \"version\" : 1\n}}",
            note.id
        );
        assert_eq!(dir.raw_store_text(), expected);
    }

    /// "A note written by note-view reads back unchanged" — updating one note
    /// must not disturb its neighbour's bytes at all.
    #[test]
    fn updating_one_note_leaves_a_note_written_in_the_note_view_schema_unchanged() {
        let dir = TempDirectory::new();
        let neighbour_id = uuid();
        dir.write_raw_store(&format!(
            "{{ \"version\": 1, \"notes\": [{}] }}",
            entry(
                &neighbour_id,
                "0.4.7",
                "written by note-view",
                "C",
                "2026-08-10T14:00:00Z"
            ),
        ));
        let store = JsonNotesStore::at(dir.notes_file());
        let mine = store.add("mine", &c_six(), "C6").unwrap();

        store.update(&mine.id, "mine, edited").unwrap();

        let reloaded = JsonNotesStore::at(dir.notes_file()).all_notes().unwrap();
        let neighbour = reloaded
            .iter()
            .find(|n| n.id == neighbour_id)
            .expect("the neighbour is still there");
        assert_eq!(neighbour.text, "written by note-view");
        assert_eq!(neighbour.chord_key, c_major());
        assert_eq!(neighbour.spelling_when_written, "C");
        let mine = reloaded.iter().find(|n| n.id == mine.id).expect("mine");
        assert_eq!(mine.text, "mine, edited");
    }
}

// MARK: - Refusing to write what cannot be read back

mod refusing_unreadable_writes {
    use super::*;

    #[test]
    fn a_note_against_the_empty_key_is_refused_rather_than_silently_lost() {
        let dir = TempDirectory::new();
        let store = JsonNotesStore::at(dir.notes_file());

        let result = store.add("would vanish", &ChordKey::empty(), "—");
        let Err(StoreError::Corrupt(message)) = result else {
            panic!("expected a refusal, got {result:?}");
        };
        assert!(message.contains("(empty)"), "{message}");

        // Nothing was written, so there is no half-state to clean up.
        assert!(dir.contents().is_empty());
    }

    #[test]
    fn a_good_note_is_unaffected_by_a_refused_one() {
        let dir = TempDirectory::new();
        let store = JsonNotesStore::at(dir.notes_file());

        let kept = store.add("keep", &c_six(), "C6").unwrap();
        assert!(store.add("refused", &ChordKey::empty(), "—").is_err());

        assert_eq!(ids(&store.all_notes().unwrap()), [kept.id.as_str()]);
        assert_eq!(
            ids(&JsonNotesStore::at(dir.notes_file()).all_notes().unwrap()),
            [kept.id.as_str()]
        );
    }

    #[test]
    fn every_note_this_store_writes_can_be_read_back() {
        let dir = TempDirectory::new();
        let clock = TestClock::new();
        let store = JsonNotesStore::with_clock(dir.notes_file(), clock.now());

        let texts_in = [
            "plain".to_string(),
            "with \"quotes\" and \\backslashes\\".to_string(),
            "unicode: Δ ♭ ♯ — 🎹".to_string(),
            "newlines\nand\ttabs".to_string(),
            "long ".repeat(2000),
            String::new(),
        ];
        for (index, text) in texts_in.iter().enumerate() {
            clock.advance(1);
            let key = ChordKey::parse(&format!("{index}.11")).unwrap();
            store.add(text, &key, &format!("Δ{index}")).unwrap();
        }

        let reloaded = JsonNotesStore::at(dir.notes_file()).all_notes().unwrap();
        assert_eq!(reloaded.len(), texts_in.len());
        let mut got: Vec<String> = reloaded.into_iter().map(|n| n.text).collect();
        let mut want = texts_in.to_vec();
        got.sort();
        want.sort();
        assert_eq!(got, want);
    }
}

// MARK: - Thread safety

mod thread_safety {
    use super::*;

    fn concurrently(count: usize, work: impl Fn(usize) + Send + Sync) {
        thread::scope(|scope| {
            for index in 0..count {
                let work = &work;
                scope.spawn(move || work(index));
            }
        });
    }

    #[test]
    fn concurrent_adds_all_survive() {
        let dir = TempDirectory::new();
        let store = JsonNotesStore::at(dir.notes_file());
        let count = 60;

        concurrently(count, |index| {
            let _ = store.add(&format!("note {index}"), &c_six(), "C6");
        });

        assert_eq!(store.all_notes().unwrap().len(), count);

        let reloaded = JsonNotesStore::at(dir.notes_file()).all_notes().unwrap();
        assert_eq!(reloaded.len(), count);
        let mut got: Vec<String> = reloaded.into_iter().map(|n| n.text).collect();
        let mut want: Vec<String> = (0..count).map(|i| format!("note {i}")).collect();
        got.sort();
        want.sort();
        assert_eq!(got, want);
    }

    #[test]
    fn concurrent_reads_and_writes_do_not_corrupt_the_store() {
        let dir = TempDirectory::new();
        let store = JsonNotesStore::at(dir.notes_file());
        store.add("seed", &c_six(), "C6").unwrap();

        concurrently(40, |index| {
            if index % 2 == 0 {
                let _ = store.add(&format!("note {index}"), &c_major(), "C");
            } else {
                let _ = store.all_notes();
                let _ = store.notes(&c_six());
            }
        });

        assert!(dir.leftover_temp_files().is_empty());
        assert_eq!(
            JsonNotesStore::at(dir.notes_file())
                .all_notes()
                .unwrap()
                .len(),
            21
        );
    }

    #[test]
    fn concurrent_deletes_remove_exactly_what_was_asked() {
        let dir = TempDirectory::new();
        let clock = TestClock::new();
        let store = JsonNotesStore::with_clock(dir.notes_file(), clock.now());

        let mut added = Vec::new();
        for index in 0..30 {
            clock.advance(1);
            added.push(store.add(&format!("note {index}"), &c_six(), "C6").unwrap());
        }

        let doomed: Vec<String> = added.iter().take(15).map(|n| n.id.clone()).collect();
        concurrently(doomed.len(), |index| {
            let _ = store.delete(&doomed[index]);
        });

        let remaining = JsonNotesStore::at(dir.notes_file()).all_notes().unwrap();
        assert_eq!(remaining.len(), 15);
        let mut got: Vec<String> = remaining.into_iter().map(|n| n.id).collect();
        let mut want: Vec<String> = added.iter().skip(15).map(|n| n.id.clone()).collect();
        got.sort();
        want.sort();
        assert_eq!(got, want);
    }
}

// MARK: - The real store location

mod real_store_location {
    use super::*;

    #[test]
    #[cfg(target_os = "macos")]
    fn the_default_path_is_the_one_the_adr_names() {
        let path = default_path().expect("a home directory");
        let home = directories::BaseDirs::new()
            .unwrap()
            .home_dir()
            .to_path_buf();

        assert!(path.ends_with("Library/Application Support/note-view/notes.json"));
        assert!(path.starts_with(&home));
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn the_default_path_is_the_platform_data_dir() {
        let path = default_path().expect("a home directory");
        let data = directories::BaseDirs::new()
            .unwrap()
            .data_dir()
            .to_path_buf();

        assert!(path.ends_with("watchord/notes.json"));
        assert!(path.starts_with(&data));
    }

    /// A read-only snapshot of a path, to prove a test did not touch it.
    #[derive(PartialEq, Debug)]
    struct PathSnapshot {
        exists: bool,
        bytes: Option<Vec<u8>>,
        modified: Option<SystemTime>,
    }

    impl PathSnapshot {
        fn of(path: &Path) -> Self {
            let exists = path.exists();
            PathSnapshot {
                exists,
                bytes: if exists { fs::read(path).ok() } else { None },
                modified: if exists {
                    fs::metadata(path).and_then(|m| m.modified()).ok()
                } else {
                    None
                },
            }
        }
    }

    /// A test that passes because it read the developer's real notes file is
    /// worse than no test. This exercises the full write path against a temp
    /// store and proves the real one is untouched, byte for byte.
    #[test]
    fn a_full_cycle_against_a_temp_store_leaves_the_real_store_untouched() {
        let real = default_path().expect("a home directory");
        let before = PathSnapshot::of(&real);

        let dir = TempDirectory::new();
        let store = JsonNotesStore::at(dir.notes_file());
        let note = store.add("temp only", &c_six(), "C6").unwrap();
        store.all_notes().unwrap();
        store.notes(&c_six()).unwrap();
        store.delete(&note.id).unwrap();

        assert_eq!(PathSnapshot::of(&real), before);
        assert_eq!(PathSnapshot::of(&real).exists, before.exists);
    }

    #[test]
    fn every_store_in_this_suite_lives_under_the_temporary_directory() {
        let dir = TempDirectory::new();
        let store = JsonNotesStore::at(dir.notes_file());

        let temp = std::env::temp_dir().canonicalize().unwrap();
        let location = store.location().parent().unwrap().canonicalize().unwrap();
        assert!(location.starts_with(&temp));
        assert!(
            !store
                .location()
                .to_string_lossy()
                .contains("Application Support")
        );
    }
}

// MARK: - Michael's real file, by copy

/// Reads a copy of the real notes file and asserts its count and one known
/// note. The copy lives outside the repo (it is his data), at the path in
/// `WATCHORD_REAL_NOTES_COPY`; run with `--ignored` to include it.
#[test]
#[ignore = "needs WATCHORD_REAL_NOTES_COPY pointing at a copy of the real notes file"]
fn the_copy_of_the_real_file_reads_back_in_full() {
    let copy = std::env::var("WATCHORD_REAL_NOTES_COPY").expect("WATCHORD_REAL_NOTES_COPY");
    let dir = TempDirectory::new();
    fs::copy(&copy, dir.notes_file()).unwrap();

    let notes = JsonNotesStore::at(dir.notes_file()).all_notes().unwrap();

    assert_eq!(notes.len(), 2, "the copy taken 2026-08-28 holds two notes");
    let known = notes
        .iter()
        .find(|n| n.chord_key.raw() == "0.3.7.8")
        .expect("the Ab major 7 over C note");
    assert_eq!(known.text, "ades t4/t8 stack");
    assert_eq!(known.spelling_when_written, "AbΔ7/C");
    assert_eq!(known.id, "7F6D5CF9-6CC7-4F25-9AFF-C604CA1EFB60");
    // Nothing was moved aside: the real file's envelope is this build's.
    assert!(dir.damaged_files().is_empty());
}
