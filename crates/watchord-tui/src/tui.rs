//! The event loop and the terminal it owns.
//!
//! One loop: key events from crossterm and model events through
//! [`AppModel::poll`], on a 50 ms tick. The terminal is put back — alternate
//! screen off, raw mode off, cursor shown — on `q`, on Ctrl-C, and on a panic,
//! because a chord displayer that leaves the shell unreadable is worse than one
//! that crashed.

use std::io::{self, Stdout, Write};
use std::panic;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use watchord_model::{AppModel, Screen};

use crate::screens::{self, UiState};

/// One tick of the loop: how long a key wait blocks before the model is polled.
const TICK: Duration = Duration::from_millis(50);

/// Set this variable to make the skin panic after its first frame. It exists
/// so the restore-on-panic path can be exercised from a shell.
pub const PANIC_TEST_VAR: &str = "WATCHORD_PANIC_TEST";

/// Puts the terminal back the way the shell had it. Safe to call twice.
pub fn restore() {
    let _ = disable_raw_mode();
    let mut stdout = io::stdout();
    let _ = execute!(stdout, LeaveAlternateScreen, crossterm::cursor::Show);
    let _ = stdout.flush();
}

/// The raw-mode alternate screen, restored on drop.
struct Guard;

impl Guard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        Ok(Guard)
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        restore();
    }
}

/// What one key asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Quit,
    NextScreen,
    PreviousScreen,
    SelectUp,
    SelectDown,
    ClearSelection,
    DeleteSelected,
    Commit,
    Backspace,
    Type(char),
    Nothing,
}

/// Reads a key against the state that decides what it means.
///
/// The field takes every printable key, so the two letter commands only fire
/// when the field is empty: `q` quits and `d` deletes the selected note. Ctrl-C
/// always quits.
fn action_for(key: KeyEvent, draft_is_empty: bool, has_selection: bool) -> Action {
    if key.kind == KeyEventKind::Release {
        return Action::Nothing;
    }
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('c') if ctrl => Action::Quit,
        KeyCode::Char('q') if draft_is_empty => Action::Quit,
        KeyCode::Char('d') if draft_is_empty && has_selection => Action::DeleteSelected,
        KeyCode::Tab => Action::NextScreen,
        KeyCode::BackTab => Action::PreviousScreen,
        KeyCode::Up => Action::SelectUp,
        KeyCode::Down => Action::SelectDown,
        KeyCode::Esc => Action::ClearSelection,
        KeyCode::Enter => Action::Commit,
        KeyCode::Backspace => Action::Backspace,
        KeyCode::Char(c) if !ctrl => Action::Type(c),
        _ => Action::Nothing,
    }
}

fn next_screen(screen: Screen, step: isize) -> Screen {
    let index = Screen::ALL.iter().position(|s| *s == screen).unwrap_or(0) as isize;
    let count = Screen::ALL.len() as isize;
    Screen::ALL[((index + step).rem_euclid(count)) as usize]
}

/// Applies one action. Returns false when the loop should end.
fn apply(action: Action, model: &mut AppModel, ui: &mut UiState) -> bool {
    let screen = model.screen;
    match action {
        Action::Quit => return false,
        Action::NextScreen => model.screen = next_screen(screen, 1),
        Action::PreviousScreen => model.screen = next_screen(screen, -1),
        Action::SelectUp | Action::SelectDown => {
            let count = screens::selectable_count(model, screen);
            if count == 0 {
                ui.set_selected(screen, None);
            } else {
                let current = ui.selected(screen);
                let next = match (action, current) {
                    (Action::SelectDown, None) => 0,
                    (Action::SelectDown, Some(i)) => (i + 1).min(count - 1),
                    (_, None) => count - 1,
                    (_, Some(i)) => i.saturating_sub(1),
                };
                ui.set_selected(screen, Some(next));
            }
        }
        Action::ClearSelection => ui.set_selected(screen, None),
        Action::DeleteSelected => {
            if let Some(id) = screens::selected_note_id(model, ui, screen) {
                model.delete_note(&id);
                let count = screens::selectable_count(model, screen);
                let kept = ui
                    .selected(screen)
                    .filter(|_| count > 0)
                    .map(|i| i.min(count - 1));
                ui.set_selected(screen, kept);
            }
        }
        Action::Commit => model.commit_note(),
        Action::Backspace => {
            model.draft_note_text.pop();
        }
        Action::Type(c) => model.draft_note_text.push(c),
        Action::Nothing => {}
    }
    true
}

/// Runs the skin until `q` or Ctrl-C. Starts and stops the model.
pub fn run(mut model: AppModel) -> io::Result<()> {
    let previous_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        restore();
        previous_hook(info);
    }));

    let guard = Guard::enter()?;
    let mut terminal: Terminal<CrosstermBackend<Stdout>> =
        Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut ui = UiState::default();
    let panic_test = std::env::var_os(PANIC_TEST_VAR).is_some();

    model.start();
    let mut frames: u64 = 0;
    loop {
        terminal.draw(|frame| screens::draw(frame, &model, &ui))?;
        frames += 1;
        if panic_test && frames > 2 {
            panic!("{PANIC_TEST_VAR} is set: proving the terminal restores");
        }

        if event::poll(TICK)? {
            match event::read()? {
                Event::Key(key) => {
                    let action = action_for(
                        key,
                        model.draft_note_text.is_empty(),
                        ui.selected(model.screen).is_some(),
                    );
                    if !apply(action, &mut model, &mut ui) {
                        break;
                    }
                }
                // A resize redraws on the next turn of the loop.
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
        model.poll();
    }

    model.stop();
    drop(guard);
    let _ = panic::take_hook();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn q_quits_only_when_the_field_is_empty() {
        assert_eq!(
            action_for(key(KeyCode::Char('q')), true, false),
            Action::Quit
        );
        assert_eq!(
            action_for(key(KeyCode::Char('q')), false, false),
            Action::Type('q')
        );
    }

    #[test]
    fn ctrl_c_always_quits() {
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(action_for(ctrl_c, false, true), Action::Quit);
    }

    #[test]
    fn d_deletes_only_with_a_selection_and_an_empty_field() {
        assert_eq!(
            action_for(key(KeyCode::Char('d')), true, true),
            Action::DeleteSelected
        );
        assert_eq!(
            action_for(key(KeyCode::Char('d')), true, false),
            Action::Type('d')
        );
        assert_eq!(
            action_for(key(KeyCode::Char('d')), false, true),
            Action::Type('d')
        );
    }

    #[test]
    fn tab_cycles_both_ways() {
        assert_eq!(next_screen(Screen::NowPlaying, 1), Screen::AllNotes);
        assert_eq!(next_screen(Screen::AllNotes, 1), Screen::NowPlaying);
        assert_eq!(next_screen(Screen::NowPlaying, -1), Screen::AllNotes);
    }
}
