//! `--print` and `--json`: the model, headless.
//!
//! Both print the same [`Frame`] the skins draw. `--print` writes every field
//! as a labelled plain line, absent optional fields as `label: —`, so a script
//! sees the same labels every time. `--json` writes the frame as one JSON line
//! per settled sounding set — pipe mode — for other programs.
//!
//! A fake's script is finite, so a fake run prints once it has been consumed
//! and exits; a live run streams until Ctrl-C.

use std::io::{self, Write};
use std::process::ExitCode;
use std::time::Duration;

use watchord_model::notes::tags_of;
use watchord_model::{AppModel, Frame};
use watchord_tui::when;

use crate::composition::GraphKinds;

/// One tick of the headless loop.
const TICK: Duration = Duration::from_millis(50);

/// Quiet ticks after a fake's last event before its output is final.
const QUIET_TICKS_BEFORE_DONE: u32 = 4;

/// The value printed for an absent optional field.
const ABSENT: &str = "—";

/// How the frame goes to stdout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Output {
    /// Labelled plain lines, reprinted whenever the display changes.
    Lines,
    /// One JSON line per settled sounding set.
    Json,
}

pub fn run(mut model: AppModel, kinds: &GraphKinds, is_fake: bool, output: Output) -> ExitCode {
    model.start();
    let graph = format!(
        "graph: naming={} source={} store={}",
        kinds.naming, kinds.source, kinds.store
    );
    match output {
        // The pipe is JSON lines only; the graph note goes beside it.
        Output::Json => eprintln!("{graph}"),
        Output::Lines => println!("{graph}"),
    }
    let mut out = io::stdout().lock();

    if is_fake {
        // Drain the whole script. A scripted source pushes it all inside
        // `start()`, so once the channel has been quiet for a few ticks after
        // the first event, everything it will ever say has arrived.
        let mut seen_any = false;
        let mut quiet_ticks = 0;
        while quiet_ticks < QUIET_TICKS_BEFORE_DONE {
            if model.wait(TICK) {
                seen_any = true;
                quiet_ticks = 0;
            } else if seen_any {
                quiet_ticks += 1;
            }
        }
        let _ = out.write_all(format_frame(&model.frame(), output).as_bytes());
        model.stop();
        return ExitCode::SUCCESS;
    }

    match output {
        Output::Lines => {
            let mut last = render(&model.frame());
            let _ = out.write_all(last.as_bytes());
            loop {
                model.wait(Duration::from_secs(1));
                let now = render(&model.frame());
                if now != last {
                    let _ = out.write_all(b"\n");
                    let _ = out.write_all(now.as_bytes());
                    let _ = out.flush();
                    last = now;
                }
            }
        }
        Output::Json => {
            // One line per settled sounding set: a line goes out when the
            // sounding set or the held/released state moves, never for a
            // keystroke or a note.
            let mut last = model.frame();
            loop {
                model.wait(Duration::from_secs(1));
                let now = model.frame();
                if now.sounding != last.sounding || now.state != last.state {
                    let _ = out.write_all(json_line(&now).as_bytes());
                    let _ = out.flush();
                    last = now;
                }
            }
        }
    }
}

fn format_frame(frame: &Frame, output: Output) -> String {
    match output {
        Output::Lines => render(frame),
        Output::Json => json_line(frame),
    }
}

/// The frame as one JSON line, newline-terminated.
pub fn json_line(frame: &Frame) -> String {
    // A `Frame` is plain data with no map keys that can fail to serialise.
    let mut line = serde_json::to_string(frame).expect("a Frame serialises");
    line.push('\n');
    line
}

/// Every `watchord-theory` annotation as its own labelled line — inversion,
/// slash, voicing (shape/span/rootless/doublings), upper structure, and the
/// staff, one line per clef. Ticket #11.
fn annotation_lines(frame: &Frame) -> Vec<String> {
    let a = &frame.annotations;
    let mut lines = Vec::new();
    lines.push(format!(
        "inversion: {}",
        a.inversion.map(|i| i.label()).unwrap_or(ABSENT)
    ));
    lines.push(format!("slash: {}", or_absent(a.slash.as_deref())));
    match &a.voicing {
        Some(v) => {
            lines.push(format!("voicing: {}", v.shape.label()));
            lines.push(format!("span: {}", v.span));
            lines.push(format!(
                "rootless: {}",
                if v.rootless { "yes" } else { "no" }
            ));
            let doublings = if v.doublings.is_empty() {
                ABSENT.to_string()
            } else {
                v.doublings
                    .iter()
                    .map(|d| {
                        format!(
                            "{} x{}",
                            watchord_core::NoteName::pitch_class(d.pitch_class, false),
                            d.count
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            lines.push(format!("doublings: {doublings}"));
        }
        None => {
            lines.push(format!("voicing: {ABSENT}"));
            lines.push(format!("span: {ABSENT}"));
            lines.push(format!("rootless: {ABSENT}"));
            lines.push(format!("doublings: {ABSENT}"));
        }
    }
    if a.upper_structures.is_empty() {
        lines.push(format!("upper structure: {ABSENT}"));
    } else {
        for structure in &a.upper_structures {
            lines.push(format!("upper structure: {structure}"));
        }
    }
    for (label, clef) in [
        ("treble", watchord_theory::staff::Clef::Treble),
        ("bass", watchord_theory::staff::Clef::Bass),
    ] {
        let notes: Vec<_> = a.staff.iter().filter(|n| n.clef == clef).collect();
        let value = if notes.is_empty() {
            ABSENT.to_string()
        } else {
            notes
                .iter()
                .map(|n| {
                    format!(
                        "{}{}={}",
                        n.spelled.letter,
                        n.spelled.accidental.symbol(),
                        watchord_core::NoteName::note(n.midi_note)
                    )
                })
                .collect::<Vec<_>>()
                .join(" ")
        };
        lines.push(format!("{label}: {value}"));
    }
    lines
}

fn or_absent(value: Option<&str>) -> &str {
    match value {
        Some(text) if !text.is_empty() => text,
        _ => ABSENT,
    }
}

/// A pedal plate's word: `down` or `up`.
fn pedal_word(down: bool) -> &'static str {
    if down { "down" } else { "up" }
}

/// A mode plate's word: `on` or `off`.
fn mode_word(on: bool) -> &'static str {
    if on { "on" } else { "off" }
}

/// A note's text on one labelled line: an embedded line break would otherwise
/// read as a second, unlabelled line.
fn single_line(text: &str) -> String {
    text.replace('\n', " ⏎ ")
}

/// Every field of the frame, as `label: value` lines.
pub fn render(frame: &Frame) -> String {
    let mut lines = Vec::new();
    lines.push(format!("banner: {}", or_absent(frame.banner.as_deref())));
    lines.push(format!("status: {}", or_absent(frame.status.as_deref())));
    lines.push(format!("input: {}", frame.input));
    lines.push(format!(
        "inputs: {}",
        if frame.inputs.is_empty() {
            ABSENT.to_string()
        } else {
            frame.inputs.join(", ")
        }
    ));
    lines.push(format!("screen: {}", frame.screen.title()));
    lines.push(format!("state: {}", frame.state.label()));
    lines.push(format!("sustain: {}", pedal_word(frame.sustain)));
    lines.push(format!("sostenuto: {}", pedal_word(frame.sostenuto)));
    lines.push(format!("soft: {}", pedal_word(frame.soft)));
    lines.push(format!("settle: {} ms", frame.settle_ms));
    lines.push(format!("arpeggio: {}", mode_word(frame.arpeggio)));
    lines.push(format!("headline: {}", frame.headline_text));
    lines.push(format!(
        "approximation: {}",
        or_absent(frame.headline_approximation())
    ));
    lines.push(format!("spoken: {}", or_absent(frame.headline_spoken())));
    lines.push(format!("fit: {}", or_absent(frame.headline_fit_note())));
    lines.push(format!(
        "declined: {}",
        or_absent(frame.declined.as_deref())
    ));
    lines.push(format!("keys: {}", or_absent(Some(frame.keys.as_str()))));
    lines.push(format!("key: {}", or_absent(Some(frame.key.raw()))));
    lines.push(format!(
        "sounding: {}",
        or_absent(Some(frame.sounding_row().as_str()))
    ));
    for (index, reading) in frame.readings().iter().enumerate() {
        let label = if index == 0 { "reading" } else { "alternate" };
        let mark = reading
            .approximation
            .as_deref()
            .map(|m| format!(" {m}"))
            .unwrap_or_default();
        let detail = reading
            .fit_detail
            .as_deref()
            .map(|d| format!(" {d}"))
            .unwrap_or_default();
        lines.push(format!(
            "{label}: {}{mark} · rank {} · origin {} · fit {}{detail} · score {} · root {} · claimed {} · spoken {}",
            reading.name,
            index + 1,
            reading.origin.raw_value(),
            reading.fit_name(),
            reading.score,
            reading.root_name(),
            reading.claimed_row(),
            reading.spoken,
        ));
    }
    lines.push(format!(
        "annotations: {}",
        if frame.annotations.is_empty() {
            "none yet"
        } else {
            ""
        }
    ));
    lines.extend(annotation_lines(frame));
    for note in &frame.notes {
        lines.push(format!(
            "note: {} (written as {})",
            single_line(&note.text),
            note.spelling_when_written
        ));
    }
    lines.push(format!("notes total: {}", frame.notes_total));
    lines.push(format!("draft: {}", or_absent(Some(frame.draft.as_str()))));
    lines.push(format!(
        "editing: {}",
        if frame.editing { "yes" } else { ABSENT }
    ));
    lines.push(format!(
        "search: {}",
        or_absent(if frame.search.is_empty() {
            None
        } else {
            Some(frame.search.as_str())
        })
    ));
    lines.push(format!("sort: {}", frame.notes_sort.label()));
    for group in &frame.groups {
        let tags = tags_of(group);
        let tags_text = if tags.is_empty() {
            ABSENT.to_string()
        } else {
            tags.iter()
                .map(|t| format!("#{t}"))
                .collect::<Vec<_>>()
                .join(" ")
        };
        lines.push(format!(
            "group: {} · key {} · notes {} · tags {}",
            group.heading,
            group.key.raw(),
            group.notes.len(),
            tags_text,
        ));
        for note in &group.notes {
            lines.push(format!(
                "group note: {} (written as {})",
                single_line(&note.text),
                note.spelling_when_written
            ));
        }
    }
    drill_lines(&frame.drill, &mut lines);
    lines.push(String::new());
    lines.join("\n")
}

/// Drill's own lines (spec #9, ticket #14). Always emitted, even when drill
/// is off, so `--print`'s labels never depend on whether drill has been used
/// — the same discipline every other optional field on the frame keeps.
fn drill_lines(drill: &watchord_model::DrillFrame, lines: &mut Vec<String>) {
    lines.push(format!(
        "drill: {}",
        if drill.active { "on" } else { "off" }
    ));
    lines.push(format!("target: {}", or_absent(drill.target.as_deref())));
    lines.push(format!(
        "next target: {}",
        or_absent(drill.next_target.as_deref())
    ));
    lines.push(format!("grade: {}", or_absent(drill.grade_display())));
    lines.push(format!("drill stats: {}", drill.stats.len()));
    for row in &drill.stats {
        let last = row
            .last_at
            .map(when::format)
            .unwrap_or_else(|| ABSENT.to_string());
        lines.push(format!(
            "drill stat: {} · attempts {} · exact {} · last {last}",
            row.chord, row.attempts, row.exact
        ));
    }
}
