# The plain skin's fields

The plain skin uses the terminal's own foreground and background: no colour,
no box drawing. It shows every field of the
[`Frame`](../crates/watchord-model/src/frame.rs), label in UPPERCASE, value
beside it. Nothing hides behind a density tier. A short terminal draws what
fits, top down, and keeps the note field and the key line at the foot.

Run it with `cargo run -- --skin plain`. The `--print` flag prints the same
fields as headless labelled lines, instead of drawing a screen. The `--json`
flag streams them as one JSON line per settled sounding set. Five flags play
a scripted session with no MIDI hardware attached: `--fake`, `--fake-released`,
`--fake-fit`, `--fake-nearest`, and `--fake-rich` — a nine-chord progression
that leaves every region on this page populated at once, for a design
screenshot. Each shows every field below with no keyboard needed.

This reference follows the order the plain screen draws. First the head.
Then the display, the readings table, the annotations, the staff, the notes,
and the foot key line.

Every example below is real output. Each names the command that made it. A
field the fakes never fill — a pedal held down, a filter set, a second
history entry, a key context — carries the mark **shape only**. That entry
quotes the line the code would draw instead, straight from its source.

## The head

The running head reports the device plugged in, the chord held, and the
notes sounding. It never decorates.

### SCREEN

Which of the two screens is showing: Now Playing or All Notes. Brackets mark
the current one. Print writes the screen's title, not the label.

Plain: `SCREEN         [Now Playing]    All Notes`
Print: `screen: Now Playing`

### BANNER

The standing line about the input graph. It shows only for a fake run or a
stubbed naming engine. `—` once the app reads real MIDI hardware, through its
real engine.

Plain: `BANNER         fake input — no MIDI hardware is being read`
Print: `banner: fake input — no MIDI hardware is being read`

### STATUS

A line of plain words when something has gone wrong — a device dropped, a
note failed to save. `—` otherwise.

Plain: `STATUS         —`
Print: `status: —`

### INPUT

What the INPUT plate reads: `fake` in a demo, or the live source's own word.

Plain: `INPUT fake   STATE held   INPUTS —   FILTER —`
Print: `input: fake`

### INPUTS

The MIDI devices currently attached, by name, comma-separated. `—` with no
device attached. Every fake shows this: a fake plays no real device.

Print: `inputs: —`

**Shape only.** With two devices attached: `INPUTS Instrument 1, Instrument 2`.

### STATE

STATE reads `idle` before anyone plays, `held` while a chord sounds, and
`released` once the keys lift, holding the last chord.

Plain: `INPUT fake   STATE held   INPUTS —   FILTER —`
Print: `state: held`

### FILTER

The one input device the app is listening to, when the picker (`i`) has
narrowed it. `—` while every attached input connects.

Plain: `INPUT fake   STATE held   INPUTS —   FILTER —`
Print: `filter: —`

**Shape only.** With the picker narrowed to one device: `FILTER Instrument 1`.

### SUSTAIN

The sustain pedal (MIDI CC64), as the source last reported it: `DOWN` or
`UP` on the plain screen, `down`/`up` in print.

Plain: `SUSTAIN UP   SOSTENUTO UP   SOFT UP   SETTLE 60 ms   ARPEGGIO OFF   KEY CONTEXT —`
Print: `sustain: up`

**Shape only.** Pedal down: `SUSTAIN DOWN`.

### SOSTENUTO

The sostenuto pedal (MIDI CC66), as the source last reported it. Its down
edge toggles arpeggio mode.

Plain: `SOSTENUTO UP`
Print: `sostenuto: up`

**Shape only.** Pedal down: `SOSTENUTO DOWN`.

### SOFT

The soft pedal (MIDI CC67), as the source last reported it. Held down, it
reads the headline's numeral, Nashville number, and function against the
headline itself as a temporary key context.

Plain: `SOFT UP`
Print: `soft: up`

**Shape only.** Pedal down: `SOFT DOWN`.

### SETTLE

The settle window in milliseconds — how long the engine waits for more keys
before it names a chord. Two keys adjust it, 10 ms at a time, from 10 to 500.

Plain: `SETTLE 60 ms`
Print: `settle: 60 ms`

### ARPEGGIO

Arpeggio mode holds notes instead of releasing them. A note-on adds to the
sounding set. A note-off does not remove it, until the sustain pedal lifts
or every key comes up.

Plain: `ARPEGGIO OFF`
Print: `arpeggio: off`

### KEY CONTEXT

The tonic and mode every reading's numeral, Nashville number, and function
read against. Two keys set it by hand. Holding the soft pedal borrows it from
the headline instead. `—` until set: the sounding keys never derive it.

Plain: `KEY CONTEXT —`
Print: `key context: —`

**Shape only.** With a key set: `KEY CONTEXT C# minor`.

## The display

The headline and every fact beside it: the one name the app is committing to,
and the raw keys behind it.

### HEADLINE

The reading ranked first, in Michael's notation. It is rank 1 of the same
list the readings table shows beneath it, not a separately guessed name.

Plain: `HEADLINE       C6`
Print: `headline: C6`
JSON: `"headlineText": "C6"`

### APPROXIMATION

`≈` beside the headline when its fit is `nearest` — the closest name the
grammar has, not an honest match. `—` otherwise.

Plain: `APPROXIMATION  —`
Print: `approximation: —`

**Shape only** (from `--fake-nearest`, where the headline is a nearest fit):
`APPROXIMATION  ≈`.

### SPOKEN

The headline said aloud, in words rather than symbols: `C major 6` for `C6`.

Plain: `SPOKEN         C major 6`
Print: `spoken: C major 6`

### FIT

The `·no5 ·no11` or `·+F#` note under the headline. It names what its fit
tier means for this chord — which tones are missing, or which extra key it
cannot claim. An exact fit shows `—`: there is nothing to name.

Plain (`--fake-fit`): `FIT            ·no5 ·no11`
Print: `fit: ·no5 ·no11`

This is the headline's own detail line. The readings table below carries its
own FIT column, one per reading. That column adds the tier word itself
(`exact`, `missing`, `plus`, `nearest`) beside the same kind of detail — see
*the readings table*.

### DECLINED

Why there is no headline, in plain words, when the keys down do not spell
anything in the grammar. `—` whenever a headline exists.

Plain: `DECLINED       no honest reading — these keys do not spell a chord in the grammar`
Print: `declined: —`

### KEYS

The keys as note names, left to right: `C3  E3  G3  A3`. Empty when nothing
is sounding.

Plain: `KEYS           C3  E3  G3  A3`
Print: `keys: C3  E3  G3  A3`

### KEY

The sounding set's identity: the pitch-class set it names, written `0.4.7.9`
— C=0 through B=11. This is the **chord key**, the identity a saved note
attaches to. It is **not** the musical key: that is KEY CONTEXT, above.

This label will likely become `CHORD KEY` in a later ticket, to say plainly
which of the two it is.

Plain: `KEY            0.4.7.9`
Print: `key: 0.4.7.9`

### SOUNDING

The sounding MIDI note numbers: `60 64 67 69`.

Plain: `SOUNDING       60 64 67 69`
Print: `sounding: 60 64 67 69`

## The readings table

Every reading the engine ranked, headline first, with every fact it ranked
on. One row per reading. The list has no limit, and scrolls past the page.

Plain (`--fake --print` shows the same data as labelled lines; the table
itself only draws on screen):

```
READINGS 3 · HEADLINE 1 · ALTERNATES 2
  RANK NAME          ORIGIN     FIT                 SCORE ROOT CLAIMED               NUMERAL   NASHVILLE FUNCTION
  1    C6            headline   exact               100   C    C E G A               —         —         —
  2    Am7/C         reRooted   exact               90    A    C E G A               —         —         —
  3    CΔ6           enharmonic exact               89    C    C E G A               —         —         —
```

Print writes one line per reading instead of a row:

```
reading: C6 · rank 1 · origin headline · fit exact · score 100 · root C · claimed C E G A · spoken C major 6 · numeral — · nashville — · function —
alternate: Am7/C · rank 2 · origin reRooted · fit exact · score 90 · root A · claimed C E G A · spoken A minor 7 · numeral — · nashville — · function —
```

### READING

Rank 1's row on `--print` — the headline, with every fact beside it. Uses
the same fields as ALTERNATE below; only the label differs.

### ALTERNATE

Every reading below rank 1, best first, one print line each. On the plain
screen these share the same table as the headline's own row.

### ORIGIN

Which axis produced the reading. The `headline` origin is the ranked-first
name. The `reRooted` origin keeps the same keys under a different root —
`C6` and `Am7`. The `enharmonic` origin keeps the same root with different
letters — `C#Δ7` and `DbΔ7`.

### SCORE

The ranking score. Higher wins. FIT orders every reading before any score
term does; score only breaks ties within one tier.

### ROOT

The root this reading claims, as a note name with sharps: `C`.

### CLAIMED

The pitch classes this reading claims — its whole spelling, whether or not a
hand plays every one of them: `C E G A`.

### NUMERAL

This reading's Roman numeral against KEY CONTEXT: `V7`. `—` with no key
context.

**Shape only.** With a key context of `C major` and a reading of `G7`:
`NUMERAL    V7`.

### NASHVILLE

This reading's Nashville number against KEY CONTEXT: `57`, matching NUMERAL's
`V7` above. `—` with no key context.

**Shape only.** Same reading, same key: `NASHVILLE  57`.

### FUNCTION

This reading's role against KEY CONTEXT: `tonic`, `subdominant`, `dominant`,
`borrowed`, `chromatic`, or a secondary dominant such as `V7/IV`. `—` with no
key context. The `borrowed` role means a diatonic root with a non-diatonic
triad quality. The `chromatic` role means a root outside the key.

A dominant seventh on a diatonic root reads as the dominant of the degree a
fourth above it, when that degree is in the key: `C7` in C major is `V7/IV`,
`D7` is `V7/V`, `A7` is `V7/ii`. The NUMERAL stays `I7`. `G7` is the plain
`dominant` in both modes. `F7` in C major resolves outside the key, so it
keeps the triad's role, `subdominant`.

**Shape only.** Same reading: `FUNCTION   dominant`.

## Annotations

What `watchord-theory` derives after the engine ranks the readings: voicing,
inversion, upper structures, and the staff. History and drill share this half
of the screen too. An annotation never changes a reading or its rank.

### ANNOTATIONS

`none yet` until the theory crate has filled in at least one of the fields
below it. Once it has, this line is blank — the fields beneath it carry the
value.

Plain: `ANNOTATIONS    none yet`
Print: `annotations: none yet`

### INVERSION

Which claimed chord tone of the headline is in the bass: `root`, `first`,
`second`, and so on. `—` before the theory crate has run.

Plain: `INVERSION root   SLASH —   VOICING close   SPAN 9   ROOTLESS no   DOUBLINGS —`
Print: `inversion: root`

### SLASH

The slash name, present only when the bass is a claimed tone other than the
root: `C13/E`. `—` when the bass is the root.

Plain (`--fake-fit`): `SLASH C13/E`
Print: `slash: C13/E`

### VOICING

The chord's shape: `close`, `open`, `drop 2`, `drop 3`.

Plain: `VOICING close`
Print: `voicing: close`

### SPAN

The distance in semitones from the lowest sounding note to the highest.

Plain: `SPAN 9`
Print: `span: 9`

### ROOTLESS

Whether the reading claims the root while no hand actually sounds it: `yes`
or `no`.

Plain: `ROOTLESS no`
Print: `rootless: no`

### DOUBLINGS

Which claimed tones sound more than once, and how many times: `A3x2`. `—`
when nothing doubles.

Plain: `DOUBLINGS —`
Print: `doublings: —`

### UPPER STRUCTURE

A major or minor triad inside the sounding set whose root differs from the
headline's root, named against the headline: `Am triad over C6`. Shown,
never ranked. `—` when none exists.

Plain: `UPPER STRUCTURE Am triad over C6`
Print: `upper structure: Am triad over C6`

### HISTORY

The count of settled sounding sets this session, then the strip itself.
Oldest goes first, newest last. Each name carries the seconds since the one
before it.

Plain: `HISTORY 1  C6   VOICE LEADING —`
Print: `history: 1 entries · live`

**Shape only.** With three entries: `HISTORY 3  C6 Am7(12s) F(4s)   VOICE
LEADING …`. Print writes one `history entry:` line per entry:

```
history entry: C6 · — since the one before it
history entry: Am7 · 12s since the one before it
history entry: F · 4s since the one before it
```

### VOICE LEADING

Between the entry on screen and the one before it. It carries the total
semitones moved, how many notes stayed on the same pitch, and the largest
single move. A second entry must exist first, or this reads `—`.

Plain: `VOICE LEADING —`
Print: `voice leading: —`

**Shape only.** Between two entries: `VOICE LEADING 5st 2kept 3max`, and in
print, `voice leading: 5 semitones · 2 common tones kept · 3 largest move`.

### DRILL

Whether drill mode is on, then its target, next target, grade, and stat
count on one line. Drill is a mode drawn on Now Playing, not a separate
screen.

Real output (the drill snapshot test, `crates/watchord-tui/tests/snapshots/plain-drill-120x44.txt`):

```
DRILL          on   TARGET Dm   NEXT TARGET Dm   GRADE missing G   DRILL STATS 1
```

Off, as every `--fake` mode shows it: `DRILL          off   TARGET —   NEXT
TARGET —   GRADE —   DRILL STATS 0`.

### TARGET

The chord drill is asking for right now. `—` while drill is off.

### NEXT TARGET

The chord after the current target. Naming it early lets the hand prepare
before drill grades the current one. `—` while drill is off.

### GRADE

The last attempt's result: `exact`, or a name for what it missed or added —
`missing G`, `extra B`. `—` before anyone plays a chord this session.

### DRILL STAT

Drill keeps one row per chord it has a history for. Each row carries how
many times it was the target, how many graded exact, and when a hand last
tried it.

Real output:

```
  CHORD         ATTEMPTS  EXACT   LAST
  C             1         0       YYYY-MM-DD HH:MM
```

Print writes one line per row: `drill stat: C · attempts 1 · exact 0 · last
2026-08-28 13:05`.

### EDITING

`yes` while the note field is editing an existing note rather than drafting a
new one. `—` otherwise.

Plain: `EDITING        —`
Print: `editing: —`

## The staff

The current chord drawn as note heads on a treble and a bass staff, spelled
the way the headline spells it. The bass staff only draws at 44 rows and up;
under that, only the treble staff shows.

### TREBLE

The treble staff's notes, top to bottom on the plain screen; on `--print`,
one `letter=note` pair per sounding note in the treble range.

Plain (`--fake`, 44 rows):

```
TREBLE
  --

  --

  --
     ●   A3
  -- ●   G3

  -- ●   E3

  -- ●   C3
```

Print: `treble: C=C3 E=E3 G=G3 A=A3`

### BASS

The same, for notes below the treble staff. `—` when nothing sounds there.
Under 44 rows the bass staff never draws at all.

Plain: `BASS    —`
Print: `bass: —`

## Notes

The notes attached to chords: the ones on the chord currently displayed (Now
Playing), and every note ever written, grouped by chord (All Notes).

### NOTE

One line per note on the displayed chord, newest first, with a `delete` mark.

Plain: `try it with the 9 on top`
Print: `note: try it with the 9 on top (written as Am7)`

On All Notes, each row also carries WRITTEN AS and WHEN — see GROUP below.

### NOTES TOTAL

Every note ever written, across every chord, counted.

Plain: `WATCHORD   NOTES TOTAL 3   NOTES 2`
Print: `notes total: 3`

### DRAFT

The text in the note field, before a hand commits it. Its label switches to
`EDIT` while EDITING reads `yes`.

Plain (nothing typed yet): `DRAFT          add a note…`
Print: `draft: —`

### GROUP

All Notes: one heading per chord that has notes, with its chord key, note
count, and tags.

Plain: `GROUP C6   KEY 0.4.7.9   NOTES 2   TAGS —`
Print: `group: C6 · key 0.4.7.9 · notes 2 · tags —`

### WRITTEN AS

The chord's name at the moment a hand wrote the note. All Notes keeps it
beside each note, so a later renaming of the same keys leaves the note's own
history untouched.

Plain column: `WRITTEN AS  Am7`
Print: `(written as Am7)` — part of the `note:` and `group note:` lines.

### EDITING

See EDITING under *Annotations* above — one field, shown on both halves of
the screen it applies to.

### SEARCH

All Notes: the search box's text, applied to every group live as typed. `—`
when empty.

Plain: `SEARCH         —`
Print: `search: —`

### SORT

All Notes: which of three orders sorts the groups. The `recent` order puts
the newest note first. The `chord` order sorts alphabetically by heading. The
`count` order puts the most notes first. The `s` key cycles through them.

Plain: `SORT           recent`
Print: `sort: recent`

### TAGS

All Notes: every `#word` found inside a group's notes, collected and shown on
the group's heading row. `—` when a group has none.

Plain: `GROUP F   KEY 0.5.9   NOTES 1   TAGS —`
Print: `tags —` — part of the `group:` line.

**Shape only.** A note reading `great with #reverb` gives the group
`TAGS #reverb`.

## The foot key line

The one-line key reference at the foot of every screen. It stays under 118
columns so `q quit` never clips. Every key the plain skin binds, in the order
they appear:

| Key | Does |
| --- | --- |
| `tab` | switch screen (Now Playing ↔ All Notes) |
| `↑↓` | select a row |
| `←→` | step through history |
| `d` | delete the selected note |
| `e` | edit the selected note |
| `s` | cycle the All Notes sort order |
| `p` | toggle drill |
| `x` | export as Markdown |
| `^x` (Ctrl-x) | export as JSON |
| `⇧enter` | insert a line break in the note field |
| `enter` | commit the note field, or choose in the input picker |
| `q` | quit |

Two more keys work but do not fit this line: `i` opens the input picker, and
`-`/`+` adjust SETTLE by 10 ms.

Real output: `TAB SCREEN · ↑↓ SELECT · ←→ HISTORY · D DEL · E EDIT · S SORT ·
P DRILL · X MD · ^X JSON · ⇧ENTER NL · ENTER · Q QUIT`
