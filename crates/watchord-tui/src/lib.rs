//! watchord-tui: the terminal skin.
//!
//! PUSH drawn with ratatui. [`push`] holds the tokens, the six structural
//! roles and the ported elements — the only module that draws a rule or a
//! box. [`figure`] is the headline drawn large. [`screens`] composes the two
//! screens from the model. [`tui`] owns the terminal and the event loop.

pub mod figure;
pub mod push;
pub mod screens;
pub mod tui;
pub mod when;

pub use screens::{UiState, draw, draw_into};
pub use tui::{PANIC_TEST_VAR, restore, run};
