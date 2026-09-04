//! Ported from note-view `Sources/NoteViewCore/ChordKey.swift`.

use std::collections::BTreeSet;

use crate::PitchClass;

/// The identity a saved text note attaches to: a chord's **pitch-class set**,
/// canonicalised.
///
/// Deliberately voicing-blind, octave-blind, inversion-blind and doubling-blind —
/// play the same chord an octave up in second inversion and it is the same
/// `ChordKey`, so the notes written against it come back.
///
/// Note what identity is **not**: it is not the displayed name. The name is
/// produced by a ranking heuristic that we fully expect to tune, and keying notes
/// on it would silently re-key or orphan every saved note the first time a weight
/// changed. The pitch classes are a fact; the name is an opinion.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ChordKey {
    /// Ascending pitch-class values joined by `.` — e.g. `"0.4.7.9"` for C E G A.
    raw: String,
}

impl ChordKey {
    /// Canonicalises any pitch classes: distinct, ascending, dot-joined.
    pub fn new(pitch_classes: impl IntoIterator<Item = PitchClass>) -> Self {
        let set: BTreeSet<PitchClass> = pitch_classes.into_iter().collect();
        ChordKey {
            raw: set
                .iter()
                .map(|pc| pc.value().to_string())
                .collect::<Vec<_>>()
                .join("."),
        }
    }

    /// Round-trips `raw`. Returns `None` on anything that is not *exactly* the
    /// canonical form, so a hand-edited or corrupted store surfaces as a skipped
    /// note rather than as a key no played chord will ever equal.
    ///
    /// The check is deliberately "re-canonicalise and compare", not a list of
    /// individual rules. Comparing against the canonical rendering cannot miss a
    /// case, because it *is* the definition.
    ///
    /// The empty string is rejected here on purpose, and this is load-bearing
    /// beyond this type: the store (`watchord-store`) reuses this exact method
    /// to refuse ever attaching a note to the key of silence
    /// (`if ChordKey::parse(key.raw()).is_none() { refuse }`), and it also
    /// treats a stored note whose `chordKey` decoded to `""` as damaged and
    /// skips it — a legitimately-written note is never keyed to silence, so an
    /// empty key on disk can only be corruption. Loosening this here would
    /// silently defeat both checks at once. A caller that legitimately needs
    /// `""` to mean "nothing displayed" (an idle `Frame`'s `key`) special-cases
    /// it at its own field, with its own `deserialize_with`, rather than here.
    pub fn parse(raw: &str) -> Option<Self> {
        if raw.is_empty() {
            return None;
        }
        let mut seen = BTreeSet::new();
        let mut values: Vec<u8> = Vec::new();
        for part in raw.split('.') {
            let n: u8 = part.parse().ok()?;
            if n >= 12 || !seen.insert(n) {
                return None;
            }
            values.push(n);
        }
        if !values.is_sorted() {
            return None;
        }
        let rendered = values
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(".");
        if rendered != raw {
            return None;
        }
        Some(ChordKey {
            raw: raw.to_string(),
        })
    }

    /// The canonical string, `"0.4.7.9"`.
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// The pitch classes this key names, ascending.
    pub fn pitch_classes(&self) -> BTreeSet<PitchClass> {
        self.raw
            .split('.')
            .filter_map(|p| p.parse::<i32>().ok())
            .map(PitchClass::new)
            .collect()
    }

    /// True for the key of silence.
    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }

    /// The key of silence. Never has notes attached.
    pub fn empty() -> Self {
        ChordKey::default()
    }
}

impl std::fmt::Display for ChordKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.raw)
    }
}

/// Encoded as a bare string — `"0.4.7.9"`, not `{"raw":"0.4.7.9"}`.
///
/// Decoding goes through the validating parser, so a hand-edited or corrupted
/// store fails loudly instead of decoding a non-canonical key that no played
/// chord can ever equal.
#[cfg(feature = "serde")]
impl serde::Serialize for ChordKey {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.raw)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ChordKey {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        ChordKey::parse(&raw).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "'{raw}' is not a canonical chord key (ascending, distinct, 0-11, dot-separated)"
            ))
        })
    }
}

/// A `deserialize_with` for exactly one field: `Frame::key`, whose `""` is a
/// real, legitimate value — nothing is displayed — and not the corruption
/// `ChordKey`'s own `Deserialize` (correctly) treats it as everywhere else
/// (see `ChordKey::parse`'s doc comment). Every non-empty string still goes
/// through the same validating parser.
#[cfg(feature = "serde")]
pub fn deserialize_key_or_empty<'de, D>(d: D) -> Result<ChordKey, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = <String as serde::Deserialize>::deserialize(d)?;
    if raw.is_empty() {
        return Ok(ChordKey::empty());
    }
    ChordKey::parse(&raw).ok_or_else(|| {
        serde::de::Error::custom(format!(
            "'{raw}' is not a canonical chord key (ascending, distinct, 0-11, dot-separated)"
        ))
    })
}
