//! `wickra-terminal` — the native TUI renderer.
//!
//! One of two reference renderers over [`wickra_terminal_core`]; the other is the Web
//! app in `web/`. Both consume the same view-models, and both are driven by the
//! same config.
//!
//! They are separate programs, not two modes of one. This binary is the TUI; the
//! web app is a Vite project you run from `web/`. There used to be a
//! `--render tui|web` flag here, but `--render web` could only ever print an
//! instruction to go and run something else, which is a worse way of saying what
//! the README says.

mod app;
mod input;
mod render;
mod spec;
mod term;
mod widgets;

use std::error::Error;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal as TuiTerminal;
use wickra_terminal_core::{Config, Symbol, Terminal};

use app::App;
use term::TermGuard;

/// The native TUI renderer for the Wickra trading terminal.
///
/// The web renderer is a separate app in `web/`; see the README.
#[derive(Parser)]
#[command(name = "wkterm", version, about)]
struct Cli {
    /// A source shorthand: `synth:<seed>`,
    /// `live:<venue>:<BASE/QUOTE>[:<market>]` or `replay:<json>`.
    ///
    /// A live source opens the venue's spot book unless a market follows the
    /// symbol -- `spot`, `usdm`, `coinm` or `margin` -- so
    /// `live:binance:BTC/USDT:usdm` watches the USD-margined perpetual.
    #[arg(long)]
    source: Option<String>,

    /// A TOML config file (overrides `--source`).
    #[arg(long)]
    config: Option<PathBuf>,

    /// Record the session, keeping this many events.
    ///
    /// The recorder was a config field and nothing else, so keeping a session
    /// meant writing a TOML file first. Applied on top of `--config` too: a
    /// stored layout is a layout, and whether this run is being recorded is a
    /// decision about this run.
    #[arg(long, value_name = "EVENTS")]
    record: Option<usize>,

    /// How many historical bars a fresh subscription fetches (0 to turn it off).
    ///
    /// Also applied on top of `--config`, and for the same reason: how far back
    /// to reach is a decision about this run, not part of the layout.
    #[arg(long, value_name = "BARS")]
    backfill: Option<usize>,
}

/// Build the config from `--config` or `--source` (or the bare default layout).
fn build_config(cli: &Cli) -> Result<Config, Box<dyn Error>> {
    let mut config = if let Some(path) = &cli.config {
        let text = std::fs::read_to_string(path)?;
        Config::from_toml(&text)?
    } else {
        let mut config = Config::default_layout();
        if let Some(shorthand) = &cli.source {
            config
                .sources
                .push(spec::parse_source(shorthand).map_err(|e| -> Box<dyn Error> { e.into() })?);
        }
        config
    };
    // After the config rather than instead of it: `--config` chooses a layout,
    // and these two are decisions about this run.
    if let Some(capacity) = cli.record {
        config.record = Some(capacity);
    }
    if let Some(bars) = cli.backfill {
        config.backfill = bars;
    }
    Ok(config)
}

/// A source with no embedded market (synth/replay) needs a default subscription
/// so the panels have something to focus.
fn ensure_subscription(terminal: &mut Terminal, config: &Config) -> Result<(), Box<dyn Error>> {
    if terminal.state().focus.is_none() && !config.sources.is_empty() {
        terminal.subscribe(0, &Symbol::new("BTC", "USDT"))?;
    }
    Ok(())
}

/// Run the event loop until the user quits.
fn run(mut app: App) -> Result<(), Box<dyn Error>> {
    let _guard = TermGuard::new()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut tui = TuiTerminal::new(backend)?;
    loop {
        app.update();
        let footer = app.footer();
        tui.draw(|frame| {
            render::draw(
                frame,
                &app.frame,
                app.terminal.config(),
                &footer,
                app.focused_panel,
                &app.scroll,
            );
        })?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                on_key(&mut app, key);
            }
        }
        if app.should_quit {
            break;
        }
    }
    Ok(())
}

/// Apply one key event to the app.
///
/// Lifted out of the event loop because it is the only policy in it -- what a
/// key means depends on whether a prompt is open, and that decision is worth a
/// test where the loop around it is not: the loop needs a terminal and an event
/// stream, and this needs neither. It was untested for exactly that reason,
/// buried four levels deep in a function no test can enter.
///
/// A release or repeat is ignored: without the filter a held key repeats the
/// action on every report the terminal sends, and on Windows every press is
/// also reported as a release.
fn on_key(app: &mut App, key: KeyEvent) {
    if key.kind != KeyEventKind::Press {
        return;
    }
    if app.is_input() {
        // A prompt takes the keyboard whole. Mapping keys to actions here would
        // fire `quit` for the `q` in a symbol.
        match key.code {
            KeyCode::Enter => app.input_submit(),
            KeyCode::Esc => app.input_cancel(),
            KeyCode::Backspace => app.input_backspace(),
            KeyCode::Char(ch) => app.input_push(ch),
            _ => {}
        }
    } else {
        let action = input::map_key(key, &app.terminal.config().layout.keybinds);
        app.on_action(action);
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let config = build_config(&cli)?;
    let mut terminal = Terminal::new(&config)?;
    ensure_subscription(&mut terminal, &config)?;
    run(App::new(terminal))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wickra_terminal_core::SourceSpec;

    /// A config file in the temp directory, removed when the guard drops.
    ///
    /// Named per test rather than shared: the suite runs in parallel, and two
    /// tests writing one path would each read the other's config.
    struct TempConfig(PathBuf);

    impl TempConfig {
        fn new(name: &str, body: &str) -> Self {
            let path = std::env::temp_dir().join(format!("wickra-terminal-{name}.toml"));
            std::fs::write(&path, body).expect("a writable temp directory");
            Self(path)
        }
    }

    impl Drop for TempConfig {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn cli(source: Option<&str>, config: Option<PathBuf>) -> Cli {
        Cli {
            source: source.map(ToOwned::to_owned),
            config,
            record: None,
            backfill: None,
        }
    }

    #[test]
    fn a_config_file_wins_over_a_source_shorthand() {
        // Documented precedence. A user who passes both and silently gets the
        // shorthand is debugging a file that was never read.
        let file = TempConfig::new("precedence", "[[sources]]\nSynth = { seed = 42 }\n");
        let cfg = build_config(&cli(Some("synth:1"), Some(file.0.clone()))).unwrap();
        assert_eq!(cfg.sources, vec![SourceSpec::Synth { seed: 42 }]);
    }

    #[test]
    fn a_config_file_that_is_not_there_is_reported() {
        let missing = std::env::temp_dir().join("wickra-terminal-no-such-config.toml");
        let _ = std::fs::remove_file(&missing);
        assert!(build_config(&cli(None, Some(missing))).is_err());
    }

    #[test]
    fn a_config_file_that_is_not_a_config_is_reported() {
        // The read succeeds and the parse does not, which is a different arm
        // from a missing file and returns a different error type.
        let file = TempConfig::new("malformed", "not = = valid\n");
        let err = build_config(&cli(None, Some(file.0.clone()))).unwrap_err();
        assert!(
            err.to_string().contains("invalid config"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn a_source_shorthand_that_does_not_parse_is_reported() {
        let err = build_config(&cli(Some("teleport:1"), None)).unwrap_err();
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn neither_flag_is_the_bare_default_layout() {
        // The terminal starts with no source and the renderer draws its hint;
        // defaulting to a synth feed here would show a market nobody asked for.
        let cfg = build_config(&cli(None, None)).unwrap();
        assert!(cfg.sources.is_empty());
        assert_eq!(cfg.layout.panels, Config::default_layout().layout.panels);
    }

    #[test]
    fn a_source_with_no_market_of_its_own_gets_one_to_focus() {
        // Synth and replay carry no symbol, so nothing would be focused and
        // every panel would render its empty state on a feed that is running.
        let mut config = Config::default_layout();
        config.sources = vec![SourceSpec::Synth { seed: 1 }];
        let mut terminal = Terminal::new(&config).unwrap();
        assert!(terminal.state().focus.is_none());
        ensure_subscription(&mut terminal, &config).unwrap();
        assert!(terminal.state().focus.is_some());
    }

    #[test]
    fn a_terminal_with_no_source_at_all_subscribes_to_nothing() {
        // Subscribing against source 0 when there is no source 0 is an error,
        // so the guard is what keeps a bare launch from failing outright.
        let config = Config::default_layout();
        let mut terminal = Terminal::new(&config).unwrap();
        ensure_subscription(&mut terminal, &config).unwrap();
        assert!(terminal.state().focus.is_none());
    }

    #[test]
    fn build_config_from_source_adds_the_source() {
        let cli = Cli {
            source: Some("synth:1".to_string()),
            config: None,
            record: None,
            backfill: None,
        };
        let cfg = build_config(&cli).unwrap();
        assert_eq!(cfg.sources, vec![SourceSpec::Synth { seed: 1 }]);
    }

    /// `--record` and `--backfill` default to leaving the config alone.
    ///
    /// Absent has to mean absent rather than zero: `--backfill` taking a
    /// default of 0 would have silently turned off the history a config asked
    /// for, on every run that did not pass the flag.
    #[test]
    fn the_run_flags_left_off_do_not_touch_the_config() {
        let cfg = build_config(&cli(Some("synth:1"), None)).unwrap();
        let bare = Config::default_layout();
        assert_eq!(cfg.record, bare.record);
        assert_eq!(cfg.backfill, bare.backfill);
    }

    /// They apply on top of a config file rather than instead of it.
    ///
    /// A stored layout is a layout; whether this run is being recorded, and how
    /// far back it reaches, are decisions about this run.
    #[test]
    fn the_run_flags_override_a_config_file() {
        let stored = TempConfig::new(
            "run-flags",
            "record = 512
backfill = 50
[[sources]]
Synth = { seed = 3 }
",
        );
        let cli = Cli {
            source: None,
            config: Some(stored.0.clone()),
            record: Some(64),
            backfill: Some(0),
        };
        let cfg = build_config(&cli).unwrap();
        assert_eq!(cfg.sources, vec![SourceSpec::Synth { seed: 3 }]);
        assert_eq!(cfg.record, Some(64), "--record did not override the file");
        assert_eq!(cfg.backfill, 0, "--backfill 0 did not turn the history off");
    }

    use crossterm::event::{KeyEvent, KeyEventKind, KeyModifiers};

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn app() -> App {
        let mut config = Config::default_layout();
        config.sources = vec![SourceSpec::Synth { seed: 1 }];
        App::new(Terminal::new(&config).expect("the default config builds"))
    }

    /// A prompt takes the keyboard whole.
    ///
    /// Mapping keys to actions while one is open would fire `quit` for the `q`
    /// in a symbol, which is the bug this branch exists to prevent -- and it
    /// lived four levels deep in a loop no test can enter until it was lifted
    /// out.
    #[test]
    fn a_key_typed_into_a_prompt_is_text_and_not_an_action() {
        let mut app = app();
        app.on_action(crate::input::Action::AddSymbol);
        assert!(app.is_input());

        for ch in "BTQ/USDT".chars() {
            on_key(&mut app, press(KeyCode::Char(ch)));
        }
        assert!(app.is_input(), "a bound key ended the prompt");
        let footer = app.footer();
        assert!(footer.contains("BTQ/USDT"), "footer: {footer}");

        on_key(&mut app, press(KeyCode::Backspace));
        on_key(&mut app, press(KeyCode::Esc));
        assert!(!app.is_input());
        assert!(!app.should_quit, "the q in a symbol quit the terminal");
    }

    /// Enter submits what was typed, rather than cancelling it.
    #[test]
    fn enter_submits_a_prompt() {
        let mut app = app();
        app.on_action(crate::input::Action::AddSymbol);
        for ch in "ETH/USDT".chars() {
            on_key(&mut app, press(KeyCode::Char(ch)));
        }
        on_key(&mut app, press(KeyCode::Enter));
        assert!(!app.is_input());
        assert_eq!(app.terminal.state().watchlist.len(), 1);
    }

    /// Outside a prompt a key is an action, through the shared keymap.
    #[test]
    fn a_key_outside_a_prompt_is_an_action() {
        let mut app = app();
        on_key(&mut app, press(KeyCode::Char('q')));
        assert!(app.should_quit);
    }

    /// A release or a repeat is not a second press.
    ///
    /// Without the filter a held key repeats the action on every report the
    /// terminal sends, and on Windows every press is also reported as a release.
    #[test]
    fn only_a_press_acts() {
        let mut app = app();
        let mut release = press(KeyCode::Char('q'));
        release.kind = KeyEventKind::Release;
        on_key(&mut app, release);
        assert!(!app.should_quit, "a release quit the terminal");

        let mut repeat = press(KeyCode::Char('q'));
        repeat.kind = KeyEventKind::Repeat;
        on_key(&mut app, repeat);
        assert!(!app.should_quit, "a repeat quit the terminal");
    }

    /// A key with no meaning in a prompt is neither text nor an action.
    #[test]
    fn an_unhandled_key_in_a_prompt_changes_nothing() {
        let mut app = app();
        app.on_action(crate::input::Action::AddSymbol);
        let before = app.footer();
        on_key(&mut app, press(KeyCode::F(5)));
        assert_eq!(app.footer(), before);
        assert!(app.is_input());
    }
}
