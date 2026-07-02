//! Parse a source shorthand (`--source` flag or the runtime source menu) into a
//! [`SourceSpec`]. Shared by the CLI and the interactive source menu.

use terminal_core::source::live::parse_live_shorthand;
use terminal_core::SourceSpec;

/// Parse `synth:<seed>`, `live:<venue>:<BASE/QUOTE>` or `replay:<json>`.
///
/// # Errors
///
/// Returns a human-readable message if the shorthand is not recognized.
pub fn parse_source(spec: &str) -> Result<SourceSpec, String> {
    let (kind, rest) = spec.split_once(':').ok_or_else(|| {
        "expected kind:… (synth:1 | live:venue:BASE/QUOTE | replay:JSON)".to_string()
    })?;
    match kind {
        "synth" => Ok(SourceSpec::Synth {
            seed: rest.parse().map_err(|e| format!("bad seed: {e}"))?,
        }),
        "live" => {
            let (venue, symbol) = parse_live_shorthand(rest).map_err(|e| e.to_string())?;
            Ok(SourceSpec::Live {
                venue,
                symbol,
                testnet: false,
            })
        }
        "replay" => Ok(SourceSpec::Replay {
            dataset: rest.to_string(),
        }),
        other => Err(format!("unknown source kind: {other}")),
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
}
