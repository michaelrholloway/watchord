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

- **The headline and the staff are pictures where the terminal can show one.**
  A terminal has one type size, so a larger headline in the same face is only
  possible as a picture placed in the cell grid — the kitty graphics protocol
  (Ghostty, kitty, WezTerm), sixel, or iTerm2. `ratatui-image` asks the
  terminal what it speaks before the alternate screen opens. With a picture:
  the headline is JetBrains Mono Bold at 1.1× the cell height, lime, two rows
  tall; the staff is drawn at pixel size with lines half a cell apart, square
  heads centred on their line or their space, italic sharps centred on their
  head, and short ledger lines out to the farthest note — the design's
  geometry, exactly. The two font files are embedded (SIL OFL 1.1,
  `assets/OFL.txt`). Michael ruled the block figure out for this skin. A
  terminal with none of the protocols gets one bold line and the text staff.
  Known cost: each new picture stays in the terminal's memory for the session.
- The text staff is nine rows a clef, one row per position, so every head sits
  exactly on its line or in its space. Two rows separate the staves; spare rows
  go above and below. The half-row scheme tried in between could not place a
  head or a sharp exactly and was dropped.
- No rule rows between sections. A rule row is a whole terminal row and reads
  as a gap above the header, not as a border. The border is an **underline on
  every cell of a section's last row**, drawn in ink: the line sits at the foot
  of that row and the next header starts flush against it, at no cost in rows.
  The trade is that the last row's text is underlined too. The title bar's
  border is the same underline, so the KEY row starts on row 2. Every row is
  one terminal row, and there is no padding between rows: a cell is the unit. History keys are filled chips, as drawn, and chips in adjacent
  rows touch.
- Note heads are `■` on one column; an accidental sits in the cell to the left
  and never moves the head. Two heads on one row (D3 and D#3) sit two cells
  apart.
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
