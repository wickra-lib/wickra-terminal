//! `wickra-terminal` — the native TUI renderer.
//!
//! One of two reference renderers over [`terminal_core`]; the other is the Web
//! app in `web/`. Both consume the same view-models. Select the renderer with
//! `--render tui|web`; this binary drives the TUI and points `--render web` at
//! the web app.

mod app;
mod input;
mod render;
mod term;
mod widgets;

use std::error::Error;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal as TuiTerminal;
use terminal_core::source::live::parse_live_shorthand;
use terminal_core::{Config, SourceSpec, Symbol, Terminal};

use app::App;
use term::TermGuard;

/// The native TUI renderer for the Wickra trading terminal.
#[derive(Parser)]
#[command(name = "wickra-terminal", version, about)]
struct Cli {
    /// Which renderer to use: `tui` (this binary) or `web` (the web app).
    #[arg(long, default_value = "tui")]
    render: String,

    /// A source shorthand: `synth:<seed>`, `live:<venue>:<BASE/QUOTE>` or
    /// `replay:<json>`.
    #[arg(long)]
    source: Option<String>,

    /// A TOML config file (overrides `--source`).
    #[arg(long)]
    config: Option<PathBuf>,
}

/// Parse a `--source` shorthand into a [`SourceSpec`].
fn parse_source(spec: &str) -> Result<SourceSpec, Box<dyn Error>> {
    let (kind, rest) = spec
        .split_once(':')
        .ok_or("source must be kind:… (synth:1 | live:venue:BASE/QUOTE | replay:JSON)")?;
    match kind {
        "synth" => Ok(SourceSpec::Synth {
            seed: rest.parse()?,
        }),
        "live" => {
            let (venue, symbol) = parse_live_shorthand(rest)?;
            Ok(SourceSpec::Live {
                venue,
                symbol,
                testnet: false,
            })
        }
        "replay" => Ok(SourceSpec::Replay {
            dataset: rest.to_string(),
        }),
        other => Err(format!("unknown source kind: {other}").into()),
    }
}

/// Build the config from `--config` or `--source` (or the bare default layout).
fn build_config(cli: &Cli) -> Result<Config, Box<dyn Error>> {
    if let Some(path) = &cli.config {
        let text = std::fs::read_to_string(path)?;
        return Ok(Config::from_toml(&text)?);
    }
    let mut config = Config::default_layout();
    if let Some(spec) = &cli.source {
        config.sources.push(parse_source(spec)?);
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
        tui.draw(|frame| render::draw(frame, &app.frame, app.terminal.config()))?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    let action = input::map_key(key, &app.terminal.config().layout.keybinds);
                    app.on_action(action);
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
    match cli.render.as_str() {
        "web" => {
            println!("The web renderer lives in web/. Run: cd web && npm install && npm run dev");
            Ok(())
        }
        "tui" => {
            let config = build_config(&cli)?;
            let mut terminal = Terminal::new(&config)?;
            ensure_subscription(&mut terminal, &config)?;
            run(App::new(terminal))
        }
        other => Err(format!("unknown renderer: {other} (expected tui or web)").into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_source_synth() {
        assert_eq!(
            parse_source("synth:7").unwrap(),
            SourceSpec::Synth { seed: 7 }
        );
    }

    #[test]
    fn parse_source_live() {
        assert_eq!(
            parse_source("live:binance:BTC/USDT").unwrap(),
            SourceSpec::Live {
                venue: "binance".to_string(),
                symbol: "BTC/USDT".to_string(),
                testnet: false,
            }
        );
    }

    #[test]
    fn parse_source_replay() {
        assert_eq!(
            parse_source("replay:[]").unwrap(),
            SourceSpec::Replay {
                dataset: "[]".to_string(),
            }
        );
    }

    #[test]
    fn parse_source_rejects_unknown_kind() {
        assert!(parse_source("nope:1").is_err());
        assert!(parse_source("noseparator").is_err());
    }

    #[test]
    fn build_config_from_source_adds_the_source() {
        let cli = Cli {
            render: "tui".to_string(),
            source: Some("synth:1".to_string()),
            config: None,
        };
        let cfg = build_config(&cli).unwrap();
        assert_eq!(cfg.sources, vec![SourceSpec::Synth { seed: 1 }]);
    }
}
