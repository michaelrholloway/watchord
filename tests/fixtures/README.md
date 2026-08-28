# engine-fixture.json

The engine-parity gate for watchord's Rust chord engine (ADR-0002). Generated
by note-view's `fixture-gen` executable target, which runs the real Swift
`NamingEngine` and writes one JSON row per pitch-class set, plus a hand-picked
voiced table.

- **note-view commit:** `1abc761aa7cb1028ab471215d9ff749a9cdb7d6b`
  (`git -C ~/Development/note-view rev-parse HEAD` at generation time; also
  recorded in the file itself as `generatedFrom`).
- **Command:** from a note-view worktree at that commit,
  `swift run fixture-gen > tests/fixtures/engine-fixture.json` (in this repo),
  run from the note-view checkout.
- **Generator source:** `Sources/fixture-gen/main.swift` in note-view. Its
  header comment documents the JSON schema in full; the summary below is a
  pointer, not the contract.

## Regenerating

A change to the Swift engine's grammar, weights, or omissible tones starts in
note-view. Once it lands there, from a note-view checkout on that commit:

```
swift run fixture-gen > /path/to/watchord/tests/fixtures/engine-fixture.json
```

Then commit the new fixture in watchord and update the commit sha above and
`generatedFrom` inside the file (the generator fills `generatedFrom`
automatically from `git rev-parse HEAD`, run in the note-view working
directory).

## Shape

```
{
  "generatedFrom": "<note-view git sha>",
  "schema": 1,
  "sets": [ <row>, ... ],    // all 4,095 non-empty pitch-class sets
  "voiced": [ <row>, ... ]   // >=60 hand-picked bass/inversion/voicing cases
}
```

Each `<row>` carries `pitchClasses`, `midiNotes`, an optional `name` (voiced
rows only), a `headline`, and `alternates` in rank order. `headline` and each
alternate carry `text`/`spoken`/`fitTier`/`fitNote`/`approximation` (and for
alternates, `axis`) — the exact strings and fit the Swift app shows — plus
`root`, `score`, and `claims` for a stronger tie-break comparison, and
`missingDegrees`/`extraPitchClasses`/`distance` where the fit tier carries
one. A declined headline carries `declineReason` (the engine's raw enum
value: `singleNote`, `tooManyPitchClasses`, `noHonestReading`, or `silent`)
instead of a `fitTier`.

`sets` is voiced in one octave from C4 (MIDI 60) upward, ascending pitch
class — so the lowest pitch class in each set is always its bass. `voiced`
exercises the same pitch-class sets under different bass notes: rotations
through every quality tone as bass for 15 chord qualities, plus doubled
notes, a drop voicing, an off-chord slash bass, and three Bill Evans-style
rootless shell voicings (3-5-7-9 and 3-7-9-13), so the slash bass and
re-rooting are proven — not just the pitch-class set.
