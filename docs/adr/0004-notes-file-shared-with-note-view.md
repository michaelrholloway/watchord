---
status: accepted
date: 2026-08-28
---

# The notes file is shared with note-view

On macOS, watchord reads and writes **the same notes file, in the same schema, as
note-view**. Michael's decision at alignment, 2026-08-28. A note written in either
app resurfaces in both, because there is one file.

## The file

- **macOS path:** `~/Library/Application Support/note-view/notes.json`. This is
  the path note-view resolves as its default store, and watchord uses it
  unchanged, including the `note-view` directory name.
- **Linux and Windows:** the platform data directory by the `directories` crate's
  conventions — `$XDG_DATA_HOME/watchord/notes.json` (default
  `~/.local/share/watchord/notes.json`) on Linux,
  `%APPDATA%\watchord\notes.json` on Windows — same schema.

## The schema

The envelope is `{"version": 1, "notes": [...]}`. Each note is:

```json
{
  "id": "6F9619FF-8B86-D011-B42D-00C04FC964FF",
  "chordKey": "0.4.7.9",
  "spellingWhenWritten": "C6",
  "text": "the one from the bridge",
  "createdAt": "2026-08-28T18:00:00Z"
}
```

- `id` — a UUID string.
- `chordKey` — the chord key: the canonical pitch-class set, dot-joined, ascending,
  no duplicates. A non-canonical key is a corrupt entry, not a different chord.
- `spellingWhenWritten` — the headline on screen when the note was written.
- `text` — the note.
- `createdAt` — an ISO-8601 instant.

note-view writes the file pretty-printed with sorted keys, and writes it
atomically: a temp file in the same directory, flushed, then renamed over the
target. watchord does the same, so neither app can read a half-written file from
the other.

note-view reads defensively: one entry that fails to decode is skipped and counted,
not fatal; a file whose `version` this build does not know is moved aside as
`notes.json.damaged-<stamp>` and never rewritten. watchord carries the same
behaviour, because both apps share the same bytes.

## Consequences

- The schema is a public contract between two repos. A schema change needs both
  apps, and a `version` bump, before either ships it.
- The macOS path names `note-view`, not `watchord`. That is deliberate: the file
  belongs to the chord, not to the app that drew it.
