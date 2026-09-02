// Watchlist row formatting, kept out of the SFC so it can be tested without a
// browser — the same reason ./layout, ./profile, ./indicator and ./keybinds are
// modules.
//
// The thresholds and the dash are shared with the TUI widget on purpose: the two
// renderers show one watchlist, and a column that abbreviates in one and not in
// the other reads as two different numbers for the same market.

import type { WatchRow } from './types'

/**
 * The spread a row's ticker implies, or a dash before the first ticker.
 *
 * A market that has traded but never tickered reports a zero bid and ask, and
 * "0.00" there would read as a zero spread rather than as no quote — which on a
 * watchlist is the difference between a market locked at the touch and one the
 * terminal has no quote for.
 */
export function spread(row: WatchRow): string {
  return row.bid > 0 && row.ask > 0 ? (row.ask - row.bid).toFixed(2) : '-'
}

/**
 * A rolling volume as a person reads it: `1.23M` rather than `1234567.89`.
 *
 * A venue's base-asset volume runs to seven figures on a liquid market and to
 * three decimals on an illiquid one, and a column wide enough for both is a
 * column that fits nothing else.
 */
export function compactVolume(volume: number): string {
  const magnitude = Math.abs(volume)
  if (magnitude >= 1e9) return `${(volume / 1e9).toFixed(2)}B`
  if (magnitude >= 1e6) return `${(volume / 1e6).toFixed(2)}M`
  if (magnitude >= 1e3) return `${(volume / 1e3).toFixed(2)}K`
  return volume.toFixed(2)
}

/** The class that colours a row's change: only the movers carry colour. */
export function changeClass(change: number): 'up' | 'down' | 'flat' {
  if (change > 0) return 'up'
  if (change < 0) return 'down'
  return 'flat'
}
