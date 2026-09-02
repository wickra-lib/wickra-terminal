// The panel shorthand, kept out of the SFC so it can be tested without a
// browser — the same reason ./layout, ./profile, ./indicator, ./keybinds and
// ./watchlist are modules.
//
// The same shape the TUI prompt takes, and deliberately so: the layout is one
// piece of data shared by both renderers, and a shorthand that meant different
// things in the terminal and the browser would be two.

import type { PanelKind, PanelSpec } from './types'

/** The kind names a config writes, in layout order. */
export const PANEL_KINDS: PanelKind[] = [
  'Chart',
  'Book',
  'Tape',
  'Watchlist',
  'Footprint',
  'Profile',
  'Bars',
]

/**
 * Parse `Book 70 0 30 35`, or `Tape 0 70 100 30 48` with the depth it carries.
 *
 * Returns `null` rather than throwing: this reads what a person typed, and a
 * typo is an ordinary outcome that belongs in the status line.
 *
 * The kind is matched case-insensitively, and a rectangle that runs off the
 * grid is refused rather than accepted and drawn clipped — that is a typo every
 * time, and a renderer that trimmed it would leave a panel the config says is
 * one size and the screen says is another.
 */
export function parsePanel(text: string): PanelSpec | null {
  const words = text.trim().split(/\s+/).filter((word) => word.length > 0)
  if (words.length < 5 || words.length > 6) {
    return null
  }
  const kind = PANEL_KINDS.find((name) => name.toLowerCase() === words[0].toLowerCase())
  if (!kind) {
    return null
  }
  const numbers = words.slice(1).map((word) => Number(word))
  if (numbers.some((value) => !Number.isInteger(value) || value < 0)) {
    return null
  }
  const [x, y, w, h, depth] = numbers
  if (w === 0 || h === 0 || x + w > 100 || y + h > 100) {
    return null
  }
  // A depth of zero carries no rows, which is not what leaving it out means.
  if (depth !== undefined && depth === 0) {
    return null
  }
  const spec: PanelSpec = { kind, rect: { x, y, w, h } }
  if (depth !== undefined) {
    spec.depth = depth
  }
  return spec
}
