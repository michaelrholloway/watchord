# watchord

A terminal app that watches MIDI and names the chord being played, in Michael's
notation, with alternate readings and attachable text notes. It is the terminal
skin of note-view: same model, same words, same notes file. Rust; macOS, Linux
and Windows.

Sessions launch from **cube** (`~/Development/cube`). This repo is a target repo:
it holds the app, its tracker, and its domain docs — never skills or steering.

## Where things live

| What                | Where                                                        |
| ------------------- | ------------------------------------------------------------ |
| Glossary / domain   | `CONTEXT.md`                                                  |
| Architecture record | `docs/adr/`                                                   |
| Tracker             | GitHub issues via `gh --repo michaelrholloway/watchord` — see `docs/agents/issue-tracker.md` |
| Notation truth      | `chord-naming-conventions.md` (Michael's; note-view's copy is the source) |
| The look            | `docs/PUSH.md` — a **vendored** standard; terminal departures in ADR-0003 |
| The engine's truth  | the fixture from note-view's Swift engine — ADR-0002          |

## The look is not ours

The app is drawn in **PUSH**, one of three themes in `rnrh-ui`, carried here as a
verbatim copy by way of note-view with its source commit named (`rnrh-ui@2af4dc7`).
Never edit it locally; a change to how PUSH *works* belongs upstream. What a
terminal cannot draw is written down in `docs/adr/0003-push-in-a-terminal.md`.

Four rules follow from it, and all four are load-bearing:

- **Never write a rule or a box directly.** Apply a structural role and let the
  skin decide what it draws. PUSH resolves a row to a leading bar; another skin
  would resolve the same call to a rule beneath or a box around.
- **Colour and case carry metadata; ink and normal case carry content.** Labels,
  column heads, timestamps and statuses are muted and UPPERCASE. Body copy never is.
- **Reach for the ported element before writing a new one.** Port from PUSH under
  its upstream name at the moment you have a call site for it, never before, and
  do not coin a name one of them already owns.
- **Nothing on screen may look like a readout without being one, and every
  element must have a call site.** If a device would have to invent its content,
  cut it. If it can carry something real — what is plugged in, what is sounding,
  what is held — connect it to that.

## Build

```sh
cargo build              # debug
cargo test               # all suites, including the engine fixture
cargo run -- --fake      # demo mode: a scripted chord, no keyboard needed
```

Other demo flags: `--fake-released`, `--fake-fit`, `--fake-nearest`.

## Conventions

- **The engine never asserts a spelling it has not verified.** A reading is
  emitted only when its pitch classes, computed forward from the grammar, are
  compared with the sounding keys and given a fit.
- **Identity is the pitch-class set, never the name.** The name is a ranked
  opinion. Keying anything persistent on it orphans data the first time a weight
  changes.
- **The Swift engine is the source of truth.** Engine behaviour changes in
  note-view first; the fixture is regenerated; watchord follows. ADR-0002.
- **The notes file is shared on macOS.** Its schema is a contract with note-view.
  ADR-0004.
