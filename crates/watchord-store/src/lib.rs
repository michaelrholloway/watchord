//! watchord-store: `NoteStoring` over note-view's JSON file.
//!
//! Ported from note-view's `Sources/NotesStore/JSONNotesStore.swift` and
//! `StoreFile.swift`. The file is a contract between two apps (ADR-0004): the
//! same bytes, the same path on macOS, so a note written in either app resurfaces
//! in both.
//!
//! The governing requirement is that a note Michael wrote is never lost, and
//! every awkward-looking decision follows from it rather than from tidiness:
//!
//! - Writes go to a temp file in the same directory, are flushed to disk, and
//!   are then renamed over the target. A crash at any instant leaves either the
//!   whole old file or the whole new one, never a truncated one.
//! - A file we cannot fully understand is **preserved** before it is replaced,
//!   including a file that parses cleanly while some of its *entries* do not.
//! - A key that cannot round-trip is refused at `add` rather than written.

pub mod drill_store;
pub mod iso8601;

pub use drill_store::{DRILL_FILE_NAME, JsonDrillStore};

use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use watchord_core::{ChordKey, ChordNote, NoteStoring, StoreError};

/// Something the store refused to do, rather than do badly.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum NotesStoreError {
    /// The chord key cannot be written and read back — so a note saved against
    /// it would vanish at the next launch. Refused at write time instead.
    #[error(
        "Refusing to save a note against the chord key '{}', which is not canonical and \
         could not be read back. Nothing was written.",
        if .0.is_empty() { "(empty)" } else { .0 }
    )]
    UnusableChordKey(String),

    /// A write could not be completed. The previous contents of the store are
    /// untouched; nothing was overwritten.
    #[error("Could not write the notes store at {}: {reason}. The previous notes are intact.", path.display())]
    WriteFailed {
        /// The file that was being written.
        path: PathBuf,
        /// The OS's own words.
        reason: String,
    },

    /// The file is there and could not be read — a permissions problem, a bad
    /// volume. Pretending the store is empty would invite a fresh one over the
    /// top of notes that are merely out of reach.
    #[error("Could not read the notes store at {}: {reason}.", path.display())]
    ReadFailed {
        /// The file that was being read.
        path: PathBuf,
        /// The OS's own words.
        reason: String,
    },
}

/// The seam's error is the coarser one: a refusal to write an unreadable key is
/// reported as `Corrupt` (the key is not the schema), and a failed read or
/// write as `Io`, each carrying this crate's fuller sentence.
impl From<NotesStoreError> for StoreError {
    fn from(error: NotesStoreError) -> Self {
        match error {
            NotesStoreError::UnusableChordKey(_) => StoreError::Corrupt(error.to_string()),
            NotesStoreError::WriteFailed { .. } | NotesStoreError::ReadFailed { .. } => {
                StoreError::Io(error.to_string())
            }
        }
    }
}

/// The store's filename wherever it lives.
pub const DEFAULT_FILE_NAME: &str = "notes.json";

/// Bumped when the shape changes. Present from the first release so a later
/// shape change is a version check rather than a hand migration.
pub const CURRENT_VERSION: u64 = 1;

/// The clock that stamps new notes. Injectable so ordering tests are
/// deterministic instead of depending on how fast the machine is.
pub type Clock = Box<dyn Fn() -> SystemTime + Send + Sync>;

/// `NoteStoring` over a single JSON file.
pub struct JsonNotesStore {
    path: PathBuf,
    now: Clock,
    /// Guards the cache and serialises read-modify-write cycles. Held across
    /// the whole load/mutate/persist sequence, not just the cache assignment:
    /// two concurrent `add`s that each read, append, and write would otherwise
    /// race and one note would be lost.
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    /// `None` until the first access. Loading is lazy so that constructing the
    /// store touches no disk at all and cannot fail.
    cache: Option<Vec<ChordNote>>,
    /// Set at load when the bytes on disk hold something we did not fully
    /// understand, and cleared once we have preserved a copy of them.
    must_preserve_before_writing: bool,
}

impl JsonNotesStore {
    /// A store at `path`. Injectable so that no test has to go anywhere near
    /// the real one.
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self::with_clock(path, Box::new(SystemTime::now))
    }

    /// A store at `path` whose new notes are stamped by `now` instead of the
    /// wall clock. For tests that need a known order.
    pub fn with_clock(path: impl Into<PathBuf>, now: Clock) -> Self {
        JsonNotesStore {
            path: path.into(),
            now,
            state: Mutex::new(State::default()),
        }
    }

    /// The real store: note-view's file on macOS, the platform data directory
    /// elsewhere. See [`default_path`]. `None` only on a machine with no home
    /// directory.
    pub fn real() -> Option<Self> {
        default_path().map(Self::at)
    }

    /// A store at `notes.json` inside `directory`, creating the directory if it
    /// is not already there. Unlike the other constructors this one touches the
    /// disk, which is why it fails: a caller who nominates a location generally
    /// wants to hear about a bad one now rather than at the first write.
    pub fn in_directory(directory: &Path) -> io::Result<Self> {
        fs::create_dir_all(directory)?;
        Ok(Self::at(directory.join(DEFAULT_FILE_NAME)))
    }

    /// Where this particular store reads and writes. Exposed so a test can
    /// assert it is nowhere near the developer's real notes.
    pub fn location(&self) -> &Path {
        &self.path
    }

    // MARK: - Reading

    /// The cached notes, newest first, loading them on first use. The caller
    /// holds the lock.
    fn loaded_notes(&self, state: &mut State) -> Result<Vec<ChordNote>, NotesStoreError> {
        if let Some(cache) = &state.cache {
            return Ok(cache.clone());
        }
        let loaded = self.load_from_disk()?;
        let degraded = loaded.is_degraded();
        let version = loaded.version;
        let ordered = newest_first(loaded.notes);
        state.cache = Some(ordered.clone());

        if !degraded {
            return Ok(ordered);
        }
        if version == Some(CURRENT_VERSION) {
            // The envelope was ours and some entries inside it were not. The
            // file is still the live store, so it stays where it is — but it
            // must be copied aside before we ever replace it, because those
            // unreadable entries exist nowhere else.
            state.must_preserve_before_writing = true;
            return Ok(ordered);
        }
        // The envelope itself is beyond us: unparseable, no version, or a
        // version a later build wrote. Move it aside now, while it still exists.
        self.move_damaged_file_aside();
        if !ordered.is_empty() {
            // We understood some of a file we are no longer keeping, so write
            // those notes forward into a file this build owns. Best effort: a
            // read-only volume must not turn a read into a failure.
            let _ = self.persist(state, ordered.clone());
        }
        Ok(ordered)
    }

    fn load_from_disk(&self) -> Result<LoadedStore, NotesStoreError> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(LoadedStore::fresh()),
            Err(error) => {
                return Err(NotesStoreError::ReadFailed {
                    path: self.path.clone(),
                    reason: error.to_string(),
                });
            }
        };
        Ok(decode_store(&bytes))
    }

    // MARK: - Writing

    /// The caller holds the lock.
    fn persist(&self, state: &mut State, notes: Vec<ChordNote>) -> Result<(), NotesStoreError> {
        let ordered = newest_first(notes);
        if state.must_preserve_before_writing {
            // A partially-readable file is still sitting at `path` and is about
            // to be replaced. Copy it aside first.
            self.copy_aside();
            state.must_preserve_before_writing = false;
        }
        let bytes = encode_store(&ordered);
        self.write_atomically(&bytes)?;
        // Only after the bytes are safely on disk. If the write failed, the
        // cache keeps its previous value and the in-memory view still matches
        // the file.
        state.cache = Some(ordered);
        Ok(())
    }

    /// Temp file in the same directory, flushed, then renamed over the target.
    ///
    /// Same directory is not incidental — a rename is only atomic within a
    /// filesystem, and a temp file elsewhere degrades the rename into a copy
    /// that can be interrupted half-way.
    fn write_atomically(&self, bytes: &[u8]) -> Result<(), NotesStoreError> {
        let directory = self
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let write_failed = |reason: String| NotesStoreError::WriteFailed {
            path: self.path.clone(),
            reason,
        };

        fs::create_dir_all(&directory)
            .map_err(|e| write_failed(format!("could not create {}: {e}", directory.display())))?;

        let file_name = self
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| DEFAULT_FILE_NAME.to_string());
        let temp = directory.join(format!(".{file_name}.tmp-{}", uuid::Uuid::new_v4()));

        let written = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| {
                write_failed(format!(
                    "could not create a temporary file in {}: {e}",
                    directory.display()
                ))
            })
            .and_then(|mut file| {
                file.write_all(bytes)
                    .and_then(|()| file.sync_all())
                    .map_err(|e| write_failed(e.to_string()))
            });
        if let Err(error) = written {
            let _ = fs::remove_file(&temp);
            return Err(error);
        }

        // `rename` replaces the destination in one step, so there is no instant
        // at which the store does not exist.
        if let Err(error) = fs::rename(&temp, &self.path) {
            let _ = fs::remove_file(&temp);
            return Err(write_failed(error.to_string()));
        }

        // Flush the directory entry too, so the rename itself survives a power
        // cut. Directories cannot be opened for sync on Windows; there the
        // rename is what NTFS journals.
        #[cfg(unix)]
        if let Ok(dir) = fs::File::open(&directory) {
            let _ = dir.sync_all();
        }
        Ok(())
    }

    // MARK: - Preserving damaged files

    /// Move the file to a sibling this build will never write to. Moving
    /// rather than copying means the same damage is not re-reported on every
    /// launch, and the bytes survive at the new name.
    fn move_damaged_file_aside(&self) {
        if let Some(destination) = self.available_quarantine_path() {
            let _ = fs::rename(&self.path, destination);
        }
    }

    fn copy_aside(&self) {
        if let Some(destination) = self.available_quarantine_path() {
            let _ = fs::copy(&self.path, destination);
        }
    }

    /// A sibling path that does not exist yet. Never overwrites an earlier
    /// quarantine — two damaged generations are two files, because one of them
    /// may be the only copy of a note.
    fn available_quarantine_path(&self) -> Option<PathBuf> {
        if !self.path.exists() {
            return None;
        }
        let directory = self.path.parent()?;
        let file_name = self.path.file_name()?.to_string_lossy();
        let stamp = iso8601::quarantine_stamp((self.now)());
        let base = format!("{file_name}.damaged-{stamp}");
        (0..1000)
            .map(|suffix| {
                if suffix == 0 {
                    directory.join(&base)
                } else {
                    directory.join(format!("{base}-{suffix}"))
                }
            })
            .find(|candidate| !candidate.exists())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        // A poisoned lock means a thread panicked mid-operation; the cache may
        // be stale but never inconsistent with a *completed* write, and
        // dropping it forces a reload. Carry on.
        self.state.lock().unwrap_or_else(|poisoned| {
            let mut guard = poisoned.into_inner();
            guard.cache = None;
            guard
        })
    }
}

impl NoteStoring for JsonNotesStore {
    fn notes(&self, key: &ChordKey) -> Result<Vec<ChordNote>, StoreError> {
        let mut state = self.lock();
        let notes = self.loaded_notes(&mut state)?;
        Ok(notes.into_iter().filter(|n| n.chord_key == *key).collect())
    }

    fn all_notes(&self) -> Result<Vec<ChordNote>, StoreError> {
        let mut state = self.lock();
        Ok(self.loaded_notes(&mut state)?)
    }

    fn add(&self, text: &str, key: &ChordKey, spelling: &str) -> Result<ChordNote, StoreError> {
        // Refused before anything is written. A key that does not survive a
        // round trip through `ChordKey::parse` decodes as a skipped entry on the
        // way back in, so the note would be written now and lost forever after.
        if ChordKey::parse(key.raw()).is_none() {
            return Err(NotesStoreError::UnusableChordKey(key.raw().to_string()).into());
        }
        let note = ChordNote::new(
            uuid::Uuid::new_v4().to_string().to_uppercase(),
            key.clone(),
            spelling,
            text,
            (self.now)(),
        );
        let mut state = self.lock();
        let mut notes = self.loaded_notes(&mut state)?;
        notes.push(note.clone());
        self.persist(&mut state, notes)?;
        Ok(note)
    }

    fn update(&self, id: &str, text: &str) -> Result<ChordNote, StoreError> {
        let mut state = self.lock();
        let mut notes = self.loaded_notes(&mut state)?;
        let index = notes
            .iter()
            .position(|n| n.id == id)
            .ok_or_else(|| StoreError::NoSuchNote(id.to_string()))?;
        notes[index].text = text.to_string();
        let updated = notes[index].clone();
        self.persist(&mut state, notes)?;
        Ok(updated)
    }

    fn delete(&self, id: &str) -> Result<(), StoreError> {
        let mut state = self.lock();
        let mut notes = self.loaded_notes(&mut state)?;
        let Some(index) = notes.iter().position(|n| n.id == id) else {
            // Deleting something that is not there is a no-op, not an error —
            // two windows showing the same list would otherwise race.
            return Ok(());
        };
        notes.remove(index);
        self.persist(&mut state, notes)?;
        Ok(())
    }
}

/// Newest first, deterministically.
///
/// `created_at` alone is not a total order — two notes written in the same
/// instant would come back in whichever order the sort happened to produce. The
/// id breaks the tie with something that is on disk, so the order is identical
/// on every reload.
fn newest_first(mut notes: Vec<ChordNote>) -> Vec<ChordNote> {
    notes.sort_by(|left, right| {
        right
            .created_at
            .cmp(&left.created_at)
            .then_with(|| right.id.cmp(&left.id))
    });
    notes
}

// MARK: - The file format

/// One entry as it sits on disk. Field names are the contract, and they are
/// declared in sorted order because that is the order they are written in.
#[derive(Serialize, Deserialize)]
struct NoteRecord {
    #[serde(rename = "chordKey")]
    chord_key: ChordKey,
    #[serde(rename = "createdAt")]
    created_at: String,
    id: String,
    #[serde(rename = "spellingWhenWritten")]
    spelling_when_written: String,
    text: String,
}

impl NoteRecord {
    fn from_note(note: &ChordNote) -> Self {
        NoteRecord {
            chord_key: note.chord_key.clone(),
            created_at: iso8601::format(note.created_at),
            id: note.id.clone(),
            spelling_when_written: note.spelling_when_written.clone(),
            text: note.text.clone(),
        }
    }

    fn into_note(self) -> Option<ChordNote> {
        let created_at = iso8601::parse(&self.created_at)?;
        Some(ChordNote::new(
            self.id,
            self.chord_key,
            self.spelling_when_written,
            self.text,
            created_at,
        ))
    }
}

/// The envelope on the way out: `{"notes": [...], "version": 1}`. Encoding is
/// total — there is no such thing as a skipped entry on the way out.
#[derive(Serialize)]
struct Envelope {
    notes: Vec<NoteRecord>,
    version: u64,
}

/// What a load actually managed to understand.
struct LoadedStore {
    notes: Vec<ChordNote>,
    /// The `version` the file declared, or `None` if we never got that far.
    version: Option<u64>,
    /// Entries that were present and could not be decoded — a non-canonical
    /// `chordKey`, a missing field, a malformed date.
    skipped_entry_count: usize,
}

impl LoadedStore {
    /// Nothing on disk yet. Stamped with the current version rather than
    /// `None`: there are no bytes to preserve, so a brand-new store must not
    /// look degraded.
    fn fresh() -> Self {
        LoadedStore {
            notes: Vec::new(),
            version: Some(CURRENT_VERSION),
            skipped_entry_count: 0,
        }
    }

    /// The file was there and we could not make sense of any of it.
    fn unreadable() -> Self {
        LoadedStore {
            notes: Vec::new(),
            version: None,
            skipped_entry_count: 0,
        }
    }

    /// True when the bytes on disk hold something we did not fully understand,
    /// so rewriting this file would discard part of what Michael wrote.
    fn is_degraded(&self) -> bool {
        self.version != Some(CURRENT_VERSION) || self.skipped_entry_count > 0
    }
}

/// Decodes the envelope. Entry-level failures are absorbed and surface as a
/// count; anything that stops us reading the envelope itself — not JSON, not an
/// object, no integer `version` — is an unreadable file.
fn decode_store(bytes: &[u8]) -> LoadedStore {
    let Ok(Value::Object(mut root)) = serde_json::from_slice::<Value>(bytes) else {
        return LoadedStore::unreadable();
    };
    let Some(version) = root.get("version").and_then(Value::as_u64) else {
        return LoadedStore::unreadable();
    };
    // A file with no `notes` array at all is empty, not broken — that is a
    // store nothing has been written to yet.
    let entries: Vec<Value> = match root.remove("notes") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(entries)) => entries,
        Some(_) => return LoadedStore::unreadable(),
    };
    let total = entries.len();
    let notes: Vec<ChordNote> = entries
        .into_iter()
        .filter_map(|entry| serde_json::from_value::<NoteRecord>(entry).ok())
        .filter_map(NoteRecord::into_note)
        .collect();
    LoadedStore {
        skipped_entry_count: total - notes.len(),
        notes,
        version: Some(version),
    }
}

/// Pretty-printed with sorted keys, the way Foundation writes it: two-space
/// indent and `"key" : value`. Byte-compatible with note-view's own output, so
/// a diff between the two apps' writes of the same notes is empty.
fn encode_store(notes: &[ChordNote]) -> Vec<u8> {
    let envelope = Envelope {
        notes: notes.iter().map(NoteRecord::from_note).collect(),
        version: CURRENT_VERSION,
    };
    let mut out = Vec::new();
    let mut serializer =
        serde_json::Serializer::with_formatter(&mut out, FoundationFormatter::default());
    // Serialising plain structs and strings into a Vec cannot fail.
    let _ = envelope.serialize(&mut serializer);
    out
}

/// serde_json's pretty formatter with Foundation's ` : ` between key and value.
struct FoundationFormatter<'a> {
    inner: serde_json::ser::PrettyFormatter<'a>,
}

impl Default for FoundationFormatter<'_> {
    fn default() -> Self {
        FoundationFormatter {
            inner: serde_json::ser::PrettyFormatter::with_indent(b"  "),
        }
    }
}

impl serde_json::ser::Formatter for FoundationFormatter<'_> {
    fn begin_array<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.inner.begin_array(w)
    }
    fn end_array<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.inner.end_array(w)
    }
    fn begin_array_value<W: ?Sized + io::Write>(
        &mut self,
        w: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.inner.begin_array_value(w, first)
    }
    fn end_array_value<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.inner.end_array_value(w)
    }
    fn begin_object<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.inner.begin_object(w)
    }
    fn end_object<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.inner.end_object(w)
    }
    fn begin_object_key<W: ?Sized + io::Write>(
        &mut self,
        w: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.inner.begin_object_key(w, first)
    }
    fn begin_object_value<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        w.write_all(b" : ")
    }
    fn end_object_value<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.inner.end_object_value(w)
    }
}

// MARK: - Where the real file lives

/// The real store's path on this machine, or `None` when the platform has no
/// home directory to speak of.
///
/// - **macOS:** `~/Library/Application Support/note-view/notes.json` — note-view's
///   own file, unchanged, `note-view` directory name included (ADR-0004).
/// - **Linux:** `$XDG_DATA_HOME/watchord/notes.json`, default
///   `~/.local/share/watchord/notes.json`.
/// - **Windows:** `%APPDATA%\watchord\notes.json`.
pub fn default_path() -> Option<PathBuf> {
    let base = directories::BaseDirs::new()?;
    let directory = if cfg!(target_os = "macos") {
        base.home_dir()
            .join("Library")
            .join("Application Support")
            .join("note-view")
    } else {
        base.data_dir().join("watchord")
    };
    Some(directory.join(DEFAULT_FILE_NAME))
}
