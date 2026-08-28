<!-- Vendored from rnrh-ui@2af4dc7 by way of note-view@1abc761aa7cb1028ab471215d9ff749a9cdb7d6b (docs/PUSH.md, copied verbatim).
     Terminal departures live in docs/adr/0003-push-in-a-terminal.md, not here. -->

# PUSH, in note-view

The visual register this app is drawn in. **PUSH is not from here** — it is one of
three themes in Michael's component library `rnrh-ui`, and this document is the
record of it being carried across, not a design of its own.

> **Vendored standard.** Source: `michaelrholloway/rnrh-ui` at commit **`2af4dc7`**,
> the `:root[data-theme='push']` block of `common/ui/globals.css` (lines 622–777)
> and `docs/DIALECT.md`. It is carried as a **copy with its source named**, never
> paraphrased and never edited locally. A change to how PUSH *works* belongs
> upstream in rnrh-ui; editing it here forks it silently. Where this app must
> depart, the departure is written down below with its reason.

<!-- proxy: decided for Michael 2026-08-15 · precedent: "a vendored standard he told
     you to conform to — carried into the repo as a copy with its source commit named,
     never absorbed or paraphrased" (excelas-marketing-web, docs/dms-ui/) -->

---

## 1. What PUSH is

The **display register** of a library whose three themes differ by *genotype*
rather than by palette. BROADSHEET is a printed page, PLOTTER is an instrument
panel, and PUSH is a thing that **emits**. Michael named the word `push` himself,
before any of it existed, and the whole register follows from taking that
seriously: black ground, one acid hue used as a **field** rather than a mark,
bright rules because a black ground gives nothing back, and glow that is a
property of the light rather than a fiction about distance.

The genotype is carried by structural tokens that are multi-value CSS properties:

| theme      | a row is…                    | `--stroke-row` |
| ---------- | ---------------------------- | -------------- |
| BROADSHEET | a **rule**, separated below  | `0 0 1px 0`    |
| PLOTTER    | a **box**, enclosed          | `1px`          |
| **PUSH**   | a **bar**, marked at the start | `0 0 0 2px`  |

Upstream these feed CSS `border-width`, so `0 0 0 2px` is a **2px leading edge and
nothing else**. That is why `Style.Stroke` in this app holds four widths rather
than a Bool per side, and why views call `.pushRow()` rather than drawing an edge:
the genotype axis survives the port as a seam, so a second theme here would be a
different set of widths and nothing more.

**PUSH takes two measured departures from DIALECT, and they are the source's, not
this app's.** Radius is not zero (a plate on a display is a rounded rectangle),
and rules are bright. Its glow arrived upstream as its own tokens set to `none` on
both print themes — so DIALECT's no-shadow rule survives intact in its original
scope rather than being softened to admit the new case.

## 2. Where it lives here

| file | what it holds |
| ---- | ------------- |
| `Sources/NoteViewApp/Style.swift` | every token, named as PUSH names it |
| `Sources/NoteViewApp/Push/PushUtilities.swift` | the `u-*` utilities — strokes, emission, type |
| `Sources/NoteViewApp/Push/PushPrimitives.swift` | `p-*` — surface, plate, button, rule, scan |
| `Sources/NoteViewApp/Push/PushTable.swift` | `p-table`, `p-definition-list` |
| `Sources/NoteViewApp/Push/PushComponents.swift` | `p-tabs`, `c-running-head`, `c-section-head` |
| `Sources/NoteViewApp/Resources/Fonts/` | the two faces, bundled |

### What came across

Ten elements, each keeping its upstream name, its variant axes and its
`<name>Defaults`. **Every one of them has a call site**, which is the rule §3.9
now states: an element carried without one is cut.

| here | upstream | doing what in this app |
| ---- | -------- | ---------------------- |
| `PSurface` | `p-surface` | the display panel, the particulars panel |
| `PPlate` | `p-plate` | `INPUT/<device>`, `STATE/HELD`, `RELEASED`, the error chip |
| `PButton` | `p-button` | every `delete` |
| `PTable` | `p-table` | alternate readings, notes on a chord, **all notes** |
| `PDefinitionList` | `p-definition-list` | the keys cluster |
| `PTabs` | `p-tabs` | the two screens |
| `CRunningHead` | `c-running-head` | the metadata bar at the top |
| `CSectionHead` | `c-section-head` | the two section openers |
| `PRule` | `p-rule` | every rule, incl. the thick-over-thin display pair |
| `PScan` | `p-scan` | says the display panel is **live** |

**`PScan` is the one worth naming.** Its whole job is saying a panel is live
rather than a still, and a chord displayer is exactly the case that ambiguity
bites: a frozen instrument and a working one look identical. So it runs on the
display panel and only while a chord is actually sounding — it stops on release,
which makes the sweep a report rather than an ornament. Its own rule is **one per
screen**, and it has one.

Token names are **PUSH's names, not Swift names invented for them** —
`Style.accentBackground` is `--color-accent-background`. That makes the port
checkable line by line against the CSS, which is the point of carrying a standard
as a copy rather than as an interpretation.

The six structural roles port one-to-one:

| DIALECT utility | here | PUSH resolves it to |
| --------------- | ---- | ------------------- |
| `u-row` | `.pushRow()` | 2px leading edge |
| `u-cell` | `.pushCell()` | 1px box, `--radius-sm` |
| `u-panel` | `.pushPanel()` | 1px box, `--radius-md` |
| `u-frame` | `.pushFrame()` | 1px box, `--radius-lg` |
| `u-field` | `.pushField()` | 2px leading edge |
| `u-control` | `.pushControl()` | 1px box, `--radius-sm` |
| `u-meta` | `.meta()` | mono, uppercase, tabular, letterspaced |
| `u-glow` / `u-glow-hard` | `.glowSoft()` / `.glowHard()` | bloom, and bloom with a rim |

## 3. Departures, each with its reason

Eleven. None of them changes a token's value; most are about the platform the
token is landing on. §9 and §10 are the exceptions, and they are about this app
rather than about SwiftUI: they are what a review of what the chrome was actually
*reporting* left behind.

1. **Two weights, not the full ramp.** rnrh-ui's web build carries Unica from
   UltraLight to Black. This app bundles **Regular and Bold**. DIALECT §3 says
   hierarchy comes from case, family and position rather than from size, and that
   a component should use two or three steps — this app uses two weights, and
   bundling nine would be ~1.1 MB of binary nothing references. Additive to
   reverse: add the file, add the case in `Typeface.name`.

2. **One theme, not three.** note-view ships PUSH only, because PUSH is what was
   asked for. The *seam* is kept (structural modifiers, four-sided `Edges`), so a
   second theme is an additive change rather than a rewrite of every call site.

3. **Glow is approximated.** CSS `box-shadow` carries a spread radius; SwiftUI's
   `.shadow` does not. `--glow-soft: 0 0 12px -2px` becomes `.shadow(radius: 6)`,
   and `--glow-hard`'s 1px rim is drawn as a real border because a shadow cannot
   express one. Stated as an approximation rather than filed as an equivalence.

4. **Tracking is resolved to points at the token.** SwiftUI's `.tracking()` takes
   points where CSS `letter-spacing` takes `em`. Each `Style.Step` therefore
   carries its `em` value already resolved against **its own size** — which is
   what `em` means, and which is what stops the whole document tracking wrong from
   one declaration. (The web build learned this the hard way.)

5. **Some tokens are not ported, and the list is deliberate.** The
   recreation-scoped exactness tokens — `--color-itp-room-*`, `--color-sdw-*`,
   `--halo` — exist upstream to hold one reference recreation each and have no
   referent in a chord displayer. The four `--color-signal-N-foreground` values
   are collapsed into one `Style.signalForeground`, because all four are `#000000`
   *by design*: PUSH's signals are chosen so type sits **in** the light rather
   than on it. `--color-sunken-*` and the `--color-danger-background` pair are
   simply not used by this app yet.

   **Every colour token this port does claim to carry was diffed against the
   source block by script** — 21 of 21 exact, with a control proving the
   comparison could fail. That check is what found `Mark` reaching for the accent
   tokens when PUSH also declares `--color-mark-*`: identical hex here, different
   hues on BROADSHEET, so the seam was decorative at that one point until it was
   fixed. Distinct roles that happen to agree in one theme are still distinct.

6. **The segmented `Picker` is gone**, replaced by tabs drawn from PUSH's own
   devices. A `.segmented` picker renders in the system's chrome — its own
   capsule, radius, tint and pressed state — and on a black emissive ground it
   reads as a piece of a different program. Likewise `.focusEffectDisabled()` on
   the tabs, with PUSH's `--color-ring` drawn in place of AppKit's blue capsule.
   <!-- proxy: decided for Michael 2026-08-15 · precedent: "native browser controls
        — dislikes them and would rather they were replaced than have their error
        messages improved" (cube-server #12) -->

7. **`── released ──` became a `PPlate`.** The string was drawn with U+2500 box
   rules, which Neue Haas Unica does not carry — in the sans face the two rules
   fell back to a system font mid-string and sat at a different weight from the
   word between them (verified against the bundled files, not assumed). It is the
   instrument reporting its own state, which is exactly what a plate is for, so it
   is now a `signal-4` plate reading `RELEASED` and the glyph problem goes away
   with the glyphs. The same fact also appears on the head band's `STATE` plate.

8. **The window has a minimum width; upstream has a scroll container.** `PTable`'s
   own docs call the horizontal scroll container "not optional — a dense table is
   wider than a phone, and the page body must never scroll horizontally." Measured
   here at a 520pt window, the All Notes table's flexible `note` column — the
   actual content — collapsed to **zero** while every column of metadata around it
   survived, which is the worst possible way for a table to fail. A web page must
   survive a 390px phone and has to scroll; a window can simply decline to be made
   too small, so the minimum went 520 → **700** and every flexible column carries a
   `minWidth` floor as well. Verified by asking the window for 200×400 and getting
   700×492 back.

9. **Nothing is carried without a call site.** Five elements were —  `PBadge`,
   `PMark`, `PRegistration`, `PBarcode`, `CAlarm` — under a note saying to cut them
   if a second reader still found them unused. A second reader did, so they are
   gone. `PBarcode` went with its two call sites; the other four never had any.

   The argument for keeping them was DIALECT §7 — "copy these, do not invent
   alongside them" — so that the next person needing a chip or a printer's mark
   reaches for the real one. That argument is intact and does not require the code
   to be sitting here: `rnrh-ui@2af4dc7` is named at the top of this file, and
   re-porting one is a copy. What the code sitting here *does* do is present as a
   library this app uses, which it was not.

10. **No element may look like a readout without being one.** The rule this pass
    was written to enforce, and the one that cost the most.

    Two `PBarcode` strips ran along the head band, seeded on the string literals
    `"note-view-head"` and `"note-view-head-2"`. The hash is pure over its seed, so
    the pattern was fixed at build time and identical in every state the app can
    reach — playing, idle, released, erroring — and `accessibilityHidden`, so it
    announced nothing either. Its own doc line said it "reads as *encoded data*
    rather than as ornament", which is the whole problem stated out loud: it read
    as data and was a picture.

    The sharper case was not ornament at all. The head band's **INPUT** plate read
    `MIDI` off `banner == nil` — a build flag. `start()` succeeds with no hardware
    attached and the stream simply stays quiet (`AppModel.start()` says so), so a
    keyboard that was never plugged in and a keyboard sitting idle produced an
    identical window: `INPUT / MIDI`, `STATE / IDLE`, no error. A status light
    wired to a constant is worse than no status light, because a reader cannot tell
    which of the console's marks to trust and ends up trusting none.

    It now reports the attached device by name, live, through a new seam —
    `SoundingSetSource.connectedInputs`, an `AsyncStream<[String]>` published from
    the same `connectToAllSources` that already re-enumerates on every hot-plug.
    The machinery to notice a device appearing after launch was already there and
    load-bearing (`CoreMIDISource.start()` carries a runloop warning defending it);
    it had nowhere to report to. Now it does.

11. **`PReadout` is deliberately not ported.** Upstream it is a fixed-width,
    zero-padded, explicitly signed *numeric* figure — an instrument printing a
    quantity. This app has no such quantity, and an earlier draft of this port had
    invented a type called `Readout` for a label/value pair, which is a different
    object with one of his names on it. That is `PDefinitionList`, and it is now
    what the keys row uses.

## 4. What this pass was NOT allowed to change

Michael named two wastes at the launch of the run that produced this file, and
they are the bar the work is held to:

- **What the app says.** Every string is the same string, in the same place,
  saying the same thing. Chord names, notation, the `≈` and `·no5` placement
  rules from ticket 23, the wording of every label and error — untouched. The
  restyle changed the face, the ink and the furniture, and nothing else.
- **The chord's legibility from the piano.** The headline went **up**, from 64pt
  system to PUSH's `mega` step at 80pt, and it is the one element on the screen
  that takes no console treatment at all — no plate, no mono, no acid field
  behind it, because each of those costs contrast or costs size.

**Measured, not eyeballed:** all 13 colour pairs the app draws were enumerated and
scored, with the probe self-tested on white/black (must be 21.00) and white/white
(must be 1.00) before any number was believed. Every pair passes; the headline
sits at **17.73:1**, and the tightest pair in the app is the hairline against the
ground at 4.47:1 against a 3:1 non-text floor.

## 5. One thing the restyle earned

`AppModel` has always distinguished `banner` (*"a standing line about the graph
itself"* — `fake input — no MIDI hardware is being read`) from `statusMessage`
(*"something outside the model's control went wrong"*). The old style drew both in
the same grey, and the distinction was invisible. PUSH has the palette to tell
them apart, so it does: a notice takes the muted ink and a neutral bar, a failure
takes the danger ink and a danger bar.

This is a change in what the screen *shows*, not in what it *says*, and it is the
only one.

<!-- proxy: decided for Michael 2026-08-15 · no precedent. The first draft drew both
     in the danger ink, which asserted a fault where there was none; it was
     indefensible on sight and was found by rendering it rather than by reasoning
     about it. -->
