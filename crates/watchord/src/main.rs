//! The `watchord` binary.
//!
//! Builds the graph at the composition root and hands the model to the skin.
//! `--print` runs the model headless instead and prints what it shows, so the
//! model can be checked without a terminal.

mod composition;
mod print;

use std::process::ExitCode;

use composition::{Composition, LaunchArgs};

const USAGE: &str = "\
watchord — watches MIDI and names the chord being played

usage: watchord [--input <name>] [--print]
       watchord --fake | --fake-released | --fake-fit | --fake-nearest [--all-notes] [--print]

  --input <name>    listen only to inputs whose name contains <name>
  --print           run headless: print the headline, spoken name, fit note
                    and alternates as plain lines (a fake exits; live streams
                    until Ctrl-C)
  --fake            a scripted C6, no hardware
  --fake-released   the same, keys lifted
  --fake-fit        a C13 with a missing fifth and eleventh, over E
  --fake-nearest    the same keys with a nearest-fit headline
  --all-notes       open on the All Notes screen
  --version, -V     print the version
  --help, -h        this";

fn main() -> ExitCode {
    let (args, unknown) = LaunchArgs::parse(std::env::args().skip(1));
    if !unknown.is_empty() {
        eprintln!("watchord: unknown argument(s): {}", unknown.join(", "));
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    if args.help {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if args.version {
        println!("watchord {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let is_fake = args.fake || args.fake_released || args.fake_fit || args.fake_nearest;
    let graph = Composition::for_launch(&args);
    let kinds = graph.kinds.clone();
    if args.print {
        return print::run(graph.make_model(), &kinds, is_fake);
    }
    match watchord_tui::run(graph.make_model()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            watchord_tui::restore();
            eprintln!("watchord: {error}");
            ExitCode::FAILURE
        }
    }
}
