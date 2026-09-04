//! **The composition root.** The only place in the app where the object graph
//! is assembled, and the only place that names a concrete type from
//! `watchord-engine`, `watchord-midi` or `watchord-store`.
//!
//! Ported from note-view's `Composition.swift`. Everything else sees the three
//! seams in `watchord-core` and nothing else.

use std::any::type_name;
use std::sync::Arc;

use watchord_core::{
    ChordFit, ChordNaming, ChordNote, ChordVocabulary, DrillStoring, NoteStoring, SoundingSet,
    SoundingSetSource, SpellingOrigin,
};
use watchord_engine::{EngineVocabulary, NamingEngine};
use watchord_midi::MidiSource;
use watchord_model::fakes::{
    FixedVocabulary, InMemoryDrillStore, InMemoryNoteStore, ScriptedSoundingSetSource,
    StubAlternate, StubChordNaming, rich_demo as build_rich_demo,
};
use watchord_model::{AppModel, Screen, seconds_ago};
use watchord_store::{JsonDrillStore, JsonNotesStore};
use watchord_tui::Skin;

/// What the command line asked for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LaunchArgs {
    pub fake: bool,
    pub fake_released: bool,
    pub fake_fit: bool,
    pub fake_nearest: bool,
    /// A scripted nine-chord session with every plain-skin region populated
    /// — for a design screenshot, never a claim about the real engine.
    pub fake_rich: bool,
    pub all_notes: bool,
    /// `--input <name>`: only inputs whose name contains this.
    pub input: Option<String>,
    /// Run the model headless and print what it shows, instead of drawing.
    pub print: bool,
    /// Run the model headless and stream one JSON line per settled sounding
    /// set — pipe mode.
    pub json: bool,
    /// `--skin push|plain`. PUSH unless asked.
    pub skin: Skin,
    pub version: bool,
    pub help: bool,
}

impl LaunchArgs {
    /// Parses the arguments after the program name. Unknown flags are kept as
    /// `unknown`, so the caller can refuse them in one place.
    pub fn parse(args: impl IntoIterator<Item = String>) -> (Self, Vec<String>) {
        let mut parsed = LaunchArgs::default();
        let mut unknown = Vec::new();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--fake" => parsed.fake = true,
                "--fake-released" => parsed.fake_released = true,
                "--fake-fit" => parsed.fake_fit = true,
                "--fake-nearest" => parsed.fake_nearest = true,
                "--fake-rich" => parsed.fake_rich = true,
                "--all-notes" => parsed.all_notes = true,
                "--print" => parsed.print = true,
                "--json" => parsed.json = true,
                "--skin" => match args.next() {
                    Some(name) => Self::set_skin(&mut parsed, &name, &mut unknown),
                    None => unknown.push("--skin needs a name: push or plain".to_string()),
                },
                "--version" | "-V" => parsed.version = true,
                "--help" | "-h" => parsed.help = true,
                "--input" => match args.next() {
                    Some(name) => parsed.input = Some(name),
                    None => unknown.push("--input needs a name".to_string()),
                },
                other => {
                    if let Some(name) = other.strip_prefix("--input=") {
                        parsed.input = Some(name.to_string());
                    } else if let Some(name) = other.strip_prefix("--skin=") {
                        Self::set_skin(&mut parsed, name, &mut unknown);
                    } else {
                        unknown.push(other.to_string());
                    }
                }
            }
        }
        (parsed, unknown)
    }

    fn set_skin(parsed: &mut LaunchArgs, name: &str, unknown: &mut Vec<String>) {
        match Skin::parse(name) {
            Some(skin) => parsed.skin = skin,
            None => unknown.push(format!(
                "--skin {name:?} is not one of {}",
                Skin::NAMES.join(", ")
            )),
        }
    }
}

pub struct Composition {
    pub naming: Arc<dyn ChordNaming>,
    pub source: Box<dyn SoundingSetSource>,
    pub store: Arc<dyn NoteStoring>,
    /// The engine's naming vocabulary, for drill (ticket #14).
    pub vocabulary: Arc<dyn ChordVocabulary>,
    /// Where drill stats live, for drill (ticket #14).
    pub drill_store: Arc<dyn DrillStoring>,
    /// Announced on screen when the graph is not the real one, so a fake run
    /// can never be mistaken for a real one by looking at it.
    pub banner: Option<String>,
    /// Which screen the skin opens on. Only the fake launches change it.
    pub screen: Screen,
    /// The concrete types behind the seams, by name, so a test can tell the
    /// real graph from a fake one without importing either.
    pub kinds: GraphKinds,
    /// A model already fully built by a synchronous fake (`--fake-rich`),
    /// bypassing the usual `naming`/`source`/`store` assembly because its
    /// history needs a scripted clock and a strict call order no
    /// channel-driven run could give it deterministically. `make_model`
    /// returns this directly when set; `kinds` above still describes it.
    pub prebuilt: Option<AppModel>,
}

/// The type names of a graph's three collaborators.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphKinds {
    pub naming: &'static str,
    pub source: &'static str,
    pub store: &'static str,
}

fn short(name: &'static str) -> &'static str {
    name.rsplit("::").next().unwrap_or(name)
}

impl Composition {
    fn assemble<N, S, T>(
        naming: N,
        source: S,
        store: T,
        vocabulary: Arc<dyn ChordVocabulary>,
        drill_store: Arc<dyn DrillStoring>,
        banner: Option<&str>,
        screen: Screen,
    ) -> Self
    where
        N: ChordNaming + 'static,
        S: SoundingSetSource + 'static,
        T: NoteStoring + 'static,
    {
        Composition {
            kinds: GraphKinds {
                naming: short(type_name::<N>()),
                source: short(type_name::<S>()),
                store: short(type_name::<T>()),
            },
            naming: Arc::new(naming),
            source: Box::new(source),
            store: Arc::new(store),
            vocabulary,
            drill_store,
            banner: banner.map(str::to_string),
            screen,
            prebuilt: None,
        }
    }

    pub fn make_model(self) -> AppModel {
        if let Some(model) = self.prebuilt {
            return model;
        }
        let mut model = AppModel::new(self.naming, self.source, self.store)
            .with_drill(self.vocabulary, self.drill_store);
        model.screen = self.screen;
        if let Some(banner) = self.banner {
            model.announce(banner);
        }
        model
    }

    // MARK: - Graphs

    /// The graph the shipped app runs on: the real engine, real MIDI, and the
    /// real notes file. `JsonNotesStore` does no I/O at construction, so
    /// nothing here can stop the skin from opening. On a machine with no home
    /// directory the store falls back to `notes.json` in the working directory.
    pub fn live(input: Option<String>) -> Composition {
        let store = JsonNotesStore::real()
            .unwrap_or_else(|| JsonNotesStore::at(watchord_store::DEFAULT_FILE_NAME));
        // `drill.json` lives beside whichever notes file is actually in use,
        // real or the working-directory fallback — never inside it (ADR-0004).
        let drill_store = JsonDrillStore::beside_notes(store.location());
        Self::assemble(
            NamingEngine::new(),
            MidiSource::new(input),
            store,
            Arc::new(EngineVocabulary::new()),
            Arc::new(drill_store),
            None,
            Screen::NowPlaying,
        )
    }

    /// A graph with no hardware and no real notes file, pre-loaded with ticket
    /// 08's worked example. Used by `watchord --fake`.
    ///
    /// `released` lifts the keys after the chord, so the hold-after-release
    /// behaviour can be looked at standing still.
    pub fn demo(released: bool, screen: Screen) -> Composition {
        // C E G A — ticket 08's worked example.
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

        // A second chord, so the All Notes screen has more than one group.
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
        let source = ScriptedSoundingSetSource::new(script);

        let store = InMemoryNoteStore::new(vec![
            ChordNote::new(
                new_id(),
                c6.key(),
                "C6",
                "sounds like the Rhodes on Voodoo",
                seconds_ago(3600),
            ),
            ChordNote::new(
                new_id(),
                c6.key(),
                "Am7",
                "try it with the 9 on top",
                seconds_ago(60),
            ),
            ChordNote::new(
                new_id(),
                f_major.key(),
                "F",
                "plain, but it lands",
                seconds_ago(600),
            ),
        ]);

        Self::assemble(
            naming,
            source,
            store,
            Arc::new(demo_vocabulary()),
            Arc::new(InMemoryDrillStore::default()),
            Some("fake input — no MIDI hardware is being read"),
            screen,
        )
    }

    // MARK: - The fit demos

    /// C13 with no fifth and no eleventh, played over E: the `Missing` case
    /// with the slash. The alternates carry the other two tiers, so one screen
    /// shows all three placements at once: `·no5 ·no11` under the headline,
    /// `≈` beside an alternate's name, and an exact alternate wearing nothing.
    ///
    /// `nearest_headline` puts the `Nearest` reading on the headline instead —
    /// the placement that matters most and cannot be seen from the alternates.
    pub fn fit_demo(nearest_headline: bool) -> Composition {
        // E3 C4 Bb4 D5 A5 — a rootless-ish C13 voicing with E underneath.
        let c13 = SoundingSet::new([64, 72, 82, 86, 93]);
        // What a C13 claims: C E G Bb D F A. Two of them are not being played.
        let c13_tones: [i32; 7] = [0, 4, 7, 10, 2, 5, 9];
        let c7_tones: [i32; 4] = [0, 4, 7, 10];
        let cm7_tones: [i32; 4] = [0, 3, 7, 10];

        let naming = StubChordNaming::new();
        let analysis = if nearest_headline {
            StubChordNaming::naming_with_fit(
                &c13,
                "C7",
                "C major, minor 7",
                ChordFit::Nearest { distance: 3 },
                Some(&c7_tones),
                vec![
                    StubAlternate::new("C13", SpellingOrigin::Enharmonic, "C major, minor 7, 13")
                        .with_fit(ChordFit::Missing(vec![5, 11]), &c13_tones),
                    StubAlternate::new("Am7", SpellingOrigin::ReRooted, "A minor 7"),
                ],
            )
        } else {
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
            )
        };
        naming.stub(&c13, analysis);

        Self::assemble(
            naming,
            ScriptedSoundingSetSource::new(vec![c13]),
            InMemoryNoteStore::default(),
            Arc::new(demo_vocabulary()),
            Arc::new(InMemoryDrillStore::default()),
            Some("fake input — fit tiers are stubbed, no MIDI hardware is being read"),
            Screen::NowPlaying,
        )
    }

    /// A scripted nine-chord session — a real key context, both pedals, a
    /// mid-session drill grade, and notes with tags and a multi-line entry —
    /// so every region of the plain skin has something real in it for a
    /// design screenshot. Built entirely synchronously in
    /// [`watchord_model::fakes::rich_demo`]; see that function's doc comment
    /// for why it never goes through the channel-driven source.
    pub fn rich_demo(screen: Screen) -> Composition {
        let mut graph = Self::assemble(
            StubChordNaming::new(),
            ScriptedSoundingSetSource::default(),
            InMemoryNoteStore::default(),
            Arc::new(FixedVocabulary::default()),
            Arc::new(InMemoryDrillStore::default()),
            Some("fake input — a scripted session, no MIDI hardware is being read"),
            screen,
        );
        graph.prebuilt = Some(build_rich_demo(screen));
        graph
    }

    /// Chooses a graph from the parsed command line.
    ///
    /// - `--fake` — the demo graph, chord held.
    /// - `--fake-released` — the demo graph, keys lifted.
    /// - `--fake-fit` — a `Missing` headline over a slash bass, with a
    ///   `Nearest` alternate beneath it.
    /// - `--fake-nearest` — the same keys with a `Nearest` **headline**.
    /// - `--fake-rich` — a scripted nine-chord session, every plain-skin
    ///   region populated.
    /// - `--all-notes` — open on the All Notes screen.
    /// - `--input <name>` — live, on the inputs whose name contains `name`.
    pub fn for_launch(args: &LaunchArgs) -> Composition {
        let screen = if args.all_notes {
            Screen::AllNotes
        } else {
            Screen::NowPlaying
        };
        if args.fake_rich {
            return Self::rich_demo(screen);
        }
        if args.fake_nearest {
            return Self::fit_demo(true);
        }
        if args.fake_fit {
            return Self::fit_demo(false);
        }
        if args.fake_released {
            return Self::demo(true, screen);
        }
        if args.fake {
            return Self::demo(false, screen);
        }
        Self::live(args.input.clone())
    }
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string().to_uppercase()
}

/// A small hand-written vocabulary for the fake graphs — never the real
/// engine's catalog (ticket #14's fakes must not become a second engine,
/// the same discipline `StubChordNaming` already keeps).
fn demo_vocabulary() -> FixedVocabulary {
    FixedVocabulary::new(vec![
        FixedVocabulary::target("C", &[0, 4, 7]),
        FixedVocabulary::target("Am7", &[9, 0, 4, 7]),
        FixedVocabulary::target("F", &[5, 9, 0]),
        FixedVocabulary::target("Cm7", &[0, 3, 7, 10]),
        FixedVocabulary::target("G7", &[7, 11, 2, 5]),
    ])
}

#[cfg(test)]
mod tests {
    //! The two graph-identity cases from note-view's `CompositionTests`. Type
    //! names rather than downcasts, because the shell sees three seams and
    //! nothing else. Nothing here calls `start()` or reads the store, so this
    //! opens no MIDI client and never touches the notes file.

    use super::*;

    #[test]
    fn a_no_argument_launch_wires_the_real_engine_real_midi_and_the_real_store() {
        let graph = Composition::for_launch(&LaunchArgs::default());

        assert_eq!(graph.kinds.naming, "NamingEngine");
        assert_eq!(graph.kinds.source, "MidiSource");
        assert_eq!(graph.kinds.store, "JsonNotesStore");
        assert_eq!(
            graph.banner, None,
            "the real graph does not announce itself as fake"
        );
        assert_eq!(graph.screen, Screen::NowPlaying);
    }

    #[test]
    fn fake_wires_fakes_instead_and_says_so_on_screen() {
        // The control for the test above: if the assertion could not tell the
        // two graphs apart it would pass on a fully faked app.
        let (args, _) = LaunchArgs::parse(["--fake".to_string()]);
        let graph = Composition::for_launch(&args);

        assert_eq!(graph.kinds.naming, "StubChordNaming");
        assert_eq!(graph.kinds.source, "ScriptedSoundingSetSource");
        assert_eq!(graph.kinds.store, "InMemoryNoteStore");
        assert!(
            graph.banner.is_some(),
            "a fake run must never look like a real one"
        );
    }

    #[test]
    fn all_notes_opens_on_the_other_screen() {
        let (args, _) = LaunchArgs::parse(["--fake".to_string(), "--all-notes".to_string()]);
        let graph = Composition::for_launch(&args);
        assert_eq!(graph.screen, Screen::AllNotes);
        assert_eq!(graph.make_model().screen, Screen::AllNotes);
    }

    #[test]
    fn skin_takes_a_name_in_either_spelling_and_defaults_to_push() {
        let (args, unknown) = LaunchArgs::parse(["--skin".to_string(), "plain".to_string()]);
        assert!(unknown.is_empty());
        assert_eq!(args.skin, Skin::Plain);
        let (args, _) = LaunchArgs::parse(["--skin=push".to_string()]);
        assert_eq!(args.skin, Skin::Push);
        let (args, _) = LaunchArgs::parse(["--fake".to_string()]);
        assert_eq!(args.skin, Skin::Push);
        let (_, unknown) = LaunchArgs::parse(["--skin".to_string(), "neon".to_string()]);
        assert_eq!(unknown.len(), 1, "{unknown:?}");
        assert!(unknown[0].contains("neon"));
        let (_, unknown) = LaunchArgs::parse(["--skin".to_string()]);
        assert_eq!(unknown, ["--skin needs a name: push or plain"]);
    }

    #[test]
    fn input_takes_a_name_in_either_spelling() {
        let (args, unknown) = LaunchArgs::parse(["--input".to_string(), "OP-1".to_string()]);
        assert!(unknown.is_empty());
        assert_eq!(args.input.as_deref(), Some("OP-1"));
        let (args, _) = LaunchArgs::parse(["--input=field".to_string()]);
        assert_eq!(args.input.as_deref(), Some("field"));
        let (_, unknown) = LaunchArgs::parse(["--input".to_string()]);
        assert_eq!(unknown, ["--input needs a name"]);
    }

    /// `--fake-rich --print` carries a real, multi-entry history and a real
    /// key-context numeral — the two claims a screenshot cannot make for
    /// itself. `crate::print::render` is the exact function the CLI's
    /// `--print` flag calls, so this is the same text a terminal would see.
    #[test]
    fn fake_rich_prints_a_real_history_and_a_real_numeral() {
        let (args, unknown) = LaunchArgs::parse(["--fake-rich".to_string()]);
        assert!(unknown.is_empty(), "{unknown:?}");
        let graph = Composition::for_launch(&args);
        assert_eq!(graph.kinds.naming, "StubChordNaming");
        assert!(graph.banner.is_some());
        let model = graph.make_model();
        let text = crate::print::render(&model.frame());

        assert!(
            text.contains("history: 9 entries"),
            "at least 8 history entries, carried as `history: `:\n{text}"
        );
        assert!(
            model.frame().history.len() >= 8,
            "the frame's own history is at least 8 long"
        );
        assert!(
            text.contains("key context: C major"),
            "a real key context, not —:\n{text}"
        );
        let numeral = model
            .frame()
            .headline
            .as_ref()
            .and_then(|h| h.numeral.clone());
        assert!(
            numeral.as_deref().is_some_and(|n| n != "—"),
            "the headline's numeral is a real value, not the absent mark: {numeral:?}"
        );
        assert!(
            text.contains(&format!("numeral {}", numeral.unwrap())),
            "the same numeral appears in the reading line:\n{text}"
        );
    }
}
