//! watchord-theory: every annotation derived after the engine has already
//! ranked its readings — voicing, inversion, slash, upper structure, staff,
//! voice leading.
//!
//! Pure functions over a [`watchord_core::ChordAnalysis`] and the sounding set.
//! Nothing here touches the engine or the ranking (ADR-0002): an annotation
//! never changes a reading or its rank. Kept to `pub mod` lines only, so two
//! parts adding modules here in parallel merge without touching each other's
//! lines — see `crates/watchord-theory` in the run's COMMON.md addendum.

pub mod annotate;
pub mod inversion;
pub mod spelling;
pub mod staff;
pub mod upper_structure;
pub mod voice_leading;
pub mod voicing;
