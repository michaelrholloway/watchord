---
status: accepted
date: 2026-09-03
---

# One value, two skins, one pipe

## Context

Michael designed watchord's look before he knew what the app should show. He
now wants to see every fact the app can produce, as plain text, before he
designs again — *"the most basic skin, just white and black, minimal terminal
ui"* that outputs *"all data, simple text"* (spec #9). Other programs want the
same facts as JSON (pipe mode). And PUSH must keep drawing exactly as it does.

Before this decision the PUSH screens read `AppModel` directly through a dozen
accessors, and `--print` read the same accessors a second time and chose which
fields to print. Two readers of one model can disagree by omission: a field the
model gains reaches the screen and not the pipe, or the reverse, and nothing
reports it.

## Decision

**One value.** `watchord_model::Frame` holds everything the screen shows: the
screen, the plates, the headline as text and as a reading, every alternate with
its score, origin, root, fit and claimed pitch classes, the keys, the key, the
sounding set, the notes on the chord, every note group, the draft, and an
`Annotations` slot. `AppModel::frame()` builds it. It derives `Serialize` and
`Deserialize`, and it round-trips through JSON to an equal value; the wire
shape is the value, not the struct (a sounding set is `[60,64,67,69]`, a key is
`"0.4.7.9"`), by the same reasoning as `PitchClass` and `ChordKey`.

**Two skins.** `watchord_tui::Skin` is `Push` or `Plain`, parsed from
`--skin push|plain`, default PUSH. Both draw a `&Frame` into the same buffer
and fill the same `Hits`, so one event loop serves both and the mouse and every
key work the same way. PUSH's `screens.rs` now reads the frame instead of the
model; its snapshots did not change, which is the proof that the frame carries
what the screen showed. The plain skin (`plain.rs`) uses the terminal's own
foreground and background, no colour token, no box-drawing character, labels
in UPPERCASE, tables aligned with spaces, the headline as one line. Every field
draws; an absent optional field draws its label with `—`. Below 30 rows it
draws what fits, top down, with the note field and the key reference kept at
the foot. What the skins share is layout and hit-testing; what they do not
share is paint.

**One pipe.** `--print` prints every field of the frame as `label: value`
lines, with a fixed label set (`Frame::LABELS`). `--json` prints the frame as
one JSON line per settled sounding set. Both format the same value the skins
draw, so the screen and the pipe cannot disagree.

**The annotations slot.** `Annotations` is an empty struct today, read with
`#[serde(default)]` so a frame written before the first annotation still parses
after it. The theory crate (spec #9, *Structure*) fills it: voicing, inversion,
numeral, staff. An annotation never changes a reading or its rank.

**The `Frame` trait in `watchord-core` is gone.** It was declared as the
terminal skin's seam, "minimal on purpose — the tui ticket grows it", and
nothing ever implemented it. One name, one meaning: `Frame` is the value.

## Consequences

- A field added to the model reaches both skins, `--print` and `--json` by
  being added to `Frame` — and a test greps each plain snapshot and the print
  output for every label in `Frame::LABELS`, with a control label proven
  absent, so a field that is not drawn fails a test.
- The plain skin's two screens split the labels: Now Playing carries the
  readings, annotations, notes and draft; All Notes carries the groups. A test
  asserts the union of the two label sets is the whole list.
- `watchord-model` depends on `serde` and turns on `watchord-core`'s `serde`
  feature. `SpellingOrigin`, `ChordFit`, `ChordNote` and `SoundingSet` gained
  feature-gated serde; the notes file schema (ADR-0004) is untouched, because
  the store crate keeps its own records.
- PUSH's look does not change. Its snapshot tests are the gate.
- The styled skin Michael designs in Figma is the next run's work. Whether it
  replaces PUSH or sits beside it as a third `Skin` is his call then.
