# watchord — domain context

The glossary for this repo. Terms here are load-bearing: they propagate from these
definitions into type names, spec text, and code comments, so a term used loosely
here becomes a wrong abstraction downstream.

> **Vocabulary warning.** Several terms below were **coined by an agent**, not by
> Michael. They are marked `[coined]`. Per cube's ratchet, a name the principal did
> not choose must not enter a spec unchallenged. The note-view terms came across
> with their markers intact; the terminal terms below carry their own.

## What the app is

A terminal app, for macOS, Linux and Windows, that watches MIDI input and names the
chord being played, in Michael's own notation, with every honest alternate reading
listed beneath it, and lets him attach text notes to a chord that resurface whenever
he plays it again. It is the same app as note-view — same model, same words, same
notes — drawn as a terminal skin instead of a macOS window. Michael's words: *"a
version of the app that is totally terminal based ... convert it into a terminal
skin ... fully port it - no missing functionality at all."*

## Core terms

### Pitch class

One of the twelve pitch classes, C=0 through B=11. **MIDI is spelling-blind** — it
transmits key numbers, never letter names — so a pitch class is the most the input
can ever tell us. This single fact shapes the whole product: the engine cannot
*read* a spelling, it can only **generate** candidate spellings and rank them.

### Sounding set `[coined]`

The distinct MIDI notes held at one instant, after sustain and settling have been
applied. The engine's input. Carries the raw notes, not just pitch classes, because
the **bass note** is the strongest single term in the ranking.

### Chord key `[coined]`

The identity a saved note attaches to: the canonical pitch-class set, written
`"0.4.7.9"`. Voicing-, octave-, inversion- and doubling-blind. Deliberately **not**
the displayed name — the name is a ranked opinion that we expect to tune, and
keying notes on an opinion would orphan them the first time a weight changed.

### Reading `[coined]`

One honest name for a sounding set — `C6`, `Am7`, `C#-dim7`. Every reading is
produced *forward* from the grammar: write a name, work out what it would sound
like, compare. Nothing here reads a name off a set of keys.

A reading carries the pitch classes **it claims** — its whole spelling, whether or
not all of it is being played — and a `fit` saying how that compares with the keys
that are down. Until ticket 20 the two had to be equal and a reading that was not an
exact match was suppressed; honesty is now *shown* rather than enforced by refusal.
See *Fit*.

### Fit `[coined]`

How well a reading matches the keys, drawn on screen beside the name. Four tiers:

| Tier | Meaning | Shown |
| ---- | ------- | ----- |
| `exact` | the reading's keys and the sounding keys are equal | *(nothing)* |
| `missing` | every sounding key is explained; the chord has tones you did not play | `·no5 ·no11` |
| `plus` | the whole chord is sounding, plus keys it cannot explain | `·+F#` (the extra key, named) |
| `nearest` | neither — the closest thing the vocabulary has | `≈` |

Michael, 2026-08-10: *"i would like for program to always try to suggest a chord, or
a close one."* So the engine names every set of keys inside its window instead of
declining on 70 % of them, and the fit is what keeps that honest.

**One direction of the invariant did not relax and must not:** a sounding key is
never *silently* ignored. It is claimed by the reading, or counted in `plus`, or the
reading admits with `≈` that it is only the nearest thing. There is no fourth
option, and it is swept over all 4,095 pitch-class sets.

Readings are ordered by fit **before any score term** — the score is a sum of
guessed weights, the fit is a measurement. One deliberate exception: an omission the
grammar already sanctioned (see *Omissible tones*) costs nothing, so it still
competes on score. Charging for it moves 311 measured headlines onto worse names.

### Headline `[coined]`

The reading ranked first, shown large. It is literally rank 1 of the same list
shown beneath it, not a separately-computed "best guess".

### Re-rooting vs enharmonic respelling `[coined]`

The two axes of "alternate ways to spell the chord", kept apart because they are
different operations:

- **Re-rooting** — same keys, different root. `C6` and `Am7`.
- **Enharmonic respelling** — same keys, same root, different letters. `C#Δ7` and
  `DbΔ7`.

## Terminal terms

### Skin

The terminal rendering of the same app: same model, same words, same notes file,
drawn in a terminal instead of a window. Michael's word. A skin changes how the app
is drawn and nothing about what it says.

### Fixture `[coined]`

The Swift engine's output — the chord analysis for every one of the 4,095
pitch-class sets, with the bass note where it changes the ranking — committed as
data. The Rust engine must match it row for row. See ADR-0002.

### Demo mode `[coined]`

The app running on a scripted chord with no keyboard attached, chosen by a `--fake`
flag: `--fake`, `--fake-released`, `--fake-fit`, `--fake-nearest`. Each shows one
state of the display so it can be seen without hardware.

### Headline figure `[coined]`

The headline drawn large as a multi-line block figure, one glyph per character of
the notation alphabet. It is the same headline, only drawn in more than one row of
text. See ADR-0003.

### Running head

The metadata bar across the top of the screen: what is plugged in, what is held,
what is sounding. PUSH's `c-running-head`. It reports; it never decorates.

### Plate

A small bordered chip that reports one fact of the app's own state — `INPUT`,
`STATE`, `RELEASED`. PUSH's `p-plate`. A plate always reads something real; a plate
reading a constant is cut.

### Frame `[coined]`

One value that holds everything the screen shows: the sounding set, the key,
the headline, every alternate with its score, origin, root, fit and claimed
pitch classes, the notes, and a slot for annotations. Both skins draw from it,
`--print` prints it, `--json` streams it. So the screen and the pipe cannot
disagree. See ADR-0005.

## The plain skin and what it shows

Words from the v2 spec (#9). They name the second skin and the facts it puts on
screen. The ones marked `[coined]` were coined by an agent, not by Michael.

### Plain skin

The black-and-white skin that shows every field with no decoration. Michael's
words: *"the most basic skin, just white and black, minimal terminal ui."*
Chosen with `--skin plain`. The terminal's own foreground and background, no
colour token, no box drawing, section labels in UPPERCASE, tables aligned with
spaces, the headline as one line of text. Nothing hides behind a density tier.

### Annotation `[coined]`

A fact derived from an analysis after the engine has ranked it: voicing,
inversion, numeral, staff position. An annotation never changes a reading or
its rank.

### History `[coined]`

The settled sounding sets of this session, newest last, with the time between
them.

### Key context `[coined]`

A tonic and a mode the user sets by hand. It relabels readings; it never
re-ranks them.

### Drill `[coined]`

A mode where the app names a target chord and grades what is played with the
fit tiers.

### Arpeggio mode `[coined]`

A mode where notes played one at a time accumulate into one sounding set until
the pedal lifts.

### Pipe mode `[coined]`

`--json`: one line of JSON per settled sounding set on stdout, for other
programs.

### Upper structure `[coined]`

A major or minor triad inside the sounding set whose root is not the headline's
root. Shown beside the headline, not ranked.

### Staff

The current chord drawn as note heads on a treble and a bass staff, spelled as
the headline spells it. Michael's word.

### Voice leading

Between two sounding sets, the assignment of previous notes to current notes
with the smallest total semitone motion, found by exhaustive search. Reported
as total semitones, common tones kept, and the largest single move. Standard
music theory vocabulary, not coined for this app — computed for each history
entry against the one before it.

## The notation

**Ruled live by Michael, 2026-08-10.** A quality symbol always describes the
**triad**; to alter the *seventh*, spell the alteration as a word.

```
<root><triad><seventh><extensions>

triad:      (none) major    m minor    + augmented    - diminished
            sus4 suspended 4th    sus2 suspended 2nd
seventh:    (none) none     7 minor    Δ7 major    dim7 diminished    aug7 augmented
extensions: 6  add9  9  11  13  b5  #5  b9  #9  #11  b13
```

A bare `7` meaning a **minor** seventh is the ordinary dominant convention, and
Michael confirmed it: *"a C7 just assumes a minor 7."*

**A bare `9`, `11` or `13` implies the seventh beneath it.** `add9` is how you say
"a ninth with no seventh" — that is the whole reason both spellings exist. Ruled
live by Michael, 2026-08-10: *"you need to add both, all relatively common, 9, 11,
13, etc need to be avail."* The same rule covers the altered extensions `b9`, `#9`,
`#11`, `b13`.

```
Csus4  C F  G          C9    C E  G  Bb D      Cadd9   C E G D     (no 7th)
Csus2  C D  G          C11   C E  G  Bb D F    C6add9  C E G A D
C7sus4 C F  G  Bb      C13   C E  G  Bb D A    CΔ9     C E G B  D
```

### Omissible tones

A full 13th chord is seven pitch classes and nobody plays it — the hands produce
`C E Bb D A`. So a quality declares which of its tones may be **absent**: the 5th
in any seventh-or-larger chord **but only when that fifth is perfect**, the 11th in
a `13`, the 9th in an `11` or `13`. The root, 3rd and 7th are never omissible —
they are what makes the chord itself.

The perfect-fifth condition is load-bearing, not a detail. Without it `C Eb Bb`
names as `C-7` — half-diminished, with the diminished fifth *that makes it
half-diminished* declared missing. Those keys are `Cm7`. The same reasoning covers
augmented triads: a fifth that has been moved is doing work and cannot be dropped.

The invariant does **not** weaken in the direction that matters: every *sounding*
key must still be explained.

Ticket 20 made these omissions **visible** — `C13 ·no5 ·no11` rather than a bare
`C13` — without making them dearer. They cost nothing in the fit ordering, because
the grammar has already ruled the chord is still that chord without them and the
score already charges `perAlteration` for each one. So a complete reading does not
automatically outrank one that dropped a sanctioned tone; it wins on score, as it
did before, which measurement showed gives the better name. Anything the grammar did
*not* sanction — a dropped root, third or seventh, a key the chord cannot explain —
does lose on fit, ahead of every score term.

| Written   | Reads as                                  |
| --------- | ----------------------------------------- |
| `C#`      | major triad                               |
| `C#m`     | minor triad                               |
| `C#+`     | augmented triad                           |
| `C#-`     | diminished triad                          |
| `C#7`     | major triad + minor 7 (dominant)          |
| `C#Δ7`    | major triad + major 7                     |
| `C#mΔ7`   | minor triad + major 7                     |
| `C#+7`    | augmented triad + minor 7                 |
| `C#-7`    | diminished triad + minor 7 — half-dim     |
| `C#-dim7` | diminished triad + dim 7 — fully dim      |
| `C#6add9` | major triad + 6 + add 9                   |
| `C#7b5`   | major triad + minor 7, flat 5             |

Rendered as one line of plain text: `b` for flat, `#` for sharp, `Δ` for major.
The headline is also drawn as a *headline figure* (see above); the figure changes
the size, never the text.

## Decisions and where they live

Specs and tickets are GitHub issues on `michaelrholloway/watchord`. Architectural
decisions land in `docs/adr/`. Decisions made in note-view before watchord existed
stay in note-view's tracker and ADRs; watchord inherits them and does not restate them.

## Standing constraints

- **The look is PUSH, as a terminal skin.** `docs/PUSH.md` is the standard;
  ADR-0003 lists every terminal departure with its reason. The plain skin sits
  beside it, chosen with `--skin plain`, and does not change PUSH (ADR-0005).
- **The app listens and never sounds.** No MIDI output, no audio.
- **One sounding chord at a time.** Progression and key analysis are out of scope.
