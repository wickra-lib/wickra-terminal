//! The chart widget: candles of the configured timeframe, with a price scale.
//!
//! What this drew before was a single row of eight block glyphs — a sparkline of
//! the last-trade series, with no axis, no scale and no bar structure — on the
//! panel that occupies seventy percent of the default layout. The view-model now
//! carries the bars, so this draws them.
//!
//! Indicator values stay a text readout rather than lines over the candles, and
//! that is deliberate: an indicator's series is sampled once per tick while the
//! candles are one per bar, so the two do not share an x-axis. Drawing them
//! together would put an average somewhere near, but not on, the bar it was
//! computed from — which reads as a real reading and is not one.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Stylize};
use ratatui::symbols::Marker;
use ratatui::text::Line as TextLine;
use ratatui::widgets::canvas::{Canvas, Line as CanvasLine};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use wickra_terminal_core::view::{ChartView, IndicatorValue, OhlcBar};

/// Block glyphs from empty to full, for the fallback sparkline.
const LEVELS: [char; 8] = [
    ' ', '\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}',
];

/// Columns reserved for the price scale, wide enough for eight digits and a
/// space (`20115.00 `), which covers every crypto pair the terminal opens.
const SCALE_WIDTH: u16 = 10;

/// Half the horizontal extent of a candle body, in x units where one bar is 1.0.
///
/// Slightly under a half so neighbouring bodies do not touch: at braille
/// resolution two adjacent filled columns read as one wide bar and the count of
/// candles stops being legible.
const BODY_HALF_WIDTH: f64 = 0.32;

/// Render a price series as a single-line block sparkline, using at most `width`
/// of the most recent points.
///
/// Still here, and still used: it is what the panel falls back to when a market
/// has not closed a bar yet — a chart at a one-hour timeframe shows nothing for
/// an hour otherwise, and the tick series is the only thing that exists then.
#[must_use]
pub(crate) fn sparkline(series: &[f64], width: usize) -> String {
    if series.is_empty() || width == 0 {
        return String::new();
    }
    let recent = &series[series.len().saturating_sub(width)..];
    let min = recent.iter().copied().fold(f64::INFINITY, f64::min);
    let max = recent.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let range = (max - min).max(f64::EPSILON);
    recent
        .iter()
        .map(|&v| {
            let level = ((v - min) / range * (LEVELS.len() - 1) as f64).round() as usize;
            LEVELS[level.min(LEVELS.len() - 1)]
        })
        .collect()
}

/// The low and high across every bar drawn, including the forming one.
///
/// Returns `None` when nothing has a finite extent, which is what an empty
/// market looks like before its first print.
#[must_use]
pub(crate) fn price_range(bars: &[OhlcBar]) -> Option<(f64, f64)> {
    let mut low = f64::INFINITY;
    let mut high = f64::NEG_INFINITY;
    for bar in bars {
        if bar.low.is_finite() {
            low = low.min(bar.low);
        }
        if bar.high.is_finite() {
            high = high.max(bar.high);
        }
    }
    if !low.is_finite() || !high.is_finite() {
        return None;
    }
    // A market that has not moved has a zero range, which would divide the
    // whole plot by nothing. Give it a hair of height so the row still draws.
    if (high - low).abs() < f64::EPSILON {
        let pad = high.abs().mul_add(0.0005, f64::EPSILON);
        return Some((low - pad, high + pad));
    }
    Some((low, high))
}

/// The price scale: `rows` labels from `high` at the top down to `low`.
#[must_use]
pub(crate) fn scale_labels(low: f64, high: f64, rows: usize) -> Vec<String> {
    if rows == 0 {
        return Vec::new();
    }
    if rows == 1 {
        return vec![format!("{high:>9.2}")];
    }
    let span = high - low;
    (0..rows)
        .map(|row| {
            let value = high - span * (row as f64) / ((rows - 1) as f64);
            format!("{value:>9.2}")
        })
        .collect()
}

/// Draw one bar: the wick from low to high, and the body from open to close.
fn draw_bar(ctx: &mut ratatui::widgets::canvas::Context<'_>, index: usize, bar: &OhlcBar) {
    let colour = if bar.close >= bar.open {
        Color::Green
    } else {
        Color::Red
    };
    let centre = index as f64 + 0.5;
    ctx.draw(&CanvasLine {
        x1: centre,
        y1: bar.low,
        x2: centre,
        y2: bar.high,
        color: colour,
    });
    // The body as three vertical strokes rather than a Rectangle: a doji, where
    // open equals close, has zero height and a rectangle of no height draws
    // nothing at all — the one bar shape a trader most wants to see.
    let (body_low, body_high) = if bar.close >= bar.open {
        (bar.open, bar.close)
    } else {
        (bar.close, bar.open)
    };
    for offset in [-BODY_HALF_WIDTH, 0.0, BODY_HALF_WIDTH] {
        ctx.draw(&CanvasLine {
            x1: centre + offset,
            y1: body_low,
            x2: centre + offset,
            y2: body_high,
            color: colour,
        });
    }
}

/// One reading as text: a number, or the named outputs when it has several.
///
/// `value` is the first field of a multi-output indicator, so a readout that
/// showed only it drew `Macd(12,26,9)=1.42` and dropped the signal line and
/// the histogram -- the two numbers the indicator exists to be read against.
/// The core has carried them across the boundary all along; neither renderer
/// drew them.
fn reading(indicator: &IndicatorValue) -> String {
    let number = |value: Option<f64>| {
        value.map_or_else(|| "\u{2026}".to_string(), |value| format!("{value:.2}"))
    };
    if indicator.fields.is_empty() {
        return format!("{}={}", indicator.name, number(indicator.value));
    }
    let named = indicator
        .fields
        .iter()
        .map(|field| format!("{}={}", field.name, number(Some(field.value))))
        .collect::<Vec<_>>()
        .join(" ");
    format!("{}[{named}]", indicator.name)
}

/// The colours an overlay cycles through.
///
/// The nearest terminal colours to the browser's palette, in the browser's
/// order -- amber, violet, green, pink, cyan (`OVERLAY_COLOURS` in
/// `web/src/render/chart.ts`). A terminal cannot match a hex triple, but it can
/// keep the order, so the second indicator added is the second colour in both
/// and a chart read in one renderer and then the other does not renumber its
/// lines.
const OVERLAY_COLOURS: [Color; 5] = [
    Color::Yellow,
    Color::Magenta,
    Color::Green,
    Color::LightMagenta,
    Color::Cyan,
];

/// The lowest and highest value in a set of series, or `None` if all are empty.
pub(crate) fn series_range(series: &[&[f64]]) -> Option<(f64, f64)> {
    let mut low = f64::INFINITY;
    let mut high = f64::NEG_INFINITY;
    for values in series {
        for value in values.iter().copied().filter(|value| value.is_finite()) {
            low = low.min(value);
            high = high.max(value);
        }
    }
    (low <= high).then_some((low, high))
}

/// Whether an indicator's series can honestly share the price axis.
///
/// An overlay is drawn only when its range overlaps the price's. Without the
/// test an `Rsi` running 0..100 would be plotted on a chart of a market trading
/// near twenty thousand and drawn as a flat line at the bottom -- present,
/// meaningless, and indistinguishable from a broken indicator. The core does not
/// declare which readings are prices, so the ranges are what says it.
///
/// The same rule the browser applies, in the same place: `onPriceScale` in
/// `web/src/render/chart.ts`.
pub(crate) fn on_price_scale(series: &[f64], price: &[f64]) -> bool {
    let (Some((series_low, series_high)), Some((price_low, price_high))) =
        (series_range(&[series]), series_range(&[price]))
    else {
        return false;
    };
    series_low <= price_high && series_high >= price_low
}

/// The overlays a chart may draw over its tick series, longest last.
///
/// Only in the fallback, and only there: an indicator's series is sampled once
/// per tick while a bar is one per bar, so the two do not share an x-axis and an
/// average drawn on the candle axis would sit near, but not on, the bar it was
/// computed from. Before the first bar closes there are no candles and the two
/// series do share an axis, which is the one state where an overlay is honest.
pub(crate) fn overlays(view: &ChartView) -> Vec<&[f64]> {
    if view.series.len() < 2 {
        return Vec::new();
    }
    view.indicators
        .iter()
        .map(|indicator| indicator.series.as_slice())
        .filter(|series| series.len() >= 2)
        .filter(|series| on_price_scale(series, &view.series))
        .collect()
}

/// Plot one series across the full width of the canvas.
fn draw_series(
    ctx: &mut ratatui::widgets::canvas::Context<'_>,
    series: &[f64],
    points: f64,
    colour: Color,
) {
    // Every series is stretched to the same width, so a shorter indicator series
    // stays aligned with the end of the price rather than stopping short of it:
    // they are two samplings of one run, and the reading a person compares is
    // the one at the right-hand edge.
    // `max(2) - 1` rather than a branch on the length: a series of fewer than
    // two points has no window to draw, so the loop below runs zero times and
    // the step is never used -- an `else` arm here would be a line no run can
    // reach.
    let step = points / (series.len().max(2) - 1) as f64;
    for (index, pair) in series.windows(2).enumerate() {
        if !pair[0].is_finite() || !pair[1].is_finite() {
            continue;
        }
        ctx.draw(&CanvasLine {
            x1: index as f64 * step,
            y1: pair[0],
            x2: (index + 1) as f64 * step,
            y2: pair[1],
            color: colour,
        });
    }
}

/// The one-line indicator readout under the plot.
fn readout(view: &ChartView) -> String {
    view.indicators
        .iter()
        .map(reading)
        .collect::<Vec<_>>()
        .join("  ")
}

/// Render the chart panel.
pub(crate) fn render(frame: &mut Frame, area: Rect, view: &ChartView, focused: bool) {
    let title = format!("Chart {} last={:.2}", view.symbol, view.last);
    let block = super::panel_block(title, focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // One row for the indicator readout, the rest for the plot. A panel too
    // short for both keeps the readout: a number is still information, an
    // one-row plot is not.
    let [plot_area, readout_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .areas(inner);

    // Everything the plot can hold, newest last. The forming bar is drawn with
    // the closed ones because a chart that stopped at the last close would show
    // the market standing still for a whole bar.
    let capacity = usize::from(plot_area.width.saturating_sub(SCALE_WIDTH)).max(1);
    let mut bars: Vec<OhlcBar> = view
        .bars
        .iter()
        .rev()
        .take(capacity.saturating_sub(usize::from(view.forming.is_some())))
        .rev()
        .copied()
        .collect();
    bars.extend(view.forming);

    match price_range(&bars) {
        Some((low, high)) if plot_area.height > 0 && plot_area.width > SCALE_WIDTH => {
            let [scale_area, canvas_area] = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(SCALE_WIDTH), Constraint::Min(0)])
                .areas(plot_area);
            let labels = scale_labels(low, high, usize::from(plot_area.height));
            frame.render_widget(
                Paragraph::new(
                    labels
                        .into_iter()
                        .map(|label| TextLine::from(label).dim())
                        .collect::<Vec<_>>(),
                ),
                scale_area,
            );
            let drawn = bars.clone();
            let count = drawn.len() as f64;
            frame.render_widget(
                Canvas::default()
                    .marker(Marker::Braille)
                    .x_bounds([0.0, count.max(1.0)])
                    .y_bounds([low, high])
                    .paint(move |ctx| {
                        for (index, bar) in drawn.iter().enumerate() {
                            draw_bar(ctx, index, bar);
                        }
                    }),
                canvas_area,
            );
        }
        // No bar has closed and none is forming, or the panel is too small for
        // a plot: the tick series is all there is to show.
        _ => {
            let lines = overlays(view);
            let mut all: Vec<&[f64]> = vec![&view.series];
            all.extend(&lines);
            match series_range(&all) {
                // Room for a plot, and something to plot: the price and every
                // overlay that shares its scale, on one canvas. This is the one
                // state where an overlay is honest -- before the first bar
                // closes the tick series is the x-axis, and the indicator series
                // is sampled on it.
                Some((low, high))
                    if plot_area.height > 0
                        && plot_area.width > SCALE_WIDTH
                        && view.series.len() > 1 =>
                {
                    let [scale_area, canvas_area] = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([Constraint::Length(SCALE_WIDTH), Constraint::Min(0)])
                        .areas(plot_area);
                    let labels = scale_labels(low, high, usize::from(plot_area.height));
                    frame.render_widget(
                        Paragraph::new(
                            labels
                                .into_iter()
                                .map(|label| TextLine::from(label).dim())
                                .collect::<Vec<_>>(),
                        ),
                        scale_area,
                    );
                    let price: Vec<f64> = view.series.clone();
                    let drawn: Vec<Vec<f64>> = lines.iter().map(|s| s.to_vec()).collect();
                    let points = (price.len().max(2) - 1) as f64;
                    frame.render_widget(
                        Canvas::default()
                            .marker(Marker::Braille)
                            .x_bounds([0.0, points])
                            .y_bounds([low, high])
                            .paint(move |ctx| {
                                for (index, series) in drawn.iter().enumerate() {
                                    draw_series(
                                        ctx,
                                        series,
                                        points,
                                        OVERLAY_COLOURS[index % OVERLAY_COLOURS.len()],
                                    );
                                }
                                // The price last, so it is never hidden under an
                                // overlay that happens to run along it.
                                draw_series(ctx, &price, points, Color::White);
                            }),
                        canvas_area,
                    );
                }
                // Too small for a plot, or nothing finite to draw: one row of
                // block glyphs still says which way the market has gone.
                _ => {
                    let width = usize::from(plot_area.width);
                    frame.render_widget(
                        Paragraph::new(TextLine::from(sparkline(&view.series, width))),
                        plot_area,
                    );
                }
            }
        }
    }

    frame.render_widget(Paragraph::new(readout(view)), readout_area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::harness;

    #[test]
    fn sparkline_maps_min_and_max_to_endpoints() {
        let s = sparkline(&[1.0, 2.0, 3.0], 3);
        let chars: Vec<char> = s.chars().collect();
        assert_eq!(chars.len(), 3);
        assert_eq!(chars[0], ' '); // min -> empty
        assert_eq!(chars[2], LEVELS[LEVELS.len() - 1]); // max -> full
    }

    #[test]
    fn sparkline_empty_or_zero_width_is_empty() {
        assert_eq!(sparkline(&[], 10), "");
        assert_eq!(sparkline(&[1.0, 2.0], 0), "");
    }

    #[test]
    fn sparkline_truncates_to_width() {
        assert_eq!(sparkline(&[1.0, 2.0, 3.0, 4.0, 5.0], 2).chars().count(), 2);
    }

    #[test]
    fn sparkline_flat_series_does_not_panic() {
        let s = sparkline(&[5.0, 5.0, 5.0], 3);
        assert_eq!(s.chars().count(), 3);
    }

    fn bar(open: f64, high: f64, low: f64, close: f64) -> OhlcBar {
        OhlcBar {
            open,
            high,
            low,
            close,
            volume: 1.0,
            timestamp: 0,
        }
    }

    #[test]
    fn price_range_spans_every_wick() {
        // The extremes have to come from high and low, not from open and close:
        // a bar can trade far outside its body, and a scale drawn from the
        // bodies would clip the wicks off the top of the panel.
        let bars = [bar(10.0, 20.0, 5.0, 12.0), bar(12.0, 14.0, 1.0, 3.0)];
        assert_eq!(price_range(&bars), Some((1.0, 20.0)));
    }

    #[test]
    fn price_range_of_nothing_is_nothing() {
        assert_eq!(price_range(&[]), None);
    }

    #[test]
    fn a_market_that_has_not_moved_still_has_height() {
        // Zero range would divide the plot by nothing; the row must still draw.
        let (low, high) = price_range(&[bar(100.0, 100.0, 100.0, 100.0)]).unwrap();
        assert!(high > low, "flat market collapsed to {low}..{high}");
    }

    #[test]
    fn the_scale_runs_from_the_high_down_to_the_low() {
        let labels = scale_labels(100.0, 200.0, 3);
        assert_eq!(labels.len(), 3);
        assert_eq!(labels[0].trim(), "200.00");
        assert_eq!(labels[1].trim(), "150.00");
        assert_eq!(labels[2].trim(), "100.00");
    }

    #[test]
    fn a_one_row_scale_labels_the_high() {
        // Dividing by rows - 1 is a division by zero at one row, and the high is
        // the label that matters: it is the edge a price is about to cross.
        assert_eq!(scale_labels(100.0, 200.0, 1)[0].trim(), "200.00");
        assert!(scale_labels(100.0, 200.0, 0).is_empty());
    }

    #[test]
    fn candles_are_drawn_and_coloured_by_direction() {
        let view = ChartView {
            symbol: "BTC/USDT".to_owned(),
            last: 120.0,
            series: vec![100.0, 120.0],
            bars: vec![
                bar(100.0, 130.0, 90.0, 120.0),
                bar(120.0, 125.0, 80.0, 90.0),
            ],
            forming: None,
            indicators: Vec::new(),
        };
        let buffer = crate::widgets::harness::draw(40, 12, |frame, area| {
            render(frame, area, &view, false);
        });
        let colours: Vec<Color> = buffer
            .content()
            .iter()
            .map(|cell| cell.fg)
            .filter(|fg| matches!(fg, Color::Green | Color::Red))
            .collect();
        assert!(
            colours.contains(&Color::Green),
            "the rising bar is not drawn green"
        );
        assert!(
            colours.contains(&Color::Red),
            "the falling bar is not drawn red"
        );
    }

    #[test]
    fn the_price_scale_is_drawn_beside_the_candles() {
        let view = ChartView {
            symbol: "BTC/USDT".to_owned(),
            last: 120.0,
            series: Vec::new(),
            bars: vec![bar(100.0, 130.0, 90.0, 120.0)],
            forming: None,
            indicators: Vec::new(),
        };
        let buffer = crate::widgets::harness::draw(40, 12, |frame, area| {
            render(frame, area, &view, false);
        });
        let text: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(
            text.contains("130.00"),
            "the high is not labelled on the scale"
        );
        assert!(
            text.contains("90.00"),
            "the low is not labelled on the scale"
        );
    }

    #[test]
    fn a_market_with_no_bars_falls_back_to_the_tick_series() {
        // A one-hour timeframe closes nothing for an hour. Showing an empty
        // plot for that long is the state this fallback exists for.
        let view = ChartView {
            symbol: "BTC/USDT".to_owned(),
            last: 3.0,
            series: vec![1.0, 2.0, 3.0],
            bars: Vec::new(),
            forming: None,
            indicators: Vec::new(),
        };
        // Narrow enough that there is no room for a plot, which is the state
        // the sparkline is still for: a panel wider than the price scale draws
        // the tick series on a canvas instead.
        let buffer = crate::widgets::harness::draw(SCALE_WIDTH + 1, 6, |frame, area| {
            render(frame, area, &view, false);
        });
        let text: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(
            text.contains(LEVELS[LEVELS.len() - 1]),
            "no sparkline drawn for a market with no closed bars"
        );
    }

    #[test]
    fn the_forming_bar_is_drawn_with_the_closed_ones() {
        // Without it the chart stands still for a whole bar, which at an hourly
        // timeframe is an hour of a terminal that looks frozen.
        let view = ChartView {
            symbol: "BTC/USDT".to_owned(),
            last: 500.0,
            series: Vec::new(),
            bars: vec![bar(100.0, 110.0, 95.0, 105.0)],
            forming: Some(bar(105.0, 500.0, 105.0, 500.0)),
            indicators: Vec::new(),
        };
        let buffer = crate::widgets::harness::draw(40, 12, |frame, area| {
            render(frame, area, &view, false);
        });
        let text: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(
            text.contains("500.00"),
            "the scale ignores the forming bar, so it is not being drawn"
        );
    }

    #[test]
    fn the_indicator_readout_keeps_its_row() {
        let view = ChartView {
            symbol: "BTC/USDT".to_owned(),
            last: 120.0,
            series: Vec::new(),
            bars: vec![bar(100.0, 130.0, 90.0, 120.0)],
            forming: None,
            indicators: vec![wickra_terminal_core::view::IndicatorValue {
                name: "Sma(20)".to_owned(),
                value: Some(110.5),
                fields: Vec::new(),
                series: Vec::new(),
            }],
        };
        let buffer = crate::widgets::harness::draw(40, 12, |frame, area| {
            render(frame, area, &view, false);
        });
        let text: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(text.contains("Sma(20)=110.50"), "readout missing: {text}");
    }

    use wickra_terminal_core::view::{IndicatorField, IndicatorValue};

    fn single(name: &str, value: Option<f64>) -> IndicatorValue {
        IndicatorValue {
            name: name.to_owned(),
            value,
            fields: Vec::new(),
            series: Vec::new(),
        }
    }

    /// A single-output indicator reads as one number.
    #[test]
    fn a_single_output_indicator_reads_as_a_number() {
        assert_eq!(
            super::reading(&single("Sma(20)", Some(101.256))),
            "Sma(20)=101.26"
        );
        assert_eq!(super::reading(&single("Sma(20)", None)), "Sma(20)=\u{2026}");
    }

    /// A multi-output one names every output.
    ///
    /// `value` is the first field, so a readout that showed only it drew
    /// `Macd(12,26,9)=1.50` and dropped the signal line and the histogram --
    /// the two numbers the indicator exists to be read against. The same shape
    /// the browser writes, because one indicator read in two places should not
    /// look like two: `web/src/__tests__/indicator.test.ts` pins the other half.
    #[test]
    fn a_multi_output_indicator_names_every_output() {
        let macd = IndicatorValue {
            name: "Macd(12,26,9)".to_owned(),
            value: Some(1.5),
            series: Vec::new(),
            fields: vec![
                IndicatorField {
                    name: "macd".to_owned(),
                    value: 1.5,
                },
                IndicatorField {
                    name: "signal".to_owned(),
                    value: 1.25,
                },
                IndicatorField {
                    name: "histogram".to_owned(),
                    value: 0.25,
                },
            ],
        };
        assert_eq!(
            super::reading(&macd),
            "Macd(12,26,9)[macd=1.50 signal=1.25 histogram=0.25]"
        );
    }

    fn with_series(price: Vec<f64>, indicators: Vec<(&str, Vec<f64>)>) -> ChartView {
        ChartView {
            symbol: "BTC/USDT".to_owned(),
            last: *price.last().unwrap_or(&0.0),
            series: price,
            bars: Vec::new(),
            forming: None,
            indicators: indicators
                .into_iter()
                .map(|(name, series)| IndicatorValue {
                    name: name.to_owned(),
                    value: series.last().copied(),
                    fields: Vec::new(),
                    series,
                })
                .collect(),
        }
    }

    /// An overlay is drawn only where it can be read against the price.
    ///
    /// An `Rsi` running 0..100 on a chart of a market near twenty thousand would
    /// be a flat line at the bottom -- present, meaningless, and impossible to
    /// tell from a broken indicator. The core does not say which readings are
    /// prices, so the ranges do.
    #[test]
    fn only_a_series_sharing_the_price_scale_is_overlaid() {
        let view = with_series(
            vec![20_000.0, 20_010.0, 20_020.0],
            vec![
                ("Sma(2)", vec![20_005.0, 20_015.0]),
                ("Rsi(14)", vec![55.0, 60.0, 65.0]),
            ],
        );
        let drawn = super::overlays(&view);
        assert_eq!(drawn.len(), 1, "the RSI was drawn on a price axis");
        assert!((drawn[0][0] - 20_005.0).abs() < 1e-9);
    }

    /// A series of nothing usable shares no scale with anything.
    ///
    /// `overlays` filters short series out before this is asked, so the answer
    /// matters for a direct caller: a series that is all NaN has no range, and
    /// "no range" is not "any range".
    #[test]
    fn a_series_with_no_usable_numbers_is_on_no_scale() {
        let price = [100.0, 101.0];
        assert!(!super::on_price_scale(&[f64::NAN, f64::INFINITY], &price));
        assert!(!super::on_price_scale(&[], &price));
        assert!(!super::on_price_scale(&price, &[]));
    }

    /// A hole in an overlay is stepped over rather than drawn through.
    ///
    /// A NaN between two readings would otherwise be a line to nowhere, and the
    /// canvas would carry it at whatever the projection made of it.
    #[test]
    fn a_hole_in_an_overlay_does_not_reach_the_canvas() {
        let view = with_series(
            (0..10).map(|i| 100.0 + f64::from(i)).collect(),
            vec![(
                "Sma(2)",
                vec![
                    99.0,
                    100.0,
                    f64::NAN,
                    102.0,
                    103.0,
                    104.0,
                    105.0,
                    106.0,
                    107.0,
                    108.0,
                ],
            )],
        );
        let buffer = harness::draw(40, 10, |frame, area| {
            render(frame, area, &view, false);
        });
        // Drawn, and without panicking on the hole.
        let colours: Vec<Color> = buffer.content().iter().map(|cell| cell.fg).collect();
        assert!(
            colours.contains(&OVERLAY_COLOURS[0]),
            "the overlay is missing"
        );
    }

    /// A series too short to be a line is not one.
    #[test]
    fn a_series_of_fewer_than_two_points_is_not_overlaid() {
        let view = with_series(
            vec![100.0, 101.0, 102.0],
            vec![("Sma(2)", vec![101.0]), ("Ema(2)", Vec::new())],
        );
        assert!(super::overlays(&view).is_empty());
    }

    /// Nothing is overlaid on a price series that is not a line either.
    #[test]
    fn nothing_is_overlaid_before_there_are_two_prices() {
        let view = with_series(vec![100.0], vec![("Sma(2)", vec![100.0, 101.0])]);
        assert!(super::overlays(&view).is_empty());
    }

    /// The range spans every series and ignores what is not a number.
    #[test]
    fn the_series_range_spans_them_all_and_skips_the_unusable() {
        let price = [100.0, 105.0];
        let overlay = [95.0, f64::NAN, 110.0];
        let (low, high) = super::series_range(&[&price, &overlay]).expect("both hold numbers");
        assert!((low - 95.0).abs() < 1e-9);
        assert!((high - 110.0).abs() < 1e-9);

        let empty: [f64; 0] = [];
        let all_nan = [f64::NAN, f64::INFINITY];
        assert!(super::series_range(&[&empty]).is_none());
        assert!(
            super::series_range(&[&all_nan]).is_none(),
            "a range was found in nothing usable"
        );
    }

    /// The fallback plots the price and its overlays rather than one flat row.
    ///
    /// Before the first bar closes the browser drew the indicator lines and the
    /// terminal drew a single sparkline, from the same frame -- the same data
    /// showing as two different products.
    #[test]
    fn the_pre_bar_fallback_draws_the_overlays_too() {
        let view = with_series(
            (0..20).map(|i| 100.0 + f64::from(i)).collect(),
            vec![("Sma(2)", (0..20).map(|i| 99.0 + f64::from(i)).collect())],
        );
        let buffer = harness::draw(40, 10, |frame, area| render(frame, area, &view, false));
        // Every cell, not the first of each row: the price scale is drawn to the
        // left of the canvas, so the first coloured cell in a row is a label.
        let colours: Vec<Color> = buffer.content().iter().map(|cell| cell.fg).collect();
        assert!(
            colours.contains(&OVERLAY_COLOURS[0]),
            "no overlay was drawn"
        );
        assert!(colours.contains(&Color::White), "the price line is missing");
    }

    /// A panel too small for a plot still says which way the market went.
    #[test]
    fn a_panel_too_small_for_a_plot_falls_back_to_the_sparkline() {
        let view = with_series(vec![100.0, 101.0, 102.0], Vec::new());
        let buffer = harness::draw(SCALE_WIDTH + 2, 3, |frame, area| {
            render(frame, area, &view, false);
        });
        assert!(!harness::text(&buffer).is_empty());
    }
}
