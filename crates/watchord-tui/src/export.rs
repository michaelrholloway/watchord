//! Session export: a markdown file and a JSON-lines file, both built from the
//! same `Frame` — its `history` strip and, per entry, the notes on that
//! chord's key out of `Frame::groups`. Ticket 12.
//!
//! Written to a `sessions` directory beside the real notes file, never the
//! notes file itself. In a test, `directory` is a temp dir, so nothing here
//! ever has to know it is being tested.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use watchord_model::{Frame, FrameHistoryEntry};

/// `sessions`, beside wherever the real notes file lives on this machine.
/// `None` only on a machine with no home directory to speak of.
pub fn sessions_directory() -> Option<PathBuf> {
    let notes_path = watchord_store::default_path()?;
    Some(notes_path.parent()?.join("sessions"))
}

/// `YYYY-MM-DD-HHMM`, sliced out of the store's own no-colons stamp so a
/// filename never reimplements date arithmetic.
fn filename_stamp(at: SystemTime) -> String {
    let full = watchord_store::iso8601::quarantine_stamp(at); // "2026-02-02T024000"
    format!("{}-{}", &full[0..10], &full[11..15])
}

/// The notes on `entry`'s chord, in the order `Frame::groups` already holds
/// them (newest first).
fn notes_for<'a>(frame: &'a Frame, entry: &FrameHistoryEntry) -> Vec<&'a str> {
    frame
        .groups
        .iter()
        .find(|group| group.key == entry.key)
        .map(|group| group.notes.iter().map(|note| note.text.as_str()).collect())
        .unwrap_or_default()
}

fn time_of_day(entry: &FrameHistoryEntry) -> String {
    let instant = UNIX_EPOCH + Duration::from_secs(entry.at_unix_seconds);
    let full = watchord_store::iso8601::format(instant); // "2026-02-02T02:40:00Z"
    full[11..19].to_string() // "02:40:00"
}

fn voice_leading_cell(entry: &FrameHistoryEntry) -> String {
    match &entry.voice_leading {
        Some(vl) => format!(
            "{} semitones, {} common, {} max",
            vl.total_semitones, vl.common_tones_kept, vl.largest_move
        ),
        None => "—".to_string(),
    }
}

/// One markdown table row per history entry: time, name, since the one
/// before it, voice leading, and the notes on that chord. Voicings and
/// numerals join this table once the tickets that annotate them land
/// (`Frame::annotations`, currently empty) — not this ticket's data.
fn markdown(frame: &Frame) -> String {
    let mut out = String::from("| Time | Chord | Since previous | Voice leading | Notes |\n");
    out.push_str("|---|---|---|---|---|\n");
    for entry in &frame.history {
        let since = entry
            .seconds_since_previous
            .map(|s| format!("{s}s"))
            .unwrap_or_else(|| "—".to_string());
        let notes = notes_for(frame, entry);
        let notes_cell = if notes.is_empty() {
            "—".to_string()
        } else {
            notes.join("; ")
        };
        out.push_str(&format!(
            "| {} | {} | {since} | {} | {notes_cell} |\n",
            time_of_day(entry),
            entry.name,
            voice_leading_cell(entry),
        ));
    }
    out
}

/// One history entry, as one line of the JSON-lines export.
#[derive(Serialize)]
struct ExportLine<'a> {
    at: String,
    name: &'a str,
    seconds_since_previous: Option<u64>,
    voice_leading: Option<&'a watchord_model::FrameVoiceLeading>,
    notes: Vec<&'a str>,
}

fn json_lines(frame: &Frame) -> String {
    let mut out = String::new();
    for entry in &frame.history {
        let instant = UNIX_EPOCH + Duration::from_secs(entry.at_unix_seconds);
        let line = ExportLine {
            at: watchord_store::iso8601::format(instant),
            name: &entry.name,
            seconds_since_previous: entry.seconds_since_previous,
            voice_leading: entry.voice_leading.as_ref(),
            notes: notes_for(frame, entry),
        };
        // Plain data with no map keys that can fail to serialise.
        out.push_str(&serde_json::to_string(&line).expect("an ExportLine serialises"));
        out.push('\n');
    }
    out
}

/// Writes the session's markdown table into `directory` (created if it does
/// not exist), named `YYYY-MM-DD-HHMM.md` from `at`. Returns the file's path.
pub fn write_markdown(frame: &Frame, directory: &Path, at: SystemTime) -> io::Result<PathBuf> {
    fs::create_dir_all(directory)?;
    let path = directory.join(format!("{}.md", filename_stamp(at)));
    fs::write(&path, markdown(frame))?;
    Ok(path)
}

/// Writes one JSON object per history entry into `directory` (created if it
/// does not exist), named `YYYY-MM-DD-HHMM.jsonl` from `at`. Returns the
/// file's path.
pub fn write_json_lines(frame: &Frame, directory: &Path, at: SystemTime) -> io::Result<PathBuf> {
    fs::create_dir_all(directory)?;
    let path = directory.join(format!("{}.jsonl", filename_stamp(at)));
    fs::write(&path, json_lines(frame))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration as StdDuration;

    use watchord_core::{ChordNote, SoundingSet};
    use watchord_model::AppModel;
    use watchord_model::fakes::{InMemoryNoteStore, ScriptedSoundingSetSource, StubChordNaming};

    use super::*;

    fn model_with_history_and_a_note() -> AppModel {
        let c6 = SoundingSet::new([60, 64, 67, 69]);
        let f = SoundingSet::new([53, 57, 60]);
        let store = InMemoryNoteStore::new(vec![ChordNote::new(
            "1",
            c6.key(),
            "C6",
            "sounds like the Rhodes",
            UNIX_EPOCH,
        )]);
        let mut model = AppModel::new(
            Arc::new(StubChordNaming::new()),
            Box::new(ScriptedSoundingSetSource::new(vec![c6, f])),
            Arc::new(store),
        );
        model.start();
        while model.wait(StdDuration::from_millis(50)) {}
        model
    }

    #[test]
    fn markdown_writes_a_file_the_test_reads_back_and_never_touches_notes_json() {
        let model = model_with_history_and_a_note();
        let dir = tempdir();
        let at = UNIX_EPOCH + StdDuration::from_secs(1_770_000_000);
        let path = write_markdown(&model.frame(), &dir, at).expect("writes");
        assert_eq!(path.file_name().unwrap(), "2026-02-02-0240.md");
        let text = fs::read_to_string(&path).expect("reads back");
        assert!(text.contains("| Time | Chord |"), "{text}");
        assert!(text.contains("sounds like the Rhodes"), "{text}");
        assert!(text.contains("semitones"), "{text}");
        // The control: the real notes file lives nowhere near a temp dir.
        assert!(!dir.join("notes.json").exists());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn json_lines_writes_one_object_per_entry_the_test_reads_back() {
        let model = model_with_history_and_a_note();
        let dir = tempdir();
        let at = UNIX_EPOCH + StdDuration::from_secs(1_770_000_000);
        let path = write_json_lines(&model.frame(), &dir, at).expect("writes");
        assert_eq!(path.file_name().unwrap(), "2026-02-02-0240.jsonl");
        let text = fs::read_to_string(&path).expect("reads back");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "one line per history entry: {text}");
        let first: serde_json::Value = serde_json::from_str(lines[0]).expect("valid json");
        assert!(first.get("name").is_some());
        assert!(first.get("notes").is_some());
        assert!(!dir.join("notes.json").exists());
        fs::remove_dir_all(&dir).ok();
    }

    /// A fresh throwaway directory under the OS temp dir, unique per call.
    fn tempdir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "watchord-export-test-{nanos}-{:?}",
            std::thread::current().id()
        ));
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }
}
