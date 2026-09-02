// Panel placement, kept out of the SFC so it can be tested without mounting a
// component or a browser.
//
// The layout is data: the config gives every panel a rectangle in percent, and
// this maps that onto CSS. Percentages rather than a CSS grid template, because
// a template can only express a layout that decomposes into rows and columns
// while a RectSpec can express any rectangle.

import type { PanelKind, PanelSpec } from './types'

export type Placement = Record<string, string>

/** The panels a config declares, or an empty list if it declares none. */
export function readLayout(configJson: string): PanelSpec[] {
  const parsed = JSON.parse(configJson) as { layout?: { panels?: PanelSpec[] } }
  return parsed.layout?.panels ?? []
}

/**
 * The same config with a different set of panels.
 *
 * The stored config is what the next reload starts from, and it used to be
 * written once and never again -- so a panel added while the terminal ran was
 * gone on reload, against a README that said the layout is persisted. The whole
 * config is rewritten rather than a layout stored beside it, because two places
 * holding a layout is one more than can be kept in step.
 *
 * Returns the input unchanged if it does not parse: a config this cannot read is
 * one the terminal did not start from either, and losing it here would turn a
 * display fault into a data loss.
 */
export function withLayout(configJson: string, panels: PanelSpec[]): string {
  let parsed: { layout?: { panels?: PanelSpec[] } }
  try {
    parsed = JSON.parse(configJson) as { layout?: { panels?: PanelSpec[] } }
  } catch {
    return configJson
  }
  if (parsed === null || typeof parsed !== 'object') {
    return configJson
  }
  parsed.layout = { ...(parsed.layout ?? {}), panels }
  return JSON.stringify(parsed)
}

/**
 * Absolute CSS placement per panel kind.
 *
 * A kind missing from the result is missing from the layout, and the renderer
 * then does not draw it at all rather than drawing it somewhere arbitrary.
 *
 * When a layout names the same kind twice the first wins, because the core
 * renders one view per panel kind and a second placement would have nothing to
 * put in it.
 */
export function placementsFor(panels: PanelSpec[]): Partial<Record<PanelKind, Placement>> {
  const out: Partial<Record<PanelKind, Placement>> = {}
  for (const spec of panels) {
    if (out[spec.kind]) {
      continue
    }
    out[spec.kind] = {
      left: `${spec.rect.x}%`,
      top: `${spec.rect.y}%`,
      width: `${spec.rect.w}%`,
      height: `${spec.rect.h}%`,
    }
  }
  return out
}
