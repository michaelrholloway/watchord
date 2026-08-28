---
status: accepted
date: 2026-08-28
---

# Rust for a cross-platform terminal app

watchord is the terminal skin of note-view, and Michael wants it open source, on
his website, for users to install on their own PC with a tool they already have.
We write it in Rust: ratatui + crossterm for the terminal, midir for MIDI, one
static binary, macOS, Linux and Windows from the first release.

Michael's stated reason: *"os is the goal, i want to put it on my website for users
to install if they want on their pc ... rust, open source is real."* A single
binary with no runtime is what makes `brew`, `npx` and `cargo install` all deliver
the same thing.

## Considered options

- **Swift.** Zero port of the engine, and the Mac app already exists. Rejected:
  MIDI input is CoreMIDI, which is Apple-only, so Linux and Windows would need a
  second MIDI layer anyway; and a Swift toolchain is a hurdle for open-source
  terminal users on the other two platforms.
- **Go with bubbletea.** Good terminal story, single binary. Rejected: the MIDI
  library situation is thinner than midir, and Michael named Rust.
- **TypeScript with ink.** Fastest to write, `npx` is natural. Rejected: it needs
  Node on the user's machine, so `brew` and `cargo install` would ship a runtime,
  not an app.

## Consequences

- The engine is a port, not a share. Parity with the Swift engine is proven by
  fixture (ADR-0002), and the Swift engine stays the source of truth.
- Three platforms on day one means every ticket that touches the terminal or MIDI
  names all three, and CI builds all three.
- Distribution is cargo-dist to GitHub Releases, then a Homebrew tap, an npm
  wrapper, and `cargo install`. cargo-dist is not yet installed on this Mac.
