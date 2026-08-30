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
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
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
    /// A source shorthand: `synth:<seed>`, `live:<venue>:<BASE/QUOTE>` or
    /// `replay:<json>`.
    #[arg(long)]
    source: Option<String>,

    /// A TOML config file (overrides `--source`).
    #[arg(long)]
    config: Option<PathBuf>,
}

/// Build the config from `--config` or `--source` (or the bare default layout).
fn build_config(cli: &Cli) -> Result<Config, Box<dyn Error>> {
    if let Some(path) = &cli.config {
        let text = std::fs::read_to_string(path)?;
        return Ok(Config::from_toml(&text)?);
    }
    let mut config = Config::default_layout();
    if let Some(shorthand) = &cli.source {
        config
            .sources
            .push(spec::parse_source(shorthand).map_err(|e| -> Box<dyn Error> { e.into() })?);
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
            );
        })?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if app.is_input() {
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
            }
        }
        if app.should_quit {
            break;
        }
    }
    Ok(())
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
        };
        let cfg = build_config(&cli).unwrap();
        assert_eq!(cfg.sources, vec![SourceSpec::Synth { seed: 1 }]);
    }
}
