//! Each screen rendered through `TestBackend` at 80x24 and 120x40, on the
//! `--fake` state, asserting the headline, both plates, and the table heads
//! are in the buffer. The rendered frames are committed under
//! `tests/snapshots/` and compared byte for byte; a deliberate change to the
//! look is re-recorded with `WATCHORD_UPDATE_SNAPSHOTS=1`.
//!
//! The glyph-coverage test reads every headline and fit note the engine
//! fixture can emit and asserts each character has a figure.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::Style;
use serde::Deserialize;
use std::time::{Duration, UNIX_EPOCH};
use watchord_core::{ChordFit, ChordNote, SoundingSet, SpellingOrigin};
use watchord_model::fakes::{
    InMemoryNoteStore, ScriptedSoundingSetSource, StubAlternate, StubChordNaming,
};

use watchord_model::{AppModel, Screen};
use watchord_tui::{UiState, draw, figure};

/// The `--fake` graph: ticket 08's C6, two alternates, three notes across two
/// chords. Mirrors `Composition::demo` in the binary, which a library test
/// cannot reach.
fn fake_model(released: bool, screen: Screen) -> AppModel {
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
    let f_major = SoundingSet::new([53, 57, 60]);
    naming.stub(
        &f_major,
        StubChordNaming::naming(&f_major, "F", "F major", vec![]),
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
        ChordNote::new(
            "3",
            f_major.key(),
            "F",
            "plain, but it lands",
            at(1_787_003_000),
        ),
    ]);
    let mut model = AppModel::new(
        Arc::new(naming),
        Box::new(ScriptedSoundingSetSource::new(script)),
        Arc::new(store),
    );
    model.screen = screen;
    model.announce("fake input — no MIDI hardware is being read");
    model.start();
    // The scripted source pushes its whole script inside `start()`.
    while model.wait(Duration::from_millis(50)) {}
    model
}

/// `--fake-fit`: a `Missing` headline over a slash bass with a `Nearest`
/// alternate. Mirrors `Composition::fit_demo(false)`.
fn fit_model() -> AppModel {
    let c13 = SoundingSet::new([64, 72, 82, 86, 93]);
    let c13_tones: [i32; 7] = [0, 4, 7, 10, 2, 5, 9];
    let cm7_tones: [i32; 4] = [0, 3, 7, 10];
    let naming = StubChordNaming::new();
    naming.stub(
        &c13,
        StubChordNaming::naming_with_fit(
            &c13,
            "C13",
            "C major, minor 7, 13",
            ChordFit::Missing(vec![5, 11]),
            Some(&c13_tones),
            vec![
                StubAlternate::new("Cm7", SpellingOrigin::ReRooted, "C minor 7")
                    .with_fit(ChordFit::Nearest { distance: 4 }, &cm7_tones),
                StubAlternate::new("Am7", SpellingOrigin::ReRooted, "A minor 7"),
            ],
        ),
    );
    let mut model = AppModel::new(
        Arc::new(naming),
        Box::new(ScriptedSoundingSetSource::new(vec![c13])),
        Arc::new(InMemoryNoteStore::default()),
    );
    model.announce("fake input — fit tiers are stubbed, no MIDI hardware is being read");
    model.start();
    while model.wait(Duration::from_millis(50)) {}
    model
}

/// A fixed instant, so the `when` column is the same on every run.
fn at(unix: u64) -> std::time::SystemTime {
    UNIX_EPOCH + Duration::from_secs(unix)
}

/// Renders one frame and returns its rows as plain text, trailing spaces cut.
fn render(model: &AppModel, width: u16, height: u16) -> Vec<String> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("a test terminal");
    let ui = UiState::default();
    terminal
        .draw(|frame| draw(frame, model, &ui))
        .expect("a frame");
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

fn text_of(rows: &[String]) -> String {
    rows.join("\n")
}

/// The figure lines of `text` as plain strings, for finding in a frame.
fn figure_rows(text: &str, tall: bool) -> Vec<String> {
    let lines = if tall {
        figure::tall(text, Style::default())
    } else {
        figure::short(text, Style::default())
    };
    lines
        .iter()
        .map(|line| line.to_string().trim_end().to_string())
        .collect()
}

fn assert_figure_present(rows: &[String], headline: &str, tall: bool) {
    let wanted = figure_rows(headline, tall);
    let found = rows.windows(wanted.len()).any(|window| {
        window.iter().zip(&wanted).all(|(row, want)| {
            row.trim_end().ends_with(want.trim_end()) || row.contains(want.trim_end())
        })
    });
    assert!(
        found,
        "the {} figure of {headline:?} is not in the frame:\n{}",
        if tall { "tall" } else { "short" },
        text_of(rows)
    );
}

/// `2026-08-28 13:46` -> `YYYY-MM-DD HH:MM`.
fn mask_times(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
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
}

fn snapshot_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots")
        .join(format!("{name}.txt"))
}

/// Compares the frame to its committed snapshot, or records it.
fn assert_snapshot(name: &str, rows: &[String]) {
    let path = snapshot_path(name);
    // The `when` column is local time, which differs by machine; it is masked
    // so the snapshot is about the layout and not the zone.
    let actual = mask_times(&text_of(rows)) + "\n";
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

const SIZES: [(u16, u16); 2] = [(80, 24), (120, 40)];

#[test]
fn now_playing_shows_the_headline_both_plates_and_the_table_heads() {
    for (width, height) in SIZES {
        let model = fake_model(false, Screen::NowPlaying);
        let rows = render(&model, width, height);
        let text = text_of(&rows);
        assert_figure_present(&rows, "C6", height >= 34);
        assert!(
            text.contains("INPUT"),
            "no INPUT plate at {width}x{height}:\n{text}"
        );
        assert!(
            text.contains("FAKE"),
            "no input label at {width}x{height}:\n{text}"
        );
        assert!(
            text.contains("STATE"),
            "no STATE plate at {width}x{height}:\n{text}"
        );
        assert!(
            text.contains("HELD"),
            "no state label at {width}x{height}:\n{text}"
        );
        for head in ["READING", "AXIS", "FIT", "SAID ALOUD", "NOTE"] {
            assert!(
                text.contains(head),
                "no {head} head at {width}x{height}:\n{text}"
            );
        }
        assert!(text.contains("C major 6"), "no spoken name:\n{text}");
        assert!(text.contains("C3  E3  G3  A3"), "no keys row:\n{text}");
        assert_snapshot(&format!("now-playing-{width}x{height}"), &rows);
    }
}

#[test]
fn released_shows_the_plate_in_the_display_and_on_the_band() {
    for (width, height) in SIZES {
        let model = fake_model(true, Screen::NowPlaying);
        let rows = render(&model, width, height);
        let text = text_of(&rows);
        assert_figure_present(&rows, "C6", height >= 34);
        assert_eq!(
            text.matches("RELEASED").count(),
            2,
            "one on the band, one in the display at {width}x{height}:\n{text}"
        );
        assert_snapshot(&format!("released-{width}x{height}"), &rows);
    }
}

#[test]
fn fit_shows_the_detail_under_the_headline_and_the_mark_beside_an_alternate() {
    for (width, height) in SIZES {
        let model = fit_model();
        let rows = render(&model, width, height);
        let text = text_of(&rows);
        assert_figure_present(&rows, "C13/E", height >= 34);
        assert!(
            text.contains("·NO5 ·NO11"),
            "no fit detail at {width}x{height}:\n{text}"
        );
        assert!(
            text.contains("Cm7/E ≈"),
            "no ≈ beside the alternate:\n{text}"
        );
        assert_snapshot(&format!("fit-{width}x{height}"), &rows);
    }
}

#[test]
fn all_notes_shows_the_groups_the_keys_and_the_heads() {
    for (width, height) in SIZES {
        let model = fake_model(false, Screen::AllNotes);
        let rows = render(&model, width, height);
        let text = text_of(&rows);
        for head in ["NOTE", "WRITTEN AS", "WHEN"] {
            assert!(
                text.contains(head),
                "no {head} head at {width}x{height}:\n{text}"
            );
        }
        assert!(
            text.contains("INPUT") && text.contains("STATE"),
            "plates:\n{text}"
        );
        assert!(text.contains("0.4.7.9"), "no chord key subhead:\n{text}");
        assert!(
            text.contains("C6") && text.contains("F"),
            "no group gutters:\n{text}"
        );
        assert!(text.contains("3 NOTES"), "no folio:\n{text}");
        assert_snapshot(&format!("all-notes-{width}x{height}"), &rows);
    }
}

#[test]
fn nothing_played_yet_says_so() {
    let model = fake_model(false, Screen::NowPlaying);
    // A fresh model with no script: the waiting state.
    let mut idle = AppModel::new(
        Arc::new(StubChordNaming::new()),
        Box::new(ScriptedSoundingSetSource::new(vec![])),
        Arc::new(InMemoryNoteStore::default()),
    );
    idle.start();
    let rows = render(&idle, 80, 24);
    let text = text_of(&rows);
    assert!(text.contains("play something"), "{text}");
    assert!(text.contains("IDLE"), "{text}");
    assert!(text.contains("NONE"), "no input reads NONE:\n{text}");
    assert!(
        text.contains("play a chord to write a note against it"),
        "{text}"
    );
    drop(model);
}

// MARK: - Glyph coverage

#[derive(Deserialize)]
struct Fixture {
    sets: Vec<Row>,
    voiced: Vec<Row>,
}

#[derive(Deserialize)]
struct Row {
    headline: Headline,
    alternates: Vec<Alternate>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Headline {
    text: String,
    fit_note: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Alternate {
    text: String,
    fit_note: Option<String>,
}

#[test]
fn every_character_the_fixture_can_put_in_a_headline_has_a_glyph() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/engine-fixture.json");
    let fixture: Fixture =
        serde_json::from_str(&fs::read_to_string(&path).expect("the engine fixture"))
            .expect("fixture parses");
    let mut alphabet = BTreeSet::new();
    for row in fixture.sets.iter().chain(&fixture.voiced) {
        alphabet.extend(row.headline.text.chars());
        alphabet.extend(row.headline.fit_note.iter().flat_map(|s| s.chars()));
        for alternate in &row.alternates {
            alphabet.extend(alternate.text.chars());
            alphabet.extend(alternate.fit_note.iter().flat_map(|s| s.chars()));
        }
    }
    // The mark the display adds beside a nearest reading, and the space the
    // figure puts before it.
    alphabet.insert('≈');
    alphabet.insert(' ');
    assert!(
        alphabet.len() > 20,
        "the fixture alphabet is suspiciously small: {alphabet:?}"
    );
    let missing: Vec<char> = alphabet
        .iter()
        .copied()
        .filter(|&c| figure::glyph(c).is_none())
        .collect();
    assert!(missing.is_empty(), "no glyph for {missing:?}");
}

#[test]
fn a_character_outside_the_alphabet_is_reported_missing() {
    // The control: the coverage test above cannot pass on a glyph set that
    // answers every character.
    assert_eq!(figure::missing("C6?"), vec!['?']);
}
