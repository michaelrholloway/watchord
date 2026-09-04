//! `DrillStoring` over its own JSON file, `drill.json`, beside the notes
//! file but never inside it (ADR-0004, spec #9, ticket #14).
//!
//! Deliberately simpler than [`crate::JsonNotesStore`]: this file is not a
//! contract with note-view, so there is no byte-compatible-with-Foundation
//! format to hold and no damaged-file quarantine to run — a drill file that
//! cannot be parsed is treated as empty and rebuilt on the next write. The
//! one thing kept from the notes store is the part that matters most: writes
//! go to a temp file in the same directory and are renamed atomically over
//! the target, so a crash mid-write never truncates it.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use watchord_core::{ChordKey, DrillChordStat, DrillStoring, StoreError};

use crate::iso8601;

/// The store's filename wherever it lives.
pub const DRILL_FILE_NAME: &str = "drill.json";

/// Bumped when the shape changes.
pub const CURRENT_VERSION: u64 = 1;

/// `DrillStoring` over a single JSON file.
pub struct JsonDrillStore {
    path: PathBuf,
    lock: Mutex<()>,
}

impl JsonDrillStore {
    /// A store at `path`.
    pub fn at(path: impl Into<PathBuf>) -> Self {
        JsonDrillStore {
            path: path.into(),
            lock: Mutex::new(()),
        }
    }

    /// A store at `drill.json` next to `notes_path` — the sibling `--print`
    /// and the ticket both describe.
    pub fn beside_notes(notes_path: &Path) -> Self {
        let directory = notes_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        Self::at(directory.join(DRILL_FILE_NAME))
    }

    /// Where this store reads and writes. Exposed so a test can assert it is
    /// nowhere near a real file.
    pub fn location(&self) -> &Path {
        &self.path
    }

    fn read_all(&self) -> Vec<DrillChordStat> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(_) => return Vec::new(),
        };
        let Ok(envelope) = serde_json::from_slice::<Envelope>(&bytes) else {
            return Vec::new();
        };
        envelope
            .chords
            .into_iter()
            .filter_map(StatRecord::into_stat)
            .collect()
    }

    fn write_all(&self, stats: &[DrillChordStat]) -> Result<(), StoreError> {
        let envelope = Envelope {
            version: CURRENT_VERSION,
            chords: stats.iter().map(StatRecord::from_stat).collect(),
        };
        let bytes = serde_json::to_vec_pretty(&envelope)
            .map_err(|e| StoreError::Io(format!("could not encode drill.json: {e}")))?;
        self.write_atomically(&bytes)
    }

    fn write_atomically(&self, bytes: &[u8]) -> Result<(), StoreError> {
        let directory = self
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let io_failed =
            |reason: String| StoreError::Io(format!("could not write drill.json: {reason}"));

        fs::create_dir_all(&directory)
            .map_err(|e| io_failed(format!("could not create {}: {e}", directory.display())))?;

        let file_name = self
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| DRILL_FILE_NAME.to_string());
        let temp = directory.join(format!(".{file_name}.tmp-{}", uuid::Uuid::new_v4()));

        let written = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| {
                io_failed(format!(
                    "could not create a temporary file in {}: {e}",
                    directory.display()
                ))
            })
            .and_then(|mut file| {
                file.write_all(bytes)
                    .and_then(|()| file.sync_all())
                    .map_err(|e| io_failed(e.to_string()))
            });
        if let Err(error) = written {
            let _ = fs::remove_file(&temp);
            return Err(error);
        }
        if let Err(error) = fs::rename(&temp, &self.path) {
            let _ = fs::remove_file(&temp);
            return Err(io_failed(error.to_string()));
        }
        #[cfg(unix)]
        if let Ok(dir) = fs::File::open(&directory) {
            let _ = dir.sync_all();
        }
        Ok(())
    }

    fn hold(&self) -> std::sync::MutexGuard<'_, ()> {
        self.lock.lock().unwrap_or_else(|p| p.into_inner())
    }
}

impl DrillStoring for JsonDrillStore {
    fn stats(&self) -> Result<Vec<DrillChordStat>, StoreError> {
        let _guard = self.hold();
        Ok(self.read_all())
    }

    fn record(
        &self,
        key: &ChordKey,
        exact: bool,
        at: SystemTime,
    ) -> Result<DrillChordStat, StoreError> {
        let _guard = self.hold();
        let mut stats = self.read_all();
        let row = match stats.iter_mut().find(|s| s.chord_key == *key) {
            Some(row) => row,
            None => {
                stats.push(DrillChordStat::new(key.clone()));
                stats.last_mut().expect("just pushed")
            }
        };
        row.attempts += 1;
        if exact {
            row.exact += 1;
        }
        row.last_at = Some(at);
        let updated = row.clone();
        self.write_all(&stats)?;
        Ok(updated)
    }
}

// MARK: - The file format

#[derive(Serialize, Deserialize)]
struct StatRecord {
    #[serde(rename = "chordKey")]
    chord_key: ChordKey,
    attempts: u32,
    exact: u32,
    #[serde(rename = "lastAt")]
    last_at: Option<String>,
}

impl StatRecord {
    fn from_stat(stat: &DrillChordStat) -> Self {
        StatRecord {
            chord_key: stat.chord_key.clone(),
            attempts: stat.attempts,
            exact: stat.exact,
            last_at: stat.last_at.map(iso8601::format),
        }
    }

    fn into_stat(self) -> Option<DrillChordStat> {
        // `chord_key` already round-tripped through `ChordKey`'s own
        // validating `Deserialize` — a non-canonical key fails the whole
        // envelope's parse, at `read_all`, and the file reads as empty.
        Some(DrillChordStat {
            chord_key: self.chord_key,
            attempts: self.attempts,
            exact: self.exact,
            last_at: self.last_at.and_then(|s| iso8601::parse(&s)),
        })
    }
}

#[derive(Serialize, Deserialize)]
struct Envelope {
    version: u64,
    chords: Vec<StatRecord>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    fn key(values: &[i32]) -> ChordKey {
        ChordKey::new(values.iter().map(|&v| watchord_core::PitchClass::new(v)))
    }

    #[test]
    fn a_fresh_store_has_no_stats() {
        let dir = tempdir();
        let store = JsonDrillStore::at(dir.join("drill.json"));
        assert_eq!(store.stats().unwrap(), Vec::new());
        cleanup(&dir);
    }

    #[test]
    fn recording_creates_then_accumulates_a_row() {
        let dir = tempdir();
        let store = JsonDrillStore::at(dir.join("drill.json"));
        let c_major = key(&[0, 4, 7]);
        let at1 = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        let at2 = UNIX_EPOCH + Duration::from_secs(1_800_000_100);

        let first = store.record(&c_major, true, at1).unwrap();
        assert_eq!(first.attempts, 1);
        assert_eq!(first.exact, 1);
        assert_eq!(first.last_at, Some(at1));

        let second = store.record(&c_major, false, at2).unwrap();
        assert_eq!(second.attempts, 2);
        assert_eq!(second.exact, 1, "a miss does not increment exact");
        assert_eq!(second.last_at, Some(at2));

        let stats = store.stats().unwrap();
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].chord_key, c_major);
        cleanup(&dir);
    }

    /// The round trip the brief asks for: values written by one store handle
    /// come back byte-for-byte equal from a fresh one reading the same path.
    #[test]
    fn the_drill_file_round_trips_through_a_temp_dir() {
        let dir = tempdir();
        let path = dir.join("drill.json");
        let writer = JsonDrillStore::at(&path);
        let c_major = key(&[0, 4, 7]);
        let g7 = key(&[7, 11, 2, 5]);
        writer
            .record(
                &c_major,
                true,
                UNIX_EPOCH + Duration::from_secs(1_800_000_000),
            )
            .unwrap();
        writer
            .record(&g7, false, UNIX_EPOCH + Duration::from_secs(1_800_000_050))
            .unwrap();
        writer
            .record(
                &c_major,
                false,
                UNIX_EPOCH + Duration::from_secs(1_800_000_100),
            )
            .unwrap();

        let bytes_before = fs::read(&path).unwrap();

        let reader = JsonDrillStore::at(&path);
        let mut stats = reader.stats().unwrap();
        stats.sort_by(|a, b| a.chord_key.raw().cmp(b.chord_key.raw()));

        let mut expected = vec![
            DrillChordStat {
                chord_key: c_major.clone(),
                attempts: 2,
                exact: 1,
                last_at: Some(UNIX_EPOCH + Duration::from_secs(1_800_000_100)),
            },
            DrillChordStat {
                chord_key: g7.clone(),
                attempts: 1,
                exact: 0,
                last_at: Some(UNIX_EPOCH + Duration::from_secs(1_800_000_050)),
            },
        ];
        expected.sort_by(|a, b| a.chord_key.raw().cmp(b.chord_key.raw()));
        assert_eq!(stats, expected);

        // The bytes on disk did not move under a read: read-only re-encoding
        // never happens on `stats()`.
        let bytes_after = fs::read(&path).unwrap();
        assert_eq!(bytes_before, bytes_after);

        // The control: a deliberate change to the same file compares unequal,
        // so the equality above is not a check that always passes.
        writer
            .record(
                &c_major,
                true,
                UNIX_EPOCH + Duration::from_secs(1_800_000_200),
            )
            .unwrap();
        let bytes_changed = fs::read(&path).unwrap();
        assert_ne!(bytes_before, bytes_changed);

        cleanup(&dir);
    }

    /// The claim ticket #14 makes: `drill.json` lives beside the notes file
    /// but a drill session never writes a byte of it. Proven, not assumed —
    /// bytes read back before and after, plus a control that a deliberate
    /// change to the same file *does* compare unequal, so the equality above
    /// is not a check that always passes.
    #[test]
    fn a_drill_session_never_touches_the_notes_file() {
        use crate::JsonNotesStore;
        use watchord_core::NoteStoring;

        let dir = tempdir();
        let notes_path = dir.join("notes.json");
        let notes = JsonNotesStore::at(&notes_path);
        notes
            .add(
                "sounds like the Rhodes on Voodoo",
                &key(&[0, 4, 7, 9]),
                "C6",
            )
            .unwrap();

        let bytes_before = fs::read(&notes_path).unwrap();

        // A whole drill session: several attempts against a drill store
        // that lives right beside the notes file.
        let drill = JsonDrillStore::beside_notes(notes.location());
        assert_eq!(drill.location(), dir.join("drill.json"));
        let target = key(&[2, 5, 9]);
        for exact in [true, false, true, true, false] {
            drill
                .record(
                    &target,
                    exact,
                    UNIX_EPOCH + Duration::from_secs(1_800_000_000),
                )
                .unwrap();
        }

        let bytes_after = fs::read(&notes_path).unwrap();
        assert_eq!(
            bytes_before, bytes_after,
            "a drill session must never change the notes file"
        );

        // The control: a deliberate change to the notes file itself compares
        // unequal, so the equality above is a real check and not a tautology.
        notes.add("a second note", &key(&[2, 5, 9]), "Dm").unwrap();
        let bytes_changed = fs::read(&notes_path).unwrap();
        assert_ne!(bytes_before, bytes_changed);

        cleanup(&dir);
    }

    #[test]
    fn a_corrupt_file_reads_as_empty_rather_than_failing() {
        let dir = tempdir();
        let path = dir.join("drill.json");
        fs::write(&path, b"not json").unwrap();
        let store = JsonDrillStore::at(&path);
        assert_eq!(store.stats().unwrap(), Vec::new());
        cleanup(&dir);
    }

    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "watchord-drill-store-test-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }
}
