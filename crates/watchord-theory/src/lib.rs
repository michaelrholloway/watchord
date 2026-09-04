//! watchord-theory: every annotation as a pure function.
//!
//! Its input is the analysis, the sounding set, an optional key context, and
//! an optional previous sounding set (spec #9, "Structure"). Nothing here
//! touches the engine or the ranking — every fact derives from the engine's
//! answer, never from inside it.
//!
//! Each annotation is its own module, added by its own ticket. This file is
//! deliberately just a list of `pub mod` lines so that several tickets adding
//! modules in the same run merge without touching each other's declarations.

pub mod key_context;
