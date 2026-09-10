//! The packed skin (spec #20), rendered through `TestBackend`. Each frame is
//! committed under `tests/snapshots/packed-*.txt` and compared byte for byte;
//! re-record with `WATCHORD_UPDATE_SNAPSHOTS=1`.
//!
//! Every snapshot is grepped for the words the design puts on screen, with a
//! control word that must be absent, and for the absence of the fields the
//! skin leaves off (drill, settle, score).

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use watchord_core::{ChordFit, ChordNote, SoundingSet, SpellingOrigin};
use watchord_model::fakes::{
    InMemoryNoteStore, ScriptedSoundingSetSource, StubAlternate, StubChordNaming,
};
use watchord_model::{AppModel, Frame, Screen};
use watchord_tui::{Hits, UiState, packed};

/// A word no renderer emits. The grep must miss it, or the grep is not a test.
const CONTROL: &str = "ZZZ_NOT_ON_SCREEN";

/// The `--fake` graph: a C6 with two notes on it.
fn fake_model(released: bool) -> AppModel {
    let c6 = SoundingSet::new([60, 64, 67, 69]);
    let naming = StubChordNaming::new();
    naming.stub(
        &c6,
        StubChordNaming::naming(
            &c6,
            "C6",
            "C major 6",
            vec![
                StubAlternate::new("Am7", SpellingOrigin::ReRooted, "A minor 7"),
                StubAlternate::new("CΔ6", SpellingOrigin::Enharmonic, "C major 6, major 7"),
            ],
        ),
    );
    let script = if released {
        vec![c6.clone(), SoundingSet::silent()]
    } else {
        vec![c6.clone()]
    };
    let store = InMemoryNoteStore::new(vec![
        ChordNote::new(
            "1",
            c6.key(),
            "C6",
            "sounds like the Rhodes on Voodoo",
            at(1_787_000_000),
        ),
        ChordNote::new(
            "2",
            c6.key(),
            "Am7",
            "try it with the 9 on top",
            at(1_787_003_540),
        ),
    ]);
    let mut model = AppModel::new(
        Arc::new(naming),
        Box::new(ScriptedSoundingSetSource::new(script)),
        Arc::new(store),
    );
    model.announce("fake input — no MIDI hardware is being read");
    model.start();
    while model.wait(Duration::from_millis(50)) {}
    model
}

/// A `Nearest` headline with a `Missing` alternate: `≈` before the name, a
/// detail line under it, and a tier word in the FIT column.
fn nearest_model() -> AppModel {
    let keys = SoundingSet::new([64, 72, 82, 86, 93]);
    let cm7_tones: [i32; 4] = [0, 3, 7, 10];
    let c13_tones: [i32; 7] = [0, 4, 7, 10, 2, 5, 9];
    let naming = StubChordNaming::new();
    naming.stub(
        &keys,
        StubChordNaming::naming_with_fit(
            &keys,
            "Cm7",
            "C minor 7",
            ChordFit::Nearest { distance: 4 },
            Some(&cm7_tones),
            vec![
                StubAlternate::new("C13", SpellingOrigin::ReRooted, "C major, minor 7, 13")
                    .with_fit(ChordFit::Missing(vec![5, 11]), &c13_tones),
            ],
        ),
    );
    let mut model = AppModel::new(
        Arc::new(naming),
        Box::new(ScriptedSoundingSetSource::new(vec![keys])),
        Arc::new(InMemoryNoteStore::default()),
    );
    model.start();
    while model.wait(Duration::from_millis(50)) {}
    model
}

/// Keys the stub has no name for: the engine declines.
fn declined_model() -> AppModel {
    let keys = SoundingSet::new([60, 61]);
    let mut model = AppModel::new(
        Arc::new(StubChordNaming::new()),
        Box::new(ScriptedSoundingSetSource::new(vec![keys])),
        Arc::new(InMemoryNoteStore::default()),
    );
    model.start();
    while model.wait(Duration::from_millis(50)) {}
    model
}

/// Nothing played yet.
fn idle_model() -> AppModel {
    let mut model = AppModel::new(
        Arc::new(StubChordNaming::new()),
        Box::new(ScriptedSoundingSetSource::default()),
        Arc::new(InMemoryNoteStore::default()),
    );
    model.start();
    while model.wait(Duration::from_millis(50)) {}
    model
}

/// `--fake-rich`: nine chords, a key context, three notes on the last chord.
fn rich_model() -> AppModel {
    let mut model = watchord_model::fakes::rich_demo(Screen::NowPlaying);
    model.start();
    while model.wait(Duration::from_millis(50)) {}
    model
}

fn at(unix: u64) -> std::time::SystemTime {
    UNIX_EPOCH + Duration::from_secs(unix)
}

/// One packed frame as rows of text, trailing spaces cut, plus its hits.
fn render(frame: &Frame, ui: &UiState, width: u16, height: u16) -> (Vec<String>, Hits) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("a test terminal");
    let mut hits = Hits::default();
    terminal
        .draw(|target| hits = packed::draw(target, frame, ui))
        .expect("a frame");
    let buffer = terminal.backend().buffer();
    let rows = (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect();
    (rows, hits)
}

fn text_of(rows: &[String]) -> String {
    rows.join("\n")
}

fn snapshot_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots")
        .join(format!("{name}.txt"))
}

fn assert_snapshot(name: &str, rows: &[String]) {
    let path = snapshot_path(name);
    let actual = text_of(rows) + "\n";
    if std::env::var_os("WATCHORD_UPDATE_SNAPSHOTS").is_some() || !path.exists() {
        fs::create_dir_all(path.parent().expect("a parent")).expect("snapshot dir");
        fs::write(&path, &actual).expect("write snapshot");
        return;
    }
    let expected = fs::read_to_string(&path).expect("read snapshot");
    assert_eq!(
        expected,
        actual,
        "frame differs from {}; re-record with WATCHORD_UPDATE_SNAPSHOTS=1",
        path.display()
    );
}

/// Every word of `present` is in `text` and every word of `absent` is not,
/// with the control proving the grep can miss.
fn assert_words(text: &str, present: &[&str], absent: &[&str], what: &str) {
    assert!(
        !text.contains(CONTROL),
        "{what}: the control word is present"
    );
    for word in present {
        assert!(text.contains(word), "{what}: {word:?} is missing\n{text}");
    }
    for word in absent {
        assert!(
            !text.contains(word),
            "{what}: {word:?} must be absent\n{text}"
        );
    }
}

/// The words every Now Playing frame carries, whatever the chord.
const CHROME: [&str; 12] = [
    "WATCHORD",
    "KEY",
    "FUNCTION:",
    "NUMERAL:",
    "KEYS:",
    "READINGS",
    "HISTORY",
    "NOTES",
    "SAVE[⏎]",
    "VIEW ALL[N]",
    "SUSTAIN:",
    "ARPEGGIO[A]:",
];
/// The fields the skin leaves off screen (spec #20).
const OFF_SCREEN: [&str; 7] = [
    "DRILL",
    "SETTLE",
    "SCORE",
    "SOSTENUTO",
    "SOFT",
    "NASHVILLE",
    "VOICE LEADING",
];

#[test]
fn now_playing_at_80x24_draws_the_design() {
    let model = fake_model(false);
    let (rows, hits) = render(&model.frame(), &UiState::default(), 80, 24);
    let text = text_of(&rows);
    assert_words(
        &text,
        &[
            "C6",
            "C MAJOR 6",
            "EXACT",
            "C3",
            "A3",
            "STATE: PLAYING",
            "INPUT[I]: FAKE",
            "CHORD",
            "NOTE",
        ],
        &OFF_SCREEN,
        "packed 80x24",
    );
    assert_words(&text, &CHROME, &[], "packed 80x24 chrome");
    assert_eq!(rows.len(), 24);
    assert!(rows.iter().all(|r| r.chars().count() <= 80));
    // At 24 rows the three readings fit; history holds one entry and no
    // note row draws. The field and VIEW ALL are still there.
    assert_words(
        &text,
        &["Am7", "REROOTED", "CΔ6", "ENHARMONIC"],
        &[],
        "packed 80x24 readings",
    );
    assert!(hits.note_rows.is_empty());
    assert!(hits.field.is_some());
    assert_eq!(hits.tabs.len(), 1);
    assert_eq!(hits.tabs[0].1, Screen::AllNotes);
    // Nothing is selected, so no DEL cell is drawn.
    assert!(hits.delete_cells.is_empty());
    // The title heads the left column over a rule that meets the staff
    // column's divider; KEY sits right under the rule.
    assert!(rows[1].starts_with("│ WATCHORD"), "{}", rows[1]);
    assert!(
        rows[2].starts_with("├───") && rows[2].contains("┤"),
        "{}",
        rows[2]
    );
    assert!(
        rows[3].starts_with("│KEY"),
        "KEY sits right under the rule\n{text}"
    );
    // The staff column: the divider runs from the top border to the foot
    // rule; the headline sits at its top, name over spoken form, with no
    // rule under it.
    assert!(rows[0].contains('┬'), "{}", rows[0]);
    assert!(rows[21].contains('┴'), "{}", rows[21]);
    let staff_column = |y: usize| -> String { rows[y].chars().skip(60).collect() };
    // `C6` fits the column as a fine figure: five rows of half blocks.
    // A blank row, the figure, a blank row, the spoken form, a blank row.
    let blank = |y: usize| staff_column(y).trim_matches('│').trim().is_empty();
    assert!(blank(1), "{}", rows[1]);
    for (y, row) in rows.iter().enumerate().take(7).skip(2) {
        assert!(
            staff_column(y).chars().any(|c| "▀▄█".contains(c)),
            "row {y} is figure\n{row}"
        );
    }
    assert!(blank(7), "{}", rows[7]);
    let spoken = staff_column(8);
    assert!(spoken.contains("C MAJOR 6"), "{}", rows[8]);
    // Centred: as much space on the left as on the right, give or take one.
    let lead = spoken.len() - spoken.trim_start().len();
    let trail = spoken.trim_end_matches('│').len() - spoken.trim_end_matches('│').trim_end().len();
    assert!(
        lead.abs_diff(trail) <= 1,
        "lead {lead} trail {trail}\n{}",
        rows[8]
    );
    assert!(blank(9), "{}", rows[9]);
    assert!(
        !rows[9].contains('┤'),
        "no rule under the headline\n{}",
        rows[9]
    );
    assert_snapshot("packed-now-playing-80x24", &rows);

    // Four rows taller, the three readings and both notes fit.
    let (rows, hits) = render(&model.frame(), &UiState::default(), 80, 28);
    let text = text_of(&rows);
    assert_words(
        &text,
        &[
            "Am7",
            "REROOTED",
            "CΔ6",
            "ENHARMONIC",
            "sounds like the Rhodes on Voodoo",
            "try it with the 9 on top",
        ],
        &OFF_SCREEN,
        "packed 80x28",
    );
    assert_eq!(hits.note_rows.len(), 2);
}

#[test]
fn a_taller_terminal_gives_the_lists_more_rows() {
    let model = rich_model();
    let frame = model.frame();
    let (rows, _) = render(&frame, &UiState::default(), 120, 40);
    let text = text_of(&rows);
    // Nine history entries: at 40 rows they all show, one row each; at 24
    // only the newest.
    let names: Vec<&str> = frame.history.iter().map(|e| e.name.as_str()).collect();
    for name in &names {
        assert!(text.contains(name), "{name}\n{text}");
    }
    // One row each, no blank between: G7's row is right under Dm7's.
    let row_of = |name: &str| {
        rows.iter()
            .position(|r| r.contains(&format!("│{name:<12}")))
            .unwrap_or_else(|| panic!("{name} row\n{text}"))
    };
    assert_eq!(row_of("G7"), row_of("Dm7") + 1, "{text}");
    let (tall, _) = render(&frame, &UiState::default(), 120, 50);
    let tall_text = text_of(&tall);
    for name in &names {
        assert!(tall_text.contains(name), "{name} at 50 rows\n{tall_text}");
    }
    assert_words(
        &text,
        &["C MAJOR", "V7/IV", "Eb"],
        &OFF_SCREEN,
        "packed 120x40",
    );
    assert_snapshot("packed-now-playing-120x40", &rows);

    let (rows, _) = render(&frame, &UiState::default(), 80, 24);
    let text = text_of(&rows);
    assert!(text.contains("C7#9"), "{text}");
    assert!(
        !text.contains("CΔ7"),
        "the oldest entry is off screen at 24 rows\n{text}"
    );
    assert_snapshot("packed-fake-rich-80x24", &rows);
}

#[test]
fn both_clefs_draw_when_the_box_has_room_and_treble_alone_below() {
    let model = rich_model();
    let frame = model.frame();
    // C7#9 as the Hendrix voicing spans both staves: 36 is deep in the bass.
    let bass_notes = frame
        .annotations
        .staff
        .iter()
        .filter(|n| n.clef == watchord_theory::staff::Clef::Bass)
        .count();
    assert!(bass_notes > 0, "the fixture must have a bass note");
    let heads_at = |height: u16| {
        let (rows, _) = render(&frame, &UiState::default(), 80, height);
        // The staff column: right of the divider at x 59, under the two
        // headline rows, down to the row above the foot rule.
        let staff: String = rows
            .iter()
            .skip(3)
            .take(height as usize - 6)
            .map(|r| r.chars().skip(60).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        (staff.matches('■').count(), rows)
    };
    let (heads_24, _) = heads_at(24);
    let (heads_34, rows_34) = heads_at(34);
    assert!(
        heads_34 > heads_24,
        "34 rows: {heads_34} heads, 24 rows: {heads_24}"
    );
    // C1 sits two octaves under the bass staff; at 40 rows every head fits.
    let (heads_40, _) = heads_at(40);
    assert_eq!(heads_40, frame.annotations.staff.len());
    assert_snapshot("packed-now-playing-80x34", &rows_34);
}

#[test]
fn released_reads_released_and_the_dot_goes_dim() {
    let model = fake_model(true);
    let (rows, _) = render(&model.frame(), &UiState::default(), 80, 24);
    let text = text_of(&rows);
    assert_words(
        &text,
        &["STATE: RELEASED", "C6"],
        &["PLAYING"],
        "packed released",
    );
    assert_snapshot("packed-released-80x24", &rows);
}

#[test]
fn nearest_marks_the_name_and_writes_the_detail() {
    let model = nearest_model();
    let (rows, _) = render(&model.frame(), &UiState::default(), 80, 24);
    let text = text_of(&rows);
    assert_words(&text, &["≈ Cm7", "NEAREST"], &[], "packed nearest");
    assert_snapshot("packed-nearest-80x24", &rows);
    // Wider and taller, the second reading shows, the FIT column takes the
    // slack, and its detail shows in full.
    let (rows, _) = render(&model.frame(), &UiState::default(), 120, 28);
    let text = text_of(&rows);
    assert_words(&text, &["MISSING ·no5 ·no11"], &[], "packed nearest 120");
}

#[test]
fn declined_says_why_in_the_headline_box() {
    let model = declined_model();
    let frame = model.frame();
    assert!(frame.declined.is_some());
    let (rows, _) = render(&frame, &UiState::default(), 80, 24);
    let text = text_of(&rows);
    let first_word = frame
        .declined
        .as_deref()
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap();
    assert_words(&text, &["—", first_word], &[], "packed declined");
    assert_snapshot("packed-declined-80x24", &rows);
}

#[test]
fn idle_draws_the_chrome_with_dashes() {
    let model = idle_model();
    let (rows, _) = render(&model.frame(), &UiState::default(), 80, 24);
    let text = text_of(&rows);
    assert_words(&text, &CHROME, &OFF_SCREEN, "packed idle");
    assert!(text.contains("STATE: IDLE"), "{text}");
    assert_snapshot("packed-idle-80x24", &rows);
}

#[test]
fn stepping_into_history_highlights_the_row_and_names_the_step() {
    let mut model = rich_model();
    model.step_history_back();
    model.step_history_back();
    let frame = model.frame();
    let step = frame.history_step.expect("stepped");
    let (rows, _) = render(&frame, &UiState::default(), 80, 30);
    let text = text_of(&rows);
    assert!(
        text.contains(&format!("HISTORY {}/{}", step.index, step.total)),
        "{text}"
    );
    let stepped = &frame.history[step.index - 1];
    assert!(text.contains(&stepped.name), "{text}");
    assert_snapshot("packed-stepped-80x30", &rows);
}

#[test]
fn a_selected_note_shows_edit_and_del_and_a_del_hit() {
    let model = fake_model(false);
    let ui = UiState {
        selected_now_playing: Some(1),
        ..UiState::default()
    };
    let (rows, hits) = render(&model.frame(), &ui, 80, 28);
    let text = text_of(&rows);
    assert_words(&text, &["EDIT[E]", "DEL[D]"], &[], "packed selected");
    assert_eq!(hits.delete_cells.len(), 1);
    assert_eq!(hits.delete_cells[0].1, 1);
    let (rect, _) = hits.delete_cells[0];
    let row = &rows[rect.y as usize];
    let cell: String = row
        .chars()
        .skip(rect.x as usize)
        .take(rect.width as usize)
        .collect();
    assert_eq!(cell, "DEL[D]");
    assert_snapshot("packed-selected-80x28", &rows);
}

#[test]
fn a_draft_shows_in_the_field_and_places_the_cursor_after_it() {
    let mut model = fake_model(false);
    model.draft_note_text = "voicing with the 9".to_string();
    let frame = model.frame();
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("a test terminal");
    terminal
        .draw(|target| {
            packed::draw(target, &frame, &UiState::default());
        })
        .expect("a frame");
    let (rows, hits) = render(&frame, &UiState::default(), 80, 24);
    let text = text_of(&rows);
    assert!(text.contains("voicing with the 9"), "{text}");
    assert!(!text.contains("add a note"), "{text}");
    let field = hits.field.expect("a field");
    let row = &rows[field.y as usize];
    assert!(row.contains("voicing with the 9"));
}

#[test]
fn below_80x24_one_line_says_so() {
    let model = fake_model(false);
    for (w, h) in [(79, 24), (80, 23)] {
        let (rows, hits) = render(&model.frame(), &UiState::default(), w, h);
        let text = text_of(&rows);
        assert!(text.contains("watchord needs 80×24"), "{w}x{h}\n{text}");
        assert!(!text.contains("WATCHORD"), "{w}x{h}\n{text}");
        assert!(hits.field.is_none());
    }
}

#[test]
fn all_notes_draws_the_groups_in_the_packed_chrome() {
    let model = rich_model();
    let mut frame = model.frame();
    frame.screen = Screen::AllNotes;
    let (rows, hits) = render(&frame, &UiState::default(), 80, 24);
    let text = text_of(&rows);
    assert_words(
        &text,
        &[
            "WATCHORD",
            "NOTES",
            "SEARCH:",
            "SORT[S]:",
            "RECENT",
            "NOW PLAYING[N]",
            "CHORD",
            "NOTE",
            "WRITTEN AS",
            "WHEN",
            "C7#9",
            "SUSTAIN:",
        ],
        &OFF_SCREEN,
        "packed all notes",
    );
    // Every group heading that fits carries its note count.
    assert!(text.contains("NOTES"), "{text}");
    assert_eq!(hits.tabs.len(), 1);
    assert_eq!(hits.tabs[0].1, Screen::NowPlaying);
    assert!(hits.field.is_some());
    assert!(!hits.note_rows.is_empty());
    assert_snapshot("packed-all-notes-80x24", &mask_times(&rows));

    let (rows, _) = render(&frame, &UiState::default(), 120, 40);
    let text = text_of(&rows);
    for group in &frame.groups {
        assert!(text.contains(&group.heading), "{}\n{text}", group.heading);
    }
    assert_snapshot("packed-all-notes-120x40", &mask_times(&rows));
}

#[test]
fn all_notes_selection_shows_edit_and_del_and_search_filters() {
    let model = rich_model();
    let mut frame = model.frame();
    frame.screen = Screen::AllNotes;
    let ui = UiState {
        selected_all_notes: Some(0),
        ..UiState::default()
    };
    let (rows, hits) = render(&frame, &ui, 80, 24);
    let text = text_of(&rows);
    assert_words(
        &text,
        &["EDIT[E]", "DEL[D]"],
        &[],
        "packed all notes selected",
    );
    assert_eq!(hits.delete_cells.len(), 1);
    assert_eq!(hits.delete_cells[0].1, 0);

    // A typed search shows in the field.
    let mut model = rich_model();
    model.screen = Screen::AllNotes;
    model.search_text = "hendrix".to_string();
    let frame = model.frame();
    let (rows, _) = render(&frame, &UiState::default(), 80, 24);
    let text = text_of(&rows);
    assert!(text.contains("hendrix"), "{text}");
    assert!(!text.contains("type to search"), "{text}");
}

/// One blank row divides two sections: after the KEYS row, after the last
/// reading, after the last history entry. Nothing is underlined any more
/// (the earlier border), and the header rows are the control: not blank.
#[test]
fn a_blank_row_divides_two_sections_and_nothing_is_underlined() {
    use ratatui::style::Modifier;
    let model = fake_model(false);
    let frame = model.frame();
    let mut terminal = Terminal::new(TestBackend::new(80, 28)).expect("a test terminal");
    terminal
        .draw(|target| {
            packed::draw(target, &frame, &UiState::default());
        })
        .expect("a frame");
    let buffer = terminal.backend().buffer();
    let row_text = |y: u16| -> String { (0..80).map(|x| buffer[(x, y)].symbol()).collect() };
    let find = |needle: &str| -> u16 {
        (0..28)
            .find(|&y| row_text(y).contains(needle))
            .unwrap_or_else(|| panic!("{needle} row"))
    };
    // The left column, x 1..59, holds only spaces on a divider row.
    let blank = |y: u16| (1..59).all(|x| buffer[(x, y)].symbol() == " ");
    assert!(blank(find("KEYS:") + 1), "a blank row after KEYS");
    assert!(blank(find("READINGS") - 1), "a blank row before READINGS");
    assert!(blank(find("HISTORY") - 1), "a blank row before HISTORY");
    assert!(blank(find("NOTES") - 1), "a blank row before NOTES");
    for control in ["READINGS", "HISTORY", "NOTES", "KEYS:", "WATCHORD"] {
        assert!(!blank(find(control)), "{control} is not blank");
    }
    let underlined =
        (0..80).any(|x| (0..28).any(|y| buffer[(x, y)].modifier.contains(Modifier::UNDERLINED)));
    assert!(!underlined, "no cell is underlined");
}

/// `2026-08-28 13:46` -> `YYYY-MM-DD HH:MM`, as the other skins' tests mask it.
fn mask_times(rows: &[String]) -> Vec<String> {
    rows.iter()
        .map(|row| {
            let chars: Vec<char> = row.chars().collect();
            let mut out = String::new();
            let mut i = 0;
            while i < chars.len() {
                let is_stamp = i + 16 <= chars.len()
                    && "0000-00-00 00:00".chars().enumerate().all(|(k, pattern)| {
                        let c = chars[i + k];
                        if pattern == '0' {
                            c.is_ascii_digit()
                        } else {
                            c == pattern
                        }
                    });
                if is_stamp {
                    out.push_str("YYYY-MM-DD HH:MM");
                    i += 16;
                } else {
                    out.push(chars[i]);
                    i += 1;
                }
            }
            out
        })
        .collect()
}
