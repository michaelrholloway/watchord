//! The event loop and the terminal it owns.
//!
//! One loop: key and mouse events from crossterm and model events through
//! [`AppModel::poll`], on a 50 ms tick. The terminal is put back — alternate
//! screen off, raw mode off, mouse capture off, cursor shown — on `q`, on
//! Ctrl-C, and on a panic,
//! because a chord displayer that leaves the shell unreadable is worse than one
//! that crashed.

use std::io::{self, Stdout, Write};
use std::panic;
use std::path::Path;
use std::time::{Duration, SystemTime};

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use watchord_model::{AppModel, Frame, Screen};

use crate::export;
use crate::screens::{self, Hits, ScrollTarget, UiState};
use crate::{Skin, plain};

/// One tick of the loop: how long a key wait blocks before the model is polled.
const TICK: Duration = Duration::from_millis(50);

/// Set this variable to make the skin panic after its first frame. It exists
/// so the restore-on-panic path can be exercised from a shell.
pub const PANIC_TEST_VAR: &str = "WATCHORD_PANIC_TEST";

/// Puts the terminal back the way the shell had it. Safe to call twice.
pub fn restore() {
    let _ = disable_raw_mode();
    let mut stdout = io::stdout();
    let _ = execute!(
        stdout,
        DisableMouseCapture,
        LeaveAlternateScreen,
        crossterm::cursor::Show
    );
    let _ = stdout.flush();
}

/// The raw-mode alternate screen, restored on drop.
struct Guard;

impl Guard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        Ok(Guard)
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        restore();
    }
}

/// What one key or click asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Quit,
    /// A tab was clicked.
    SwitchTo(Screen),
    /// A note row was clicked: select it.
    Select(usize),
    /// A row's `delete` was clicked: select it and delete it.
    Delete(usize),
    /// The note field was clicked: the keys go to it, so the selection goes.
    FocusField,
    /// The wheel turned over a table.
    Scroll(ScrollTarget, i32),
    NextScreen,
    PreviousScreen,
    SelectUp,
    SelectDown,
    ClearSelection,
    DeleteSelected,
    Commit,
    Backspace,
    Type(char),
    /// Steps the display one entry further into the past.
    StepHistoryBack,
    /// Steps the display one entry back toward now.
    StepHistoryForward,
    /// Writes the session as markdown.
    ExportMarkdown,
    /// Writes the session as JSON lines.
    ExportJson,
    /// Inserts a line break into the active field, rather than committing.
    Newline,
    /// `e`: loads the selected note into the field for editing.
    EditSelected,
    /// `s`: steps All Notes to its next sort order.
    CycleSort,
    /// `p`: enters drill if it is off, leaves it if it is on.
    ToggleDrill,
    /// `i`: opens or closes the input picker (ticket 13).
    ToggleInputPicker,
    /// The picker is open: ↑ moves the highlight up, wrapping.
    PickerUp,
    /// The picker is open: ↓ moves the highlight down, wrapping.
    PickerDown,
    /// The picker is open: enter chooses the highlighted input and closes it.
    PickerSelect,
    /// The picker is open: esc closes it without choosing.
    PickerClose,
    /// `-`: narrows the settle window by one step.
    SettleDown,
    /// `+`: widens the settle window by one step.
    SettleUp,
    /// `k`: steps the key context's tonic up a semitone (ticket #16).
    CycleKeyTonic,
    /// `m`: flips the key context's mode, major/minor (ticket #16).
    ToggleKeyMode,
    Nothing,
}

/// Reads a key against the state that decides what it means.
///
/// The field takes every printable key, so the letter and symbol commands
/// only fire when it is empty: `q` quits, `d` deletes the selected note, `e`
/// edits it, `s` cycles the All Notes sort order, `i` opens the input picker,
/// `-`/`+` adjust the settle window, `x` writes the session as markdown.
/// Ctrl-C always quits; Ctrl-`x` writes the session as JSON lines regardless
/// of the field. While the picker is open, ↑↓/enter/esc drive it instead of
/// their usual selection and commit meanings.
///
/// `x` was `e` at first, but the notes ticket bound `e` to edit-selected-note
/// first; moved per the conductor's ruling (ticket 12). Left/Right step
/// history back/forward — non-printable, so they need no
/// active-field-is-empty guard the way the letter keys do.
///
/// Shift-Enter inserts a line break rather than committing. crossterm reports
/// it as `KeyCode::Enter` with `KeyModifiers::SHIFT` on most terminals — this
/// is read from that combination. Some terminals never distinguish Shift-Enter
/// from a bare Enter at the protocol level, and on those this cannot tell the
/// two apart; there is no workaround from here.
fn action_for(
    key: KeyEvent,
    screen: Screen,
    active_field_is_empty: bool,
    has_selection: bool,
    picker_open: bool,
) -> Action {
    if key.kind == KeyEventKind::Release {
        return Action::Nothing;
    }
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    match key.code {
        KeyCode::Char('c') if ctrl => Action::Quit,
        KeyCode::Char('x') if ctrl => Action::ExportJson,
        KeyCode::Up if picker_open => Action::PickerUp,
        KeyCode::Down if picker_open => Action::PickerDown,
        KeyCode::Enter if picker_open => Action::PickerSelect,
        KeyCode::Esc if picker_open => Action::PickerClose,
        KeyCode::Char('q') if active_field_is_empty => Action::Quit,
        KeyCode::Char('d') if active_field_is_empty && has_selection => Action::DeleteSelected,
        KeyCode::Char('e') if active_field_is_empty && has_selection => Action::EditSelected,
        KeyCode::Char('s') if active_field_is_empty && screen == Screen::AllNotes => {
            Action::CycleSort
        }
        KeyCode::Char('p') if active_field_is_empty => Action::ToggleDrill,
        KeyCode::Char('i') if active_field_is_empty => Action::ToggleInputPicker,
        KeyCode::Char('-') if active_field_is_empty => Action::SettleDown,
        KeyCode::Char('+') if active_field_is_empty => Action::SettleUp,
        KeyCode::Char('k') if active_field_is_empty => Action::CycleKeyTonic,
        KeyCode::Char('m') if active_field_is_empty => Action::ToggleKeyMode,
        KeyCode::Char('x') if active_field_is_empty => Action::ExportMarkdown,
        KeyCode::Tab => Action::NextScreen,
        KeyCode::BackTab => Action::PreviousScreen,
        KeyCode::Up => Action::SelectUp,
        KeyCode::Down => Action::SelectDown,
        KeyCode::Left => Action::StepHistoryBack,
        KeyCode::Right => Action::StepHistoryForward,
        KeyCode::Esc => Action::ClearSelection,
        KeyCode::Enter if shift => Action::Newline,
        KeyCode::Enter => Action::Commit,
        KeyCode::Backspace => Action::Backspace,
        KeyCode::Char(c) if !ctrl => Action::Type(c),
        _ => Action::Nothing,
    }
}

/// Reads a click or a wheel turn against where the last frame put things.
/// Anything the frame did not lay out is nothing.
fn action_for_mouse(mouse: MouseEvent, hits: &Hits) -> Action {
    let (x, y) = (mouse.column, mouse.row);
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if let Some(screen) = hits.tab_at(x, y) {
                Action::SwitchTo(screen)
            } else if let Some(ordinal) = hits.delete_at(x, y) {
                Action::Delete(ordinal)
            } else if let Some(ordinal) = hits.note_row_at(x, y) {
                Action::Select(ordinal)
            } else if hits.field_at(x, y) {
                Action::FocusField
            } else {
                Action::Nothing
            }
        }
        MouseEventKind::ScrollUp => hits
            .scroll_at(x, y)
            .map_or(Action::Nothing, |target| Action::Scroll(target, -1)),
        MouseEventKind::ScrollDown => hits
            .scroll_at(x, y)
            .map_or(Action::Nothing, |target| Action::Scroll(target, 1)),
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
        Action::SwitchTo(to) => model.screen = to,
        Action::Select(ordinal) => ui.set_selected(screen, Some(ordinal)),
        Action::Delete(ordinal) => {
            ui.set_selected(screen, Some(ordinal));
            return apply(Action::DeleteSelected, model, ui);
        }
        Action::FocusField => ui.set_selected(screen, None),
        Action::Scroll(target, delta) => {
            let rows = ui.scroll.get(target);
            let next = if delta < 0 {
                rows.saturating_sub(delta.unsigned_abs() as usize)
            } else {
                rows.saturating_add(delta as usize)
            };
            ui.scroll.set(target, next);
        }
        Action::NextScreen => model.screen = next_screen(screen, 1),
        Action::PreviousScreen => model.screen = next_screen(screen, -1),
        Action::SelectUp | Action::SelectDown => {
            let count = screens::selectable_count(&model.frame(), screen);
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
        Action::ClearSelection => {
            ui.set_selected(screen, None);
            model.cancel_edit();
        }
        Action::DeleteSelected => {
            if let Some(id) = screens::selected_note_id(&model.frame(), ui, screen) {
                model.delete_note(&id);
                let count = screens::selectable_count(&model.frame(), screen);
                let kept = ui
                    .selected(screen)
                    .filter(|_| count > 0)
                    .map(|i| i.min(count - 1));
                ui.set_selected(screen, kept);
            }
        }
        Action::EditSelected => {
            if let Some(id) = screens::selected_note_id(&model.frame(), ui, screen) {
                model.begin_edit(&id);
            }
        }
        Action::StepHistoryBack => model.step_history_back(),
        Action::StepHistoryForward => model.step_history_forward(),
        Action::ExportMarkdown => run_export(model, export::write_markdown),
        Action::ExportJson => run_export(model, export::write_json_lines),
        Action::CycleSort => model.cycle_notes_sort(),
        Action::ToggleDrill => model.toggle_drill(),
        Action::Commit => model.commit_note(),
        Action::Backspace => match screen {
            Screen::AllNotes => {
                model.search_text.pop();
            }
            Screen::NowPlaying => {
                model.draft_note_text.pop();
            }
        },
        Action::Type(c) => match screen {
            Screen::AllNotes => model.search_text.push(c),
            Screen::NowPlaying => model.draft_note_text.push(c),
        },
        Action::Newline => {
            if screen == Screen::NowPlaying {
                model.draft_note_text.push('\n');
            }
        }
        Action::ToggleInputPicker => {
            ui.input_picker_open = !ui.input_picker_open;
            ui.input_picker_index = 0;
        }
        Action::PickerUp => {
            let count = model.connected_inputs().len();
            if count > 0 {
                ui.input_picker_index = (ui.input_picker_index + count - 1) % count;
            }
        }
        Action::PickerDown => {
            let count = model.connected_inputs().len();
            if count > 0 {
                ui.input_picker_index = (ui.input_picker_index + 1) % count;
            }
        }
        Action::PickerSelect => {
            if let Some(name) = model.connected_inputs().get(ui.input_picker_index).cloned() {
                model.select_input(Some(name));
            }
            ui.input_picker_open = false;
        }
        Action::PickerClose => ui.input_picker_open = false,
        Action::SettleDown => model.decrease_settle(),
        Action::SettleUp => model.increase_settle(),
        Action::CycleKeyTonic => model.cycle_key_tonic(),
        Action::ToggleKeyMode => model.toggle_key_mode(),
        Action::Nothing => {}
    }
    true
}

/// Resolves the real sessions directory, writes with `write`, and reports the
/// outcome on the model's error row — the same row a failed save uses.
fn run_export(
    model: &mut AppModel,
    write: fn(&Frame, &Path, SystemTime) -> io::Result<std::path::PathBuf>,
) {
    let Some(directory) = export::sessions_directory() else {
        model.set_status(Some(
            "could not export — no home directory on this machine".to_string(),
        ));
        return;
    };
    let frame = model.frame();
    match write(&frame, &directory, SystemTime::now()) {
        Ok(path) => model.set_status(Some(format!("exported to {}", path.display()))),
        Err(error) => model.set_status(Some(format!("could not export — {error}"))),
    }
}

/// Draws one frame of `skin`. Returns what the mouse can hit in it.
fn draw(target: &mut ratatui::Frame, skin: Skin, model: &AppModel, ui: &UiState) -> Hits {
    let frame = model.frame();
    match skin {
        Skin::Push => screens::draw(target, &frame, ui),
        Skin::Plain => plain::draw(target, &frame, ui),
    }
}

/// Runs `skin` until `q` or Ctrl-C. Starts and stops the model.
pub fn run(mut model: AppModel, skin: Skin) -> io::Result<()> {
    let previous_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        restore();
        previous_hook(info);
    }));

    let guard = Guard::enter()?;
    let mut terminal: Terminal<CrosstermBackend<Stdout>> =
        Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut ui = UiState::default();
    let mut hits = Hits::default();
    let panic_test = std::env::var_os(PANIC_TEST_VAR).is_some();

    model.start();
    let mut frames: u64 = 0;
    loop {
        terminal.draw(|target| hits = draw(target, skin, &model, &ui))?;
        frames += 1;
        if panic_test && frames > 2 {
            panic!("{PANIC_TEST_VAR} is set: proving the terminal restores");
        }

        if event::poll(TICK)? {
            match event::read()? {
                Event::Key(key) => {
                    let active_field_is_empty = match model.screen {
                        Screen::NowPlaying => model.draft_note_text.is_empty(),
                        Screen::AllNotes => model.search_text.is_empty(),
                    };
                    let action = action_for(
                        key,
                        model.screen,
                        active_field_is_empty,
                        ui.selected(model.screen).is_some(),
                        ui.input_picker_open,
                    );
                    if !apply(action, &mut model, &mut ui) {
                        break;
                    }
                }
                Event::Mouse(mouse) => {
                    let action = action_for_mouse(mouse, &hits);
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
    use std::sync::Arc;
    use std::time::UNIX_EPOCH;

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use watchord_core::{ChordNote, SoundingSet, SpellingOrigin};
    use watchord_model::fakes::{
        InMemoryNoteStore, ScriptedSoundingSetSource, StubAlternate, StubChordNaming,
    };

    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// A C6 with two notes on it, played and settled.
    fn model_with_notes() -> AppModel {
        let c6 = SoundingSet::new([60, 64, 67, 69]);
        let naming = StubChordNaming::new();
        naming.stub(
            &c6,
            StubChordNaming::naming(
                &c6,
                "C6",
                "C major 6",
                vec![StubAlternate::new(
                    "Am7",
                    SpellingOrigin::ReRooted,
                    "A minor 7",
                )],
            ),
        );
        let store = InMemoryNoteStore::new(vec![
            ChordNote::new("first", c6.key(), "C6", "older", UNIX_EPOCH),
            ChordNote::new(
                "second",
                c6.key(),
                "C6",
                "newer",
                UNIX_EPOCH + Duration::from_secs(60),
            ),
        ]);
        let mut model = AppModel::new(
            Arc::new(naming),
            Box::new(ScriptedSoundingSetSource::new(vec![c6])),
            Arc::new(store),
        );
        model.start();
        while model.wait(Duration::from_millis(50)) {}
        model
    }

    /// One frame at 120x40, returning what it laid out for the mouse.
    fn frame(model: &AppModel, ui: &UiState) -> Hits {
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).expect("a test terminal");
        let mut hits = Hits::default();
        terminal
            .draw(|target| hits = screens::draw(target, &model.frame(), ui))
            .expect("a frame");
        hits
    }

    fn click(x: u16, y: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn middle(rect: Rect) -> (u16, u16) {
        (rect.x + rect.width / 2, rect.y)
    }

    #[test]
    fn clicking_the_all_notes_tab_switches_screens() {
        let mut model = model_with_notes();
        let mut ui = UiState::default();
        let hits = frame(&model, &ui);
        let (rect, _) = hits
            .tabs
            .iter()
            .find(|(_, screen)| *screen == Screen::AllNotes)
            .expect("an All Notes tab");
        let (x, y) = middle(*rect);

        let action = action_for_mouse(click(x, y), &hits);
        assert_eq!(action, Action::SwitchTo(Screen::AllNotes));
        apply(action, &mut model, &mut ui);
        assert_eq!(model.screen, Screen::AllNotes);

        // The control: a click on the page ground does nothing.
        assert_eq!(action_for_mouse(click(x, 39), &hits), Action::Nothing);
    }

    #[test]
    fn clicking_delete_on_a_row_deletes_that_note() {
        let mut model = model_with_notes();
        let mut ui = UiState::default();
        let hits = frame(&model, &ui);
        assert_eq!(model.notes_for_displayed_chord().len(), 2);
        // Row 0 is the newest note, "newer".
        let (cell, ordinal) = hits.delete_cells[0];
        assert_eq!(ordinal, 0);
        let (x, y) = (cell.x + cell.width - 1, cell.y);

        let action = action_for_mouse(click(x, y), &hits);
        assert_eq!(action, Action::Delete(0));
        apply(action, &mut model, &mut ui);
        let left: Vec<&str> = model
            .notes_for_displayed_chord()
            .iter()
            .map(|n| n.text.as_str())
            .collect();
        assert_eq!(left, ["older"]);

        // The control: the same row's text cell selects rather than deletes.
        let hits = frame(&model, &ui);
        let (row, _) = hits.note_rows[0];
        assert_eq!(
            action_for_mouse(click(row.x + 3, row.y), &hits),
            Action::Select(0)
        );
    }

    #[test]
    fn q_quits_only_when_the_field_is_empty() {
        assert_eq!(
            action_for(
                key(KeyCode::Char('q')),
                Screen::NowPlaying,
                true,
                false,
                false
            ),
            Action::Quit
        );
        assert_eq!(
            action_for(
                key(KeyCode::Char('q')),
                Screen::NowPlaying,
                false,
                false,
                false
            ),
            Action::Type('q')
        );
    }

    #[test]
    fn ctrl_c_always_quits() {
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(
            action_for(ctrl_c, Screen::NowPlaying, false, true, false),
            Action::Quit
        );
    }

    #[test]
    fn d_deletes_only_with_a_selection_and_an_empty_field() {
        assert_eq!(
            action_for(
                key(KeyCode::Char('d')),
                Screen::NowPlaying,
                true,
                true,
                false
            ),
            Action::DeleteSelected
        );
        assert_eq!(
            action_for(
                key(KeyCode::Char('d')),
                Screen::NowPlaying,
                true,
                false,
                false
            ),
            Action::Type('d')
        );
        assert_eq!(
            action_for(
                key(KeyCode::Char('d')),
                Screen::NowPlaying,
                false,
                true,
                false
            ),
            Action::Type('d')
        );
    }

    #[test]
    fn x_exports_only_with_an_empty_field_ctrl_x_exports_json_regardless() {
        assert_eq!(
            action_for(
                key(KeyCode::Char('x')),
                Screen::NowPlaying,
                true,
                false,
                false
            ),
            Action::ExportMarkdown
        );
        assert_eq!(
            action_for(
                key(KeyCode::Char('x')),
                Screen::NowPlaying,
                false,
                false,
                false
            ),
            Action::Type('x'),
            "typing into the draft still wins"
        );
        let ctrl_x = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL);
        assert_eq!(
            action_for(ctrl_x, Screen::NowPlaying, false, false, false),
            Action::ExportJson
        );
    }

    #[test]
    fn left_and_right_step_history_regardless_of_the_draft() {
        assert_eq!(
            action_for(key(KeyCode::Left), Screen::NowPlaying, false, false, false),
            Action::StepHistoryBack
        );
        assert_eq!(
            action_for(key(KeyCode::Right), Screen::NowPlaying, false, false, false),
            Action::StepHistoryForward
        );
    }

    #[test]
    fn e_edits_only_with_a_selection_and_an_empty_field() {
        assert_eq!(
            action_for(
                key(KeyCode::Char('e')),
                Screen::NowPlaying,
                true,
                true,
                false
            ),
            Action::EditSelected
        );
        assert_eq!(
            action_for(
                key(KeyCode::Char('e')),
                Screen::NowPlaying,
                true,
                false,
                false
            ),
            Action::Type('e')
        );
        assert_eq!(
            action_for(
                key(KeyCode::Char('e')),
                Screen::NowPlaying,
                false,
                true,
                false
            ),
            Action::Type('e')
        );
    }

    #[test]
    fn s_cycles_sort_only_on_all_notes_with_an_empty_field() {
        assert_eq!(
            action_for(
                key(KeyCode::Char('s')),
                Screen::AllNotes,
                true,
                false,
                false
            ),
            Action::CycleSort
        );
        assert_eq!(
            action_for(
                key(KeyCode::Char('s')),
                Screen::NowPlaying,
                true,
                false,
                false
            ),
            Action::Type('s'),
            "sort has no meaning on Now Playing"
        );
        assert_eq!(
            action_for(
                key(KeyCode::Char('s')),
                Screen::AllNotes,
                false,
                false,
                false
            ),
            Action::Type('s')
        );
    }

    #[test]
    fn shift_enter_inserts_a_line_break_plain_enter_commits() {
        let shift_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT);
        assert_eq!(
            action_for(shift_enter, Screen::NowPlaying, true, false, false),
            Action::Newline
        );
        assert_eq!(
            action_for(key(KeyCode::Enter), Screen::NowPlaying, true, false, false),
            Action::Commit
        );
    }

    #[test]
    fn typing_goes_to_the_draft_on_now_playing_and_the_search_on_all_notes() {
        let mut model = model_with_notes();
        let mut ui = UiState::default();

        model.screen = Screen::NowPlaying;
        apply(Action::Type('x'), &mut model, &mut ui);
        assert!(model.draft_note_text.ends_with('x'));
        assert_eq!(model.search_text, "");

        model.screen = Screen::AllNotes;
        apply(Action::Type('y'), &mut model, &mut ui);
        assert_eq!(model.search_text, "y");

        apply(Action::Backspace, &mut model, &mut ui);
        assert_eq!(model.search_text, "");
    }

    #[test]
    fn e_loads_the_selected_note_for_editing_and_commit_updates_it() {
        let mut model = model_with_notes();
        let mut ui = UiState::default();
        ui.set_selected(Screen::NowPlaying, Some(0));
        let target_id = model.notes_for_displayed_chord()[0].id.clone();

        apply(Action::EditSelected, &mut model, &mut ui);
        assert!(model.is_editing());
        assert_eq!(
            model.draft_note_text,
            model.notes_for_displayed_chord()[0].text
        );

        model.draft_note_text = "edited via the field".into();
        apply(Action::Commit, &mut model, &mut ui);

        assert!(!model.is_editing());
        assert_eq!(
            model.notes_for_displayed_chord()[0].text,
            "edited via the field"
        );
        assert_eq!(model.notes_for_displayed_chord()[0].id, target_id);
    }

    #[test]
    fn esc_cancels_an_edit_in_progress() {
        let mut model = model_with_notes();
        let mut ui = UiState::default();
        ui.set_selected(Screen::NowPlaying, Some(0));
        apply(Action::EditSelected, &mut model, &mut ui);
        assert!(model.is_editing());

        apply(Action::ClearSelection, &mut model, &mut ui);
        assert!(!model.is_editing());
        assert_eq!(model.draft_note_text, "");
    }

    #[test]
    fn tab_cycles_both_ways() {
        assert_eq!(next_screen(Screen::NowPlaying, 1), Screen::AllNotes);
        assert_eq!(next_screen(Screen::AllNotes, 1), Screen::NowPlaying);
        assert_eq!(next_screen(Screen::NowPlaying, -1), Screen::AllNotes);
    }

    // MARK: - Pedals, settle, and the input picker (ticket 13)

    #[test]
    fn i_opens_the_picker_only_when_the_field_is_empty() {
        assert_eq!(
            action_for(
                key(KeyCode::Char('i')),
                Screen::NowPlaying,
                true,
                false,
                false
            ),
            Action::ToggleInputPicker
        );
        assert_eq!(
            action_for(
                key(KeyCode::Char('i')),
                Screen::NowPlaying,
                false,
                false,
                false
            ),
            Action::Type('i'),
            "the field wins while it has text"
        );
    }

    #[test]
    fn settle_keys_fire_only_when_the_field_is_empty() {
        assert_eq!(
            action_for(
                key(KeyCode::Char('-')),
                Screen::NowPlaying,
                true,
                false,
                false
            ),
            Action::SettleDown
        );
        assert_eq!(
            action_for(
                key(KeyCode::Char('+')),
                Screen::NowPlaying,
                true,
                false,
                false
            ),
            Action::SettleUp
        );
        assert_eq!(
            action_for(
                key(KeyCode::Char('-')),
                Screen::NowPlaying,
                false,
                false,
                false
            ),
            Action::Type('-'),
            "the field wins while it has text"
        );
    }

    #[test]
    fn while_the_picker_is_open_up_down_enter_and_esc_drive_it_not_selection() {
        assert_eq!(
            action_for(key(KeyCode::Up), Screen::NowPlaying, true, false, true),
            Action::PickerUp
        );
        assert_eq!(
            action_for(key(KeyCode::Down), Screen::NowPlaying, true, false, true),
            Action::PickerDown
        );
        assert_eq!(
            action_for(key(KeyCode::Enter), Screen::NowPlaying, true, false, true),
            Action::PickerSelect
        );
        assert_eq!(
            action_for(key(KeyCode::Esc), Screen::NowPlaying, true, false, true),
            Action::PickerClose
        );
        // The control: closed, the same keys mean what they always meant.
        assert_eq!(
            action_for(key(KeyCode::Up), Screen::NowPlaying, true, false, false),
            Action::SelectUp
        );
        assert_eq!(
            action_for(key(KeyCode::Enter), Screen::NowPlaying, true, false, false),
            Action::Commit
        );
        assert_eq!(
            action_for(key(KeyCode::Esc), Screen::NowPlaying, true, false, false),
            Action::ClearSelection
        );
    }

    #[test]
    fn toggling_the_picker_opens_it_on_the_first_input_and_closes_it_again() {
        let mut model = model_with_notes();
        let mut ui = UiState::default();
        assert!(!ui.input_picker_open);

        apply(Action::ToggleInputPicker, &mut model, &mut ui);
        assert!(ui.input_picker_open);
        assert_eq!(ui.input_picker_index, 0);

        apply(Action::ToggleInputPicker, &mut model, &mut ui);
        assert!(!ui.input_picker_open);
    }

    /// An idle model with two connected inputs, started and pumped so
    /// `connected_inputs()` already reports them, plus a handle onto the same
    /// scripted source so a test can read back what reached it.
    fn model_with_two_inputs() -> (AppModel, ScriptedSoundingSetSource) {
        let store = InMemoryNoteStore::default();
        let source = ScriptedSoundingSetSource::default();
        source.attach(&["Nord Stage 3", "IAC Driver Bus 1"]);
        let mut model = AppModel::new(
            Arc::new(StubChordNaming::new()),
            Box::new(source.clone()),
            Arc::new(store),
        );
        model.start();
        while model.connected_inputs().len() < 2 {
            model.wait(Duration::from_millis(50));
        }
        (model, source)
    }

    #[test]
    fn picker_up_and_down_wrap_around_the_connected_inputs() {
        let (mut model, _source) = model_with_two_inputs();
        let mut ui = UiState::default();
        assert_eq!(ui.input_picker_index, 0);

        apply(Action::PickerUp, &mut model, &mut ui);
        assert_eq!(ui.input_picker_index, 1, "up from 0 wraps to the last row");
        apply(Action::PickerDown, &mut model, &mut ui);
        assert_eq!(
            ui.input_picker_index, 0,
            "down from the last row wraps to 0"
        );
        apply(Action::PickerDown, &mut model, &mut ui);
        assert_eq!(ui.input_picker_index, 1);
    }

    #[test]
    fn picker_select_calls_select_input_with_the_highlighted_name_and_closes() {
        let (mut model, source) = model_with_two_inputs();
        let mut ui = UiState {
            input_picker_open: true,
            input_picker_index: 1,
            ..Default::default()
        };
        apply(Action::PickerSelect, &mut model, &mut ui);
        assert!(!ui.input_picker_open, "selecting closes the picker");
        assert_eq!(
            source.last_selected_input(),
            Some(Some("IAC Driver Bus 1".to_string())),
            "the highlighted row's name reached the source"
        );
    }

    #[test]
    fn settle_keys_move_the_models_settle_window() {
        let mut model = model_with_notes();
        assert_eq!(model.settle_ms(), 60);
        apply(Action::SettleDown, &mut model, &mut UiState::default());
        assert_eq!(model.settle_ms(), 50);
        apply(Action::SettleUp, &mut model, &mut UiState::default());
        apply(Action::SettleUp, &mut model, &mut UiState::default());
        assert_eq!(model.settle_ms(), 70);
    }
}
