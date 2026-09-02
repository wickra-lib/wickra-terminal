//! Shorthands: the terse forms a person types at a prompt or passes on the
//! command line, parsed into the specs the core takes.
//!
//! A source (`--source` and the runtime source menu), an indicator, a panel and
//! a rectangle. Each is a shape a config can also express in full; these exist
//! because nobody types JSON at a prompt.

use wickra_terminal_core::panels::PanelKind;
use wickra_terminal_core::source::live::parse_live_shorthand;
use wickra_terminal_core::{IndicatorSpec, Market, PanelSpec, RectSpec, SourceSpec};

/// Parse `synth:<seed>`, `live:<venue>:<BASE/QUOTE>` or `replay:<json>`.
///
/// # Errors
///
/// Returns a human-readable message if the shorthand is not recognized.
pub(crate) fn parse_source(spec: &str) -> Result<SourceSpec, String> {
    let (kind, rest) = spec.split_once(':').ok_or_else(|| {
        "expected kind:… (synth:1 | live:venue:BASE/QUOTE[:market] | replay:JSON)".to_string()
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

/// Parse a rectangle: four grid numbers, `x y w h`, each a percent of the
/// screen.
///
/// # Errors
///
/// Returns a human-readable message if there are not four numbers, if one is
/// not a number, or if the rectangle would leave the grid.
pub(crate) fn parse_rect(text: &str) -> Result<RectSpec, String> {
    let mut numbers = Vec::new();
    for word in text.split_whitespace() {
        let value: u16 = word
            .parse()
            .map_err(|_| format!("{word:?} is not a grid number (0-100)"))?;
        numbers.push(value);
    }
    let [x, y, w, h] = numbers.as_slice() else {
        return Err(format!(
            "expected four numbers x y w h, got {}",
            numbers.len()
        ));
    };
    // Refused here rather than drawn clipped: a rectangle running off the grid
    // is a typo every time, and a renderer that silently trimmed it would leave
    // a panel the config says is one size and the screen says is another.
    if x + w > 100 || y + h > 100 {
        return Err(format!(
            "{x} {y} {w} {h} runs off the grid (x+w and y+h must not pass 100)"
        ));
    }
    if *w == 0 || *h == 0 {
        return Err("a panel with no width or no height would draw nothing".to_string());
    }
    Ok(RectSpec::new(*x, *y, *w, *h))
}

/// Parse a panel shorthand into a [`PanelSpec`].
///
/// `Book 70 0 30 35`, or `Tape 0 70 100 30 48` with the depth it carries. The
/// kind is case-insensitive, because this is what a person types at a prompt.
///
/// # Errors
///
/// Returns a human-readable message if the kind is missing or unknown, if the
/// rectangle is not four numbers, or if the depth is not a count.
pub(crate) fn parse_panel(text: &str) -> Result<PanelSpec, String> {
    let text = text.trim();
    let (kind, rest) = text
        .split_once(char::is_whitespace)
        .ok_or_else(|| "expected a kind and a rectangle (Book 70 0 30 35)".to_string())?;
    let kind: PanelKind = kind.parse()?;

    // The depth is a fifth number, so the rectangle is the first four and
    // whatever is left is the depth. Split that way rather than by counting
    // words, so a bad fifth word is reported as a depth rather than as a
    // rectangle of the wrong length.
    let words: Vec<&str> = rest.split_whitespace().collect();
    let (rect_words, depth_words) = words.split_at(words.len().min(4));
    let rect = parse_rect(&rect_words.join(" "))?;
    let depth = match depth_words {
        [] => None,
        [word] => Some(
            word.parse::<usize>()
                .map_err(|_| format!("{word:?} is not a depth (a row count)"))?,
        ),
        _ => {
            return Err(format!(
                "expected at most a depth after the rectangle, got {depth_words:?}"
            ))
        }
    };
    if depth == Some(0) {
        return Err("a depth of 0 carries no rows; leave it out for the default".to_string());
    }
    Ok(PanelSpec { kind, rect, depth })
}

/// Parse an indicator shorthand into an [`IndicatorSpec`].
///
/// `Sma 20`, `Macd 12 26 9`, `Beta 20 vs ETH/USDT`. Also accepts the label form
/// the chart panel prints -- `Sma(20)`, `Beta(20) vs ETH/USDT` -- so a user can
/// read a name off the screen and type it straight back to remove or re-add it.
///
/// # Errors
///
/// Returns a human-readable message if the kind is missing or a parameter is
/// not a number. A dangling `vs` is not one of them: the text is trimmed first,
/// so ` vs ` can only match with a character after it, and `Beta 20 vs` with
/// nothing behind it never splits at all -- `vs` stays in the head and is
/// reported as the parameter it is not.
pub(crate) fn parse_indicator(text: &str) -> Result<IndicatorSpec, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("expected a kind (Sma 20 | Macd 12 26 9 | Beta 20 vs ETH/QUOTE)".to_string());
    }
    // `vs` splits the reference off first, so a market with digits in it is
    // never mistaken for a parameter.
    // The text is trimmed, so the pattern's trailing space guarantees a
    // character after it: `market` is never empty here, and a guard for that
    // would be a branch nothing can take.
    let (head, reference) = match text.split_once(" vs ") {
        Some((head, market)) => (head.trim(), Some(market.trim().to_string())),
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

    /// The two ways an indicator prompt can be empty, and both name what to type.
    ///
    /// A prompt that answers a blank submit with silence -- or with a parse
    /// error about a missing parameter -- teaches nothing. These are the only
    /// messages a user gets, so they carry the shapes the parser accepts.
    #[test]
    fn parse_indicator_rejects_an_empty_prompt() {
        let err = parse_indicator("   ").expect_err("a blank prompt is not an indicator");
        assert!(err.contains("expected a kind"), "{err}");
        assert!(
            err.contains("Sma 20"),
            "the message does not show a shape: {err}"
        );
    }

    /// A dangling `vs` is reported as the word it is, not swallowed.
    ///
    /// It cannot reach the reference split -- the trim leaves no trailing space
    /// for the pattern -- so it falls through as a parameter, and the message
    /// has to say so rather than complaining about something the user did not
    /// type.
    #[test]
    fn parse_indicator_reports_a_dangling_vs_as_a_bad_parameter() {
        let err = parse_indicator("Beta 20 vs").expect_err("`vs` is not a parameter");
        assert!(
            err.contains("vs"),
            "the message does not name the word: {err}"
        );
    }

    /// The reference is trimmed off whatever spacing surrounded it.
    #[test]
    fn parse_indicator_trims_the_reference_market() {
        let spec = parse_indicator("Beta 20 vs   ETH/USDT").expect("a pairwise spec");
        assert_eq!(spec.reference.as_deref(), Some("ETH/USDT"));
    }

    #[test]
    fn parse_panel_reads_a_kind_and_a_rectangle() {
        assert_eq!(
            parse_panel("Book 70 0 30 35").unwrap(),
            PanelSpec::new(PanelKind::Book, RectSpec::new(70, 0, 30, 35))
        );
        // Case-insensitive, because this is what a person types at a prompt.
        assert_eq!(
            parse_panel("tape 0 0 100 100").unwrap().kind,
            PanelKind::Tape
        );
    }

    #[test]
    fn parse_panel_takes_a_depth_after_the_rectangle() {
        let spec = parse_panel("Tape 0 70 100 30 48").unwrap();
        assert_eq!(spec.depth, Some(48));
        // And a depth of zero is refused rather than carried: it would build a
        // panel that draws nothing, which is not what leaving it out means.
        assert!(parse_panel("Tape 0 0 10 10 0")
            .unwrap_err()
            .contains("carries no rows"));
    }

    #[test]
    fn parse_panel_names_the_kinds_it_knows() {
        let err = parse_panel("Ladder 0 0 10 10").unwrap_err();
        assert!(err.contains("unknown panel"), "{err}");
        assert!(
            err.contains("Watchlist"),
            "the message lists no kinds: {err}"
        );
    }

    #[test]
    fn parse_panel_rejects_a_kind_with_no_rectangle() {
        assert!(parse_panel("Book").unwrap_err().contains("Book 70 0 30 35"));
        assert!(parse_panel("Book 1 2 3")
            .unwrap_err()
            .contains("four numbers"));
    }

    /// A rectangle running off the grid is a typo every time.
    ///
    /// Refused rather than drawn clipped: a renderer that trimmed it silently
    /// would leave a panel the config says is one size and the screen says is
    /// another.
    #[test]
    fn parse_rect_refuses_a_rectangle_that_leaves_the_grid() {
        assert_eq!(
            parse_rect("0 0 100 100").unwrap(),
            RectSpec::new(0, 0, 100, 100)
        );
        assert!(parse_rect("70 0 40 35")
            .unwrap_err()
            .contains("runs off the grid"));
        assert!(parse_rect("0 70 100 40")
            .unwrap_err()
            .contains("runs off the grid"));
        assert!(parse_rect("0 0 0 50").unwrap_err().contains("draw nothing"));
        assert!(parse_rect("0 0 x 50").unwrap_err().contains("grid number"));
    }
}
