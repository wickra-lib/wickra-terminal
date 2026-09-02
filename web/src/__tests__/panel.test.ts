import { describe, expect, it } from 'vitest'
import { PANEL_KINDS, parsePanel } from '../panel'

describe('parsePanel', () => {
  it('reads a kind and a rectangle', () => {
    expect(parsePanel('Book 70 0 30 35')).toEqual({
      kind: 'Book',
      rect: { x: 70, y: 0, w: 30, h: 35 },
    })
  })

  it('is case-insensitive on the kind, because a person types it', () => {
    expect(parsePanel('tape 0 0 100 100')?.kind).toBe('Tape')
  })

  it('takes a depth after the rectangle', () => {
    expect(parsePanel('Tape 0 70 100 30 48')?.depth).toBe(48)
    // Zero carries no rows, which is not what leaving it out means.
    expect(parsePanel('Tape 0 0 10 10 0')).toBeNull()
  })

  it('refuses a rectangle that leaves the grid', () => {
    // A typo every time. Accepting it would leave a panel the config says is
    // one size and the screen says is another.
    expect(parsePanel('Book 70 0 40 35')).toBeNull()
    expect(parsePanel('Book 0 70 100 40')).toBeNull()
    expect(parsePanel('Book 0 0 0 50')).toBeNull()
  })

  it('refuses an unknown kind and a malformed rectangle', () => {
    expect(parsePanel('Ladder 0 0 10 10')).toBeNull()
    expect(parsePanel('Book 1 2 3')).toBeNull()
    expect(parsePanel('Book 1 2 3 4 5 6')).toBeNull()
    expect(parsePanel('Book 0 0 x 50')).toBeNull()
    expect(parsePanel('')).toBeNull()
  })

  it('knows every kind the core can build', () => {
    // Seven, and the same seven the layout places. A kind added to the core and
    // forgotten here is a panel the browser cannot ask for.
    expect(PANEL_KINDS).toHaveLength(7)
    for (const kind of PANEL_KINDS) {
      expect(parsePanel(`${kind} 0 0 10 10`)?.kind).toBe(kind)
    }
  })
})
