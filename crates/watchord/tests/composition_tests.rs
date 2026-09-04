//! The four `CompositionTests` from note-view's `AppModelTests.swift`, on the
//! binary's composition root. Run through the binary because the root is
//! private to it: each case launches `watchord` with `--print`.
//!
//! Neither `MidiSource::new` nor `JsonNotesStore::real` does any I/O at
//! construction, but `--print` on the live graph would open MIDI and stream, so
//! the live-graph case checks the flag parse and the fake control only.

use std::process::Command;

use watchord_model::Frame;

/// A label no renderer emits. The grep must miss it, or the grep is not a test.
const CONTROL_LABEL: &str = "ZZZ_NOT_A_FIELD";

fn watchord(args: &[&str]) -> (String, String, bool) {
    let output = Command::new(env!("CARGO_BIN_EXE_watchord"))
        .args(args)
        .output()
        .expect("the binary runs");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.success(),
    )
}

#[test]
fn fake_graph_is_fake_and_says_so_on_screen() {
    let (out, _, ok) = watchord(&["--fake", "--print"]);
    assert!(ok);
    assert!(out.contains("banner: fake input"), "{out}");
    assert!(out.contains("input: fake"), "{out}");
    assert!(out.contains("headline: C6"), "{out}");
}

#[test]
fn all_notes_opens_on_the_other_screen() {
    let (out, _, _) = watchord(&["--fake", "--all-notes", "--print"]);
    assert!(out.contains("screen: All Notes"), "{out}");
    let (out, _, _) = watchord(&["--fake", "--print"]);
    assert!(out.contains("screen: Now Playing"), "{out}");
}

#[test]
fn the_demo_graph_draws_the_tickets_worked_example() {
    let (out, _, ok) = watchord(&["--fake-released", "--print"]);
    assert!(ok);
    assert!(out.contains("headline: C6"), "{out}");
    assert!(out.contains("keys: C3  E3  G3  A3"), "{out}");
    assert!(out.contains("state: released"), "{out}");
    assert_eq!(out.matches("\nnote: ").count(), 2, "{out}");
    assert!(out.contains("notes total: 3"), "{out}");
}

#[test]
fn the_fit_demos_put_each_tier_where_it_belongs() {
    let (out, _, _) = watchord(&["--fake-fit", "--print"]);
    assert!(out.contains("headline: C13/E"), "{out}");
    assert!(out.contains("fit: ·no5 ·no11"), "{out}");
    assert!(out.contains("alternate: Cm7/E ≈"), "{out}");

    let (out, _, _) = watchord(&["--fake-nearest", "--print"]);
    assert!(out.contains("headline: C7/E"), "{out}");
    assert!(out.contains("approximation: ≈"), "{out}");
}

#[test]
fn an_unknown_flag_is_refused_before_anything_starts() {
    let (_, err, ok) = watchord(&["--nope"]);
    assert!(!ok);
    assert!(err.contains("unknown argument"), "{err}");
}

#[test]
fn version_prints_the_crate_version() {
    let (out, _, ok) = watchord(&["--version"]);
    assert!(ok);
    assert_eq!(
        out.trim(),
        format!("watchord {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn print_lists_every_frame_label_and_not_the_control() {
    let (out, _, ok) = watchord(&["--fake", "--print"]);
    assert!(ok);
    for label in Frame::LABELS {
        assert!(out.contains(label), "no {label:?} label in --print:\n{out}");
    }
    assert!(
        !out.contains(CONTROL_LABEL),
        "the control label is present, so the grep proves nothing:\n{out}"
    );
    // Each reading carries the facts it was ranked on.
    assert!(out.contains("reading: C6 · rank 1 · origin headline · fit exact · score 100 · root C · claimed C E G A · spoken C major 6"), "{out}");
    assert!(out.contains("alternate: Am7/C · rank 2 · origin reRooted · fit exact · score 90 · root A · claimed C E G A"), "{out}");
    // An absent optional field keeps its label.
    assert!(out.contains("\nstatus: —\n"), "{out}");
    assert!(out.contains("\ndeclined: —\n"), "{out}");
    assert!(out.contains("\nsounding: 60 64 67 69\n"), "{out}");
    assert!(out.contains("\nkey: 0.4.7.9\n"), "{out}");
    assert!(
        out.contains("\ngroup: C6 · key 0.4.7.9 · notes 2 · tags —\n"),
        "{out}"
    );
    // Ticket #15: search, sort, tags, and editing state carry through --print.
    assert!(out.contains("\nsearch: —\n"), "{out}");
    assert!(out.contains("\nsort: recent\n"), "{out}");
    assert!(out.contains("\nediting: —\n"), "{out}");
}

#[test]
fn json_prints_one_line_that_parses_back_into_an_equal_frame() {
    let (out, err, ok) = watchord(&["--fake", "--json"]);
    assert!(ok);
    assert!(
        err.contains("graph:"),
        "the graph note goes to stderr:\n{err}"
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 1, "one line per settled sounding set:\n{out}");
    let frame: Frame = serde_json::from_str(lines[0]).expect("the line is a Frame");
    assert_eq!(frame.headline_text, "C6");
    assert_eq!(frame.sounding.midi_notes(), [60, 64, 67, 69]);
    assert_eq!(frame.alternates.len(), 2);
    assert_eq!(frame.notes.len(), 2);
    // Equal after a second round: the value is stable, not merely parseable.
    let again = serde_json::to_string(&frame).expect("serialises");
    assert_eq!(again, lines[0]);
    let back: Frame = serde_json::from_str(&again).expect("parses again");
    assert_eq!(back, frame);
}

#[test]
fn the_plain_skin_is_a_launch_choice() {
    // The skin only changes how the frame is drawn; headless output is the same.
    let (plain, _, ok) = watchord(&["--fake", "--skin", "plain", "--print"]);
    assert!(ok);
    let (push, _, _) = watchord(&["--fake", "--print"]);
    assert_eq!(plain, push);
    let (_, err, ok) = watchord(&["--fake", "--skin", "neon", "--print"]);
    assert!(!ok);
    assert!(err.contains("neon"), "{err}");
}
