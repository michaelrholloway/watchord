//! watchord-tui: the terminal skin.
//!
//! PUSH drawn with ratatui. [`push`] holds the tokens, the six structural
//! roles and the ported elements — the only module that draws a rule or a
//! box. [`figure`] is the headline drawn large.

pub mod figure;
pub mod push;
