---
status: accepted
date: 2026-08-28
---

# PUSH in a terminal

The skin is **PUSH**, the display register vendored at `docs/PUSH.md` from
`rnrh-ui@2af4dc7` by way of note-view. Michael's words: *"the goal is not to port
over the rnrh-ui exactly, but basically convert it into a terminal skin."* So the
standard stays a verbatim copy, and every departure a terminal forces is listed
here, with its reason, so nobody edits the standard to fit the medium.

## Departures

(a) **The mono/sans rule becomes colour + case.** A terminal has one face, so
"mono carries metadata, sans carries content" cannot be drawn. Metadata — labels,
column heads, timestamps, statuses — is drawn in `mutedForeground` and UPPERCASE.
Content — the headline, readings, note text — is drawn in `foreground` and normal
case. Same two registers, two different signals.

(b) **Structural roles resolve to box-drawing.** `row` and `field` resolve to a
2-column leading bar (`▌` or `┃`), the terminal's reading of PUSH's 2px leading
edge. `cell`, `panel`, `frame` and `control` resolve to a 1-character box drawn
with light lines. Radius does not exist in a terminal; the three radius tokens
resolve to nothing.

(c) **Colour is the PUSH token hex values as 24-bit truecolor**, name for name. A
terminal without truecolor gets a 256-colour fallback chosen once per token, never
per call site.

(d) **Glow does not exist in a terminal, and we do not fake it.** No bright
halo characters, no doubled rules. A rule that stands in for glow is a mark that
reports nothing, which §10 of the standard forbids.

(e) **The headline is a block figure.** The headline is drawn large as a
multi-line figure from a hand-made glyph set that covers the notation alphabet:
`A`–`G`, `#`, `b`, digits, `Δ`, `m`, `+`, `-`, `sus`, `add`, `/`, `≈`, `·`, `no`.
The figure is the same text, only taller; every glyph in the alphabet has one
figure and nothing outside the alphabet is ever drawn large.

(f) **The kitty graphics protocol is a later, optional enhancement.** Where a
terminal supports it, the headline may be drawn as an image. It is not core: the
block figure is the headline on every terminal, and the image is an extra.

## Rules carried from note-view

These four rules from note-view's `CLAUDE.md` bind here unchanged:

- **Never write a rule or a box at a call site.** Apply a structural role and let
  the skin decide what it draws.
- **Reach for the ported element before writing a new one.** Copy from PUSH under
  its upstream name at the moment there is a call site for it, never before.
- **Nothing on screen may look like a readout without being one.** A mark that
  reads as data must be data.
- **Every element has a call site.** An element carried without one is cut.
