---
status: accepted
date: 2026-08-28
---

# Engine parity by fixture

The chord engine is a port of note-view's Swift engine, and Michael's rule is *"no
missing functionality at all."* The gate for the port is a committed **fixture**:
the Swift engine's chord analysis — the full ranked list of readings, each with its
fit — for all 4,095 pitch-class sets. The Rust engine's test loads the fixture and
must match every row.

The fixture must cover the **bass note**, not only the pitch-class set. The ranking
runs on the sounding set's raw notes, and the bass note is the strongest single
term in it, so one pitch-class set can headline differently under different bass
notes. A fixture keyed on pitch classes alone would pass an engine that gets the
bass wrong. For each set the fixture holds one row per distinct bass pitch class
that changes the result, and at least the lowest-note case.

## Considered options

- **Port the Swift test suite by hand.** Rejected: it proves only what the tests
  happened to cover, and the invariant that matters — no sounding key is silently
  ignored — is swept over all 4,095 sets in Swift, not over a handful of examples.
- **Trust review.** Rejected: two engines that read the same on paper can still
  rank differently on a tie, and a ranking difference is invisible to a reader.

## Consequences

- The fixture generator is a small executable committed in note-view. It runs the
  Swift engine and writes the fixture; watchord commits the output.
- Regenerating the fixture is the only way to change engine behaviour in watchord.
  A change to a weight, an omissible tone, or the grammar is made in Swift first,
  the fixture is regenerated, and the Rust engine is brought back to green. The
  Swift engine stays the source of truth until Michael says otherwise.
- The fixture is data, not code, so its format is a contract between the two repos
  and is written down beside the generator.
