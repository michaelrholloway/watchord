<!-- Copied verbatim from michaelrholloway/note-view (chord-naming-conventions.md). note-view's copy is the source; edit it there, then re-copy. -->

C# - c sharp major 

C#m - c sharp minor 

C#7 = c sharp major, minor 7

C#mΔ7 - c sharp minor, major 7

C#6add9 - c sharp major, major 6. add 9

C#7b5 - c sharp major, minor 7, flat 5

C#+7 - c sharp major, aug 7

C#-7 - c sharp major, dim 7

C#+ - c sharp aug

C#- - c sharp dim

---

## Amendment, 2026-08-10 — the rule behind the examples

Added by an agent while building the app. Michael's ten examples above are the
source of truth, but they never state the rule that *generates* a name, and an
engine has to generate names for chords not in the list. Asked directly, Michael
ruled:

> "lets do this, and you can use aug or dim to alter the 7th, and the alt
> spellings will show up. C#-dim7, c sharp dim, dim 7."

**A quality symbol always describes the TRIAD. To alter the seventh, spell it out.**

```
<root><triad><seventh><extensions>

triad:       (none) major     m minor     + augmented     - diminished
seventh:     (none) none      7 minor     Δ7 major     dim7 diminished     aug7 augmented
extensions:  6  add9  9  11  13  b5  #5  b9  #9  #11  b13
```

A bare `7` meaning a *minor* seventh is the ordinary dominant convention, confirmed
in the same exchange: *"a C7 just assumes a minor 7."*

### ⚠️ Two lines above are superseded

The glosses for `C#+7` and `C#-7` read the *other* way — triad stays major, only
the seventh moves. Michael's ruling overrides them. Kept visible rather than edited
away, so the change is legible:

| Written   | Said above (superseded)   | Now reads as                          | Pitches      |
| --------- | ------------------------- | ------------------------------------- | ------------ |
| `C#+7`    | c sharp major, aug 7      | **aug** triad + minor 7               | C# E# G## B  |
| `C#-7`    | c sharp major, dim 7      | **dim** triad + minor 7 (half-dim)    | C# E  G   B  |
| `C#-dim7` | *(not listed)*            | dim triad + dim 7 (fully diminished)  | C# E  G   Bb |

### Why the app can never just read your spelling

MIDI transmits key numbers, not letter names — it is **spelling-blind**. `C#-dim7`
and `E-dim7` and `G-dim7` are the same four keys. `C#6` and `A#m7` are the same
four keys. So the app cannot know which you meant; it generates every spelling the
keys honestly admit, ranks them, and shows the lot. That ambiguity isn't a defect
being papered over — surfacing it *is* the alternate-spellings feature.
