//! watchord-theory: every annotation derived after the engine has ranked a
//! reading — voicing, inversion, numeral, staff, voice leading. Nothing here
//! touches the engine or the ranking (ADR-0002): each function is pure, over
//! the sounding set and, where the fact needs it, the analysis, an optional
//! key context, or the previous sounding set.
//!
//! Kept to one `pub mod` line per function group so two tickets landing on
//! this crate at once collide in a module file, never in this one.

pub mod key_context;
pub mod voice_leading;
