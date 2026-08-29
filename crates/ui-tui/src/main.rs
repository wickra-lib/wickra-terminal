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
#[command(name = "wickra-terminal", version, about)]
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
