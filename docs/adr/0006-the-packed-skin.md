---
status: accepted
date: 2026-09-09
---

# The packed skin

## Context

ADR-0005 left one question open: whether the skin Michael designs in Figma
replaces PUSH or sits beside it. He designed it — frame `48:2938`, `80×24
(packed)` — after seeing every field the plain skin can show, and he pared the
display down to what he wants to look at while he plays. He also ruled that no
feature leaves the code: *"I don't want you to remove the features from the
codebase, but many of them will not be displayed in the UI."*

## Decision

**A third skin, the default.** `Skin::Packed`, `--skin packed`, draws the
frame cell for cell at 80×24 and grows into a larger terminal. `watchord` with
no flags opens it. PUSH and plain stay, chosen by name, unchanged; their
snapshots are the gate that proves it.

**It shows what the design shows.** The headline with `≈` and the fit detail,
the staff, KEY / FUNCTION / NUMERAL / KEYS, READINGS (name, origin, fit,
numeral, function), HISTORY (name, keys, numeral, function against the current
key), NOTES with the field and the saved notes, and the foot: SUSTAIN,
ARPEGGIO, STATE, INPUT. All Notes draws in the same chrome. Everything else
the `Frame` carries — drill, settle, the sostenuto and soft pedals, score,
root, claimed, Nashville, the annotations, voice leading — stays in the
frame, the plain skin, `--print` and `--json`.

**No key without a readout.** The packed skin binds nothing to `p` (drill),
`-`/`+` (settle), `x`/Ctrl-x (export). Those letters type into the note field
instead. A mode you can switch on with no sign on screen is a trap. It adds
`a` (arpeggio, already on the foot) and `n` (switch screen), as the design's
`[A]` and `[N]` say. The event loop takes the skin's key table; one loop still
serves every skin.

**Terminal departures from the frame**, with reasons:

- The headline is one bold line. A terminal has one type size, and Michael
  ruled the block figure (ADR-0003 (e)) out for this skin: *"it should just be
  larger text in the same font."* Ticket #8, kitty graphics, is the path to
  that.
- The design's section borders are rule rows. A rule row costs a whole line, so
  at 24 rows there are none; the first three spare rows draw them, before any
  list grows. A header sits flush under its rule: a list never keeps a blank
  row, and rows it cannot fill move to the notes at the foot of the panel.
- Every row is one terminal row; nothing is double-spaced. The history's keys
  are plain text rather than filled chips, because filled chips in adjacent
  rows touch.
- Note heads are `■` on one column; an accidental sits in the cell to the left
  and never moves the head. Two heads on one row (D3 and D#3) sit two cells
  apart.
- Treble staff only at 24 rows; both clefs from 34. A five-line staff is nine
  terminal rows, and two do not fit in the ten-row box. The two staves sit
  tight, one row apart, with the spare rows above and below. When notes outrun
  the box, the empty staff edges give way first, then the ledger note farthest
  from the staff.
- The table grid is fixed from the left. FIT widens with the terminal up to 26
  cells; NUMERAL and FUNCTION follow it rather than hanging off the far edge.
- The FIT column shows the tier word; the detail sits under the spoken name in
  the headline box. At 80 columns the column has eight cells.
- `SAVE[⏎]`, not `SAVE[S]`: the field takes every printable key.
- `KEYS`, not `NOTES`, for the sounding keys, on the chip row and in HISTORY.
  The glossary's word; NOTES is the text-notes section.
- STATE reads `PLAYING` / `RELEASED` / `IDLE` with one dot, lit while playing.

**The frame grew, the model grew.** `FrameHistoryEntry` carries `keys`,
`numeral` and `function` so the HISTORY table can read them against the
current key; all three default when absent, so an older JSON line still
parses. `AppModel::toggle_arpeggio` is the sostenuto pedal's flip, callable
from a key.

## Consequences

- Three skins share one `Frame`, one `Hits`, one event loop. A field added to
  the frame reaches the pipe and the plain skin as before; the packed skin
  shows it only if the design does.
- The packed skin's snapshots (`tests/snapshots/packed-*.txt`) are its gate,
  at 80×24, 80×30, 80×34 and 120×40, for the live, released, nearest,
  declined, idle, stepped and selected states and for All Notes.
- Tickets #17 (fit every field at 44 rows) and #19 (pick a variant) close
  here: the packed skin is the pick, and it does not try to fit every field.
- A designed All Notes screen is later work. Until then it reuses the packed
  chrome and grid.
