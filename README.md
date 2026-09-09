# watchord

A terminal app that watches MIDI input and names the chord you are playing, in Michael's notation, with every honest alternate reading beneath it and text notes that resurface when you play the chord again. Rust; macOS, Linux and Windows.

![screenshot: coming with the first release](docs/screenshot.png)

From v0.1.0:

- `brew install michaelrholloway/tap/watchord`
- `npx watchord`
- `cargo install watchord`

`watchord` opens the packed skin, the 80×24 design. `--skin push` and
`--skin plain` draw the two older skins; `--fake-rich` plays a scripted session
with no hardware.

[Every field the plain skin shows, with examples.](docs/fields.md)
