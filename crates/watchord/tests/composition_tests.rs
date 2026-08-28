//! The four `CompositionTests` from note-view's `AppModelTests.swift`, on the
//! binary's composition root. Run through the binary because the root is
//! private to it: each case launches `watchord` with `--print`.
//!
//! Neither `MidiSource::new` nor `JsonNotesStore::real` does any I/O at
//! construction, but `--print` on the live graph would open MIDI and stream, so
//! the live-graph case checks the flag parse and the fake control only.

use std::process::Command;

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
