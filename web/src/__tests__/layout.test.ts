import { describe, expect, it } from 'vitest'

import { placementsFor, readLayout } from '../layout'
import type { PanelSpec } from '../types'

describe('readLayout', () => {
  it('reads the panels a config declares', () => {
    const panels = readLayout(
      JSON.stringify({
        sources: [{ Synth: { seed: 1 } }],
        layout: { panels: [{ kind: 'Chart', rect: { x: 0, y: 0, w: 100, h: 100 } }] },
      }),
    )
    expect(panels).toHaveLength(1)
    expect(panels[0].kind).toBe('Chart')
  })

  it('returns nothing for a config with no layout', () => {
    // The core fills in the standard five panels itself, so an absent layout is
    // a valid config rather than an error. This renderer then has nothing to
    // place until it reads the layout back from the core.
    expect(readLayout(JSON.stringify({ sources: [] }))).toEqual([])
  })
})

describe('placementsFor', () => {
  const layout: PanelSpec[] = [
    { kind: 'Chart', rect: { x: 0, y: 0, w: 70, h: 70 } },
    { kind: 'Book', rect: { x: 70, y: 0, w: 30, h: 35 } },
  ]

  it('maps a rect onto absolute percentages', () => {
    expect(placementsFor(layout).Chart).toEqual({
      left: '0%',
      top: '0%',
      width: '70%',
      height: '70%',
    })
    expect(placementsFor(layout).Book).toEqual({
      left: '70%',
      top: '0%',
      width: '30%',
      height: '35%',
    })
  })

  it('omits a kind the layout does not name', () => {
    // The renderer keys `v-if` off this, so a missing entry is what stops a
    // panel being drawn somewhere arbitrary.
    const placements = placementsFor(layout)
    expect(placements.Tape).toBeUndefined()
    expect(placements.Footprint).toBeUndefined()
    expect(placements.Watchlist).toBeUndefined()
  })

  it('is empty for an empty layout', () => {
    expect(placementsFor([])).toEqual({})
  })

  it('places a panel a grid template could not', () => {
    // The whole reason for percentages over a CSS grid: this layout does not
    // decompose into rows and columns, and the previous fixed template could
    // not have expressed it.
    const overlapping: PanelSpec[] = [
      { kind: 'Chart', rect: { x: 10, y: 10, w: 55, h: 40 } },
      { kind: 'Tape', rect: { x: 40, y: 30, w: 55, h: 40 } },
    ]
    expect(placementsFor(overlapping)).toEqual({
      Chart: { left: '10%', top: '10%', width: '55%', height: '40%' },
      Tape: { left: '40%', top: '30%', width: '55%', height: '40%' },
    })
  })

  it('keeps the first when a kind is named twice', () => {
    // The core renders one view per panel kind, so a second placement would
    // have nothing to put in it.
    const duplicated: PanelSpec[] = [
      { kind: 'Chart', rect: { x: 0, y: 0, w: 50, h: 50 } },
      { kind: 'Chart', rect: { x: 50, y: 50, w: 50, h: 50 } },
    ]
    expect(placementsFor(duplicated).Chart).toEqual({
      left: '0%',
      top: '0%',
      width: '50%',
      height: '50%',
    })
  })
})
