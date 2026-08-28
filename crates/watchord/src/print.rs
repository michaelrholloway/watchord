//! `--print`: the model, headless, as plain lines.
//!
//! Prints everything the Now Playing screen would draw, one fact per line, and
//! prints it again whenever the display changes. A fake's script is finite, so
//! a fake run prints once it has been consumed and exits; a live run streams
//! until Ctrl-C.

use std::io::{self, Write};
use std::process::ExitCode;
use std::time::Duration;

use watchord_model::AppModel;

use crate::composition::GraphKinds;

/// One tick of the headless loop.
const TICK: Duration = Duration::from_millis(50);

/// Quiet ticks after a fake's last event before its output is final.
const QUIET_TICKS_BEFORE_DONE: u32 = 4;

pub fn run(mut model: AppModel, kinds: &GraphKinds, is_fake: bool) -> ExitCode {
    model.start();
    println!(
        "graph: naming={} source={} store={}",
        kinds.naming, kinds.source, kinds.store
    );
    let mut out = io::stdout().lock();
    let mut last = String::new();

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
        let _ = out.write_all(render(&model).as_bytes());
        model.stop();
        return ExitCode::SUCCESS;
    }

    let _ = out.write_all(render(&model).as_bytes());
    loop {
        model.wait(Duration::from_secs(1));
        let now = render(&model);
        if now != last {
            let _ = out.write_all(b"\n");
            let _ = out.write_all(now.as_bytes());
            let _ = out.flush();
            last = now;
        }
    }
}

/// Every fact on the screen, as `label: value` lines.
pub fn render(model: &AppModel) -> String {
    let mut lines = Vec::new();
    if let Some(banner) = model.banner() {
        lines.push(format!("banner: {banner}"));
    }
    if let Some(status) = model.status_message() {
        lines.push(format!("status: {status}"));
    }
    lines.push(format!("input: {}", model.input_label()));
    lines.push(format!("screen: {}", model.screen.title()));
    lines.push(format!("headline: {}", model.headline_text()));
    if let Some(approximation) = model.headline_approximation() {
        lines.push(format!("approximation: {approximation}"));
    }
    if let Some(spoken) = model.headline_spoken() {
        lines.push(format!("spoken: {spoken}"));
    }
    if let Some(fit) = model.headline_fit_note() {
        lines.push(format!("fit: {fit}"));
    }
    if let Some(reason) = model.decline_reason() {
        lines.push(format!("declined: {reason}"));
    }
    lines.push(format!(
        "state: {}",
        if model.is_released() {
            "released"
        } else {
            "held"
        }
    ));
    let keys = model.keys_row();
    if !keys.is_empty() {
        lines.push(format!("keys: {keys}"));
    }
    for alternate in model.alternates() {
        let mark = alternate.approximation.as_deref().unwrap_or("");
        let detail = alternate.fit_detail.as_deref().unwrap_or("");
        lines.push(
            format!(
                "alternate: {}{} [{}] {} {}",
                alternate.name,
                if mark.is_empty() {
                    String::new()
                } else {
                    format!(" {mark}")
                },
                alternate.origin.raw_value(),
                alternate.spoken,
                detail
            )
            .trim_end()
            .to_string(),
        );
    }
    for note in model.notes_for_displayed_chord() {
        lines.push(format!(
            "note: {} (written as {})",
            note.text, note.spelling_when_written
        ));
    }
    lines.push(format!("notes total: {}", model.total_note_count()));
    lines.push(String::new());
    lines.join("\n")
}
