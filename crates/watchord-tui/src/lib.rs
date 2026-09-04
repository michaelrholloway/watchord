//! watchord-tui: the terminal skins.
//!
//! Two skins draw the same [`watchord_model::Frame`]. [`push`] holds PUSH's
//! tokens, the six structural roles and the ported elements — the only module
//! that draws a rule or a box. [`figure`] is the headline drawn large.
//! [`screens`] composes PUSH's two screens. [`plain`] is the plain skin: the
//! terminal's own colours, no boxes, every field. [`tui`] owns the terminal
//! and the one event loop both skins share.

pub mod figure;
pub mod plain;
pub mod push;
pub mod screens;
pub mod tui;
pub mod when;

pub use screens::{Drawn, Hits, ScrollTarget, UiState, draw, draw_into};
pub use tui::{PANIC_TEST_VAR, restore, run};

/// Which skin draws the frame. `--skin push|plain`; the default is PUSH.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Skin {
    /// PUSH, drawn with its tokens and box roles. The look standard.
    #[default]
    Push,
    /// The plain skin: default foreground and background, no colour, no boxes,
    /// every field of the frame on screen.
    Plain,
}

impl Skin {
    /// The names `--skin` accepts.
    pub const NAMES: [&'static str; 2] = ["push", "plain"];

    /// `push` or `plain`, case-insensitive. `None` for anything else.
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "push" => Some(Skin::Push),
            "plain" => Some(Skin::Plain),
            _ => None,
        }
    }

    /// The name `--skin` would take to get this skin.
    pub fn name(self) -> &'static str {
        match self {
            Skin::Push => "push",
            Skin::Plain => "plain",
        }
    }
}
