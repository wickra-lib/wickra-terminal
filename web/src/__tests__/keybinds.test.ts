import { describe, expect, it } from 'vitest'

import { actionFor, isTyping, keyName, readKeybinds } from '../keybinds'

function press(key: string, mods: Partial<KeyboardEvent> = {}): KeyboardEvent {
  return { key, shiftKey: false, ctrlKey: false, metaKey: false, altKey: false, ...mods } as KeyboardEvent
}

const BINDS = { quit: 'q', next_symbol: 'right', prev_panel: 'backtab', seek_back: ',' }

describe('readKeybinds', () => {
  it('reads the bindings a config declares', () => {
    const binds = readKeybinds(
      JSON.stringify({ layout: { keybinds: { bindings: { quit: 'q' } } } }),
    )
    expect(binds.quit).toBe('q')
  })

  it('is empty for a config with no keymap, and for one that does not parse', () => {
    // The core fills in the default keymap itself, so an absent one is a valid
    // config; a renderer that threw here would fail to start over a shortcut.
    expect(readKeybinds(JSON.stringify({ sources: [] }))).toEqual({})
    expect(readKeybinds('not json')).toEqual({})
  })
})

describe('keyName', () => {
  it('uses the names the config writes, which are the TUI’s', () => {
    expect(keyName(press('ArrowRight'))).toBe('right')
    expect(keyName(press('Escape'))).toBe('esc')
    expect(keyName(press('Enter'))).toBe('enter')
    expect(keyName(press(','))).toBe(',')
  })

  it('reports Shift+Tab as backtab', () => {
    // crossterm reports it as its own key and the browser as Tab plus a
    // modifier; the shared config has to mean the same key in both.
    expect(keyName(press('Tab'))).toBe('tab')
    expect(keyName(press('Tab', { shiftKey: true }))).toBe('backtab')
  })

  it('lower-cases a printable key, as the TUI does', () => {
    expect(keyName(press('Q'))).toBe('q')
  })

  it('has no name for a key the config cannot spell', () => {
    expect(keyName(press('F5'))).toBeNull()
    expect(keyName(press('Shift'))).toBeNull()
  })
})

describe('actionFor', () => {
  it('resolves a bound key to its action', () => {
    expect(actionFor(press('q'), BINDS)).toBe('quit')
    expect(actionFor(press('ArrowRight'), BINDS)).toBe('next_symbol')
    expect(actionFor(press('Tab', { shiftKey: true }), BINDS)).toBe('prev_panel')
  })

  it('leaves an unbound key alone', () => {
    expect(actionFor(press('z'), BINDS)).toBeNull()
  })

  it('leaves the browser its own shortcuts', () => {
    // Ctrl+R is a reload and Cmd+L is the address bar. A terminal that stole
    // those would be a worse citizen than one with fewer shortcuts.
    expect(actionFor(press('q', { ctrlKey: true }), BINDS)).toBeNull()
    expect(actionFor(press('q', { metaKey: true }), BINDS)).toBeNull()
    expect(actionFor(press('q', { altKey: true }), BINDS)).toBeNull()
  })
})

describe('isTyping', () => {
  const element = (tagName: string, isContentEditable = false) =>
    ({ tagName, isContentEditable }) as unknown as EventTarget

  it('is true inside a field', () => {
    // Without this, typing ETH/USDT into the subscribe box fires the `t`
    // binding on its way past.
    expect(isTyping(element('INPUT'))).toBe(true)
    expect(isTyping(element('TEXTAREA'))).toBe(true)
    expect(isTyping(element('SELECT'))).toBe(true)
    expect(isTyping(element('DIV', true))).toBe(true)
  })

  it('is false at the page, and for a target that is not an element', () => {
    expect(isTyping(element('DIV'))).toBe(false)
    expect(isTyping(null)).toBe(false)
    expect(isTyping({} as EventTarget)).toBe(false)
  })
})
