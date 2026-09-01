//! Parse a source shorthand (`--source` flag or the runtime source menu) into a
//! [`SourceSpec`]. Shared by the CLI and the interactive source menu.

use wickra_terminal_core::source::live::parse_live_shorthand;
use wickra_terminal_core::{IndicatorSpec, Market, SourceSpec};

/// Parse `synth:<seed>`, `live:<venue>:<BASE/QUOTE>` or `replay:<json>`.
///
/// # Errors
///
/// Returns a human-readable message if the shorthand is not recognized.
pub(crate) fn parse_source(spec: &str) -> Result<SourceSpec, String> {
    let (kind, rest) = spec.split_once(':').ok_or_else(|| {
        "expected kind:… (synth:1 | live:venue:BASE/QUOTE | replay:JSON)".to_string()
    })?;
    match kind {
        "synth" => Ok(SourceSpec::Synth {
            seed: rest.parse().map_err(|e| format!("bad seed: {e}"))?,
        }),
        "live" => {
            // An optional market after the symbol: `live:binance:BTC/USDT:usdm`.
            // A symbol carries a slash and never a colon, so a second colon in
            // `rest` can only be the market's -- and if the word after it is not
            // a market name, that is a typo rather than part of the symbol.
            //
            // Letting it fall through was the previous reading and it was worse
            // than an error: `parse_live_shorthand` splits on the first colon,
            // so `binance:BTC/USDT:usdn` became the market `USDT:usdn`, which
            // the venue then rejected with a message about an unknown symbol.
            let (rest, market) = match rest.rsplit_once(':') {
                Some((head, tail)) if head.contains(':') => match parse_market(tail) {
                    Some(market) => (head, market),
                    None => {
                        return Err(format!(
                            "unknown market {tail:?} (spot | usdm | coinm | margin)"
                        ))
                    }
                },
                _ => (rest, Market::Spot),
            };
            let (venue, symbol) = parse_live_shorthand(rest).map_err(|e| e.to_string())?;
            Ok(SourceSpec::Live {
                venue,
                symbol,
                testnet: false,
                market,
            })
        }
        "replay" => Ok(SourceSpec::Replay {
            dataset: rest.to_string(),
        }),
        other => Err(format!("unknown source kind: {other}")),
    }
}

/// The market names the source shorthand accepts.
///
/// Short forms rather than the enum's own spelling: these are typed at a prompt,
/// and `usdm` is what a trader calls the linear book.
fn parse_market(text: &str) -> Option<Market> {
    match text {
        "spot" => Some(Market::Spot),
        "usdm" | "perp" | "futures" => Some(Market::UsdMFutures),
        "coinm" | "inverse" => Some(Market::CoinMFutures),
        "margin" => Some(Market::Margin),
        _ => None,
    }
}

/// Parse an indicator shorthand into an [`IndicatorSpec`].
///
/// `Sma 20`, `Macd 12 26 9`, `Beta 20 vs ETH/USDT`. Also accepts the label form
/// the chart panel prints -- `Sma(20)`, `Beta(20) vs ETH/USDT` -- so a user can
/// read a name off the screen and type it straight back to remove or re-add it.
///
/// # Errors
///
/// Returns a human-readable message if the kind is missing, a parameter is not
/// a number, or `vs` is given without a market after it.
pub(crate) fn parse_indicator(text: &str) -> Result<IndicatorSpec, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("expected a kind (Sma 20 | Macd 12 26 9 | Beta 20 vs ETH/QUOTE)".to_string());
    }
    // `vs` splits the reference off first, so a market with digits in it is
    // never mistaken for a parameter.
    let (head, reference) = match text.split_once(" vs ") {
        Some((head, market)) => {
            let market = market.trim();
            if market.is_empty() {
                return Err("`vs` with no market after it".to_string());
            }
            (head.trim(), Some(market.to_string()))
        }
        None => (text, None),
    };
    // The label form packs the parameters into brackets; both forms then read as
    // whitespace-separated words.
    let flattened = head.replace(['(', ')', ','], " ");
    let mut words = flattened.split_whitespace();
    let kind = words
        .next()
        .ok_or_else(|| "expected a kind".to_string())?
        .to_string();
    let params = words
        .map(|word| {
            word.parse::<f64>()
                .map_err(|_| format!("`{word}` is not a number"))
        })
        .collect::<Result<Vec<f64>, String>>()?;
    Ok(IndicatorSpec {
        kind,
        params,
        reference,
    })
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
                market: Market::Spot,
            }
        );
    }

    #[test]
    fn parse_source_live_takes_a_market() {
        // The market was hard-coded to spot, so a perpetual could not be opened
        // at all -- and adding it to the config alone would have left it
        // unreachable from the prompt, which is the mistake this repository
        // keeps making.
        assert_eq!(
            parse_source("live:binance:BTC/USDT:usdm").unwrap(),
            SourceSpec::Live {
                venue: "binance".to_string(),
                symbol: "BTC/USDT".to_string(),
                testnet: false,
                market: Market::UsdMFutures,
            }
        );
        assert_eq!(
            parse_source("live:binance:BTC/USDT:coinm").unwrap(),
            SourceSpec::Live {
                venue: "binance".to_string(),
                symbol: "BTC/USDT".to_string(),
                testnet: false,
                market: Market::CoinMFutures,
            }
        );
    }

    #[test]
    fn a_trailing_word_that_is_not_a_market_is_reported() {
        // Not merely rejected -- reported usefully. Falling through was the
        // previous reading, and it made `binance:BTC/USDT:usdn` a request for
        // the market `USDT:usdn`, which the venue answered with a message about
        // an unknown symbol.
        let err = parse_source("live:binance:BTC/USDT:nonsense").unwrap_err();
        assert!(err.contains("unknown market"), "unhelpful: {err}");
        assert!(
            err.contains("usdm"),
            "the message does not list the names: {err}"
        );
    }

    #[test]
    fn a_two_part_live_shorthand_still_means_spot() {
        assert_eq!(
            parse_source("live:binance:BTC/USDT").unwrap(),
            SourceSpec::Live {
                venue: "binance".to_string(),
                symbol: "BTC/USDT".to_string(),
                testnet: false,
                market: Market::Spot,
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
