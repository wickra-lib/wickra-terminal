// The indicator shorthand, kept out of the SFC so it can be tested without
// mounting a component — the same reason ./layout and ./profile are modules.
//
// The grammar is the TUI's, deliberately: a user who reads a label off one
// renderer can type it into the other.

/** An indicator spec as the `AddIndicator` command takes it. */
export interface IndicatorSpec {
  kind: string
  params: number[]
  reference?: string
}

/**
 * Parse `Sma 20`, `Macd 12 26 9` or `Beta 20 vs ETH/USDT`.
 *
 * Also accepts the label form the chart panel prints — `Sma(20)`,
 * `Beta(20) vs ETH/USDT` — so a name read off the screen can be typed straight
 * back to remove or re-add it.
 *
 * Returns `null` rather than throwing: the caller is a form handler, and every
 * rejection here is a typo rather than a fault.
 */
export function parseIndicator(text: string): IndicatorSpec | null {
  const trimmed = text.trim()
  if (trimmed === '') {
    return null
  }
  // `vs` splits the reference off first, so a market with digits in it is never
  // mistaken for a parameter.
  let head = trimmed
  let reference: string | undefined
  const at = trimmed.indexOf(' vs ')
  if (at >= 0) {
    head = trimmed.slice(0, at).trim()
    reference = trimmed.slice(at + 4).trim()
    if (reference === '') {
      return null
    }
  }
  const words = head.replace(/[(),]/g, ' ').split(/\s+/).filter(Boolean)
  const kind = words.shift()
  if (kind === undefined) {
    return null
  }
  const params: number[] = []
  for (const word of words) {
    const value = Number(word)
    if (!Number.isFinite(value)) {
      return null
    }
    params.push(value)
  }
  return reference === undefined ? { kind, params } : { kind, params, reference }
}
