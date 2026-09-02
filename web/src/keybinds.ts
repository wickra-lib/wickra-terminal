// The keymap, kept out of the SFC so it can be tested without a browser — the
// same reason ./layout, ./profile and ./indicator are modules.
//
// `Keybinds` lives in the config expressly so both renderers share one keymap:
// rebind `next_symbol` once and the TUI and the browser both move. The browser
// half was never written, so that sentence was true of the config and false of
// the product. This is the half that was missing.

/** action name -> key name, as the config writes it. */
export type Keybinds = Record<string, string>

/** The keymap a config declares, or an empty one if it declares none. */
export function readKeybinds(configJson: string): Keybinds {
  try {
    const parsed = JSON.parse(configJson) as {
      layout?: { keybinds?: { bindings?: Keybinds } }
    }
    return parsed.layout?.keybinds?.bindings ?? {}
  } catch {
    return {}
  }
}

/**
 * The config key-name for a browser `KeyboardEvent.key`.
 *
 * The names are the TUI's, because the config is shared: a `Keybinds` entry
 * reading `"backtab"` has to mean the same key in both renderers. Shift+Tab is
 * `backtab` for exactly that reason — crossterm reports it as its own key, and
 * the browser reports Tab with a shift modifier.
 *
 * Returns `null` for a key with no config name, which is most of them.
 */
export function keyName(event: KeyboardEvent): string | null {
  const key = event.key
  if (key === 'Tab') {
    return event.shiftKey ? 'backtab' : 'tab'
  }
  const named: Record<string, string> = {
    ArrowLeft: 'left',
    ArrowRight: 'right',
    ArrowUp: 'up',
    ArrowDown: 'down',
    Enter: 'enter',
    Escape: 'esc',
  }
  if (key in named) {
    return named[key]
  }
  // A single printable character, lower-cased the way the TUI lower-cases it, so
  // a binding on `q` also fires on `Q`.
  return key.length === 1 ? key.toLowerCase() : null
}

/**
 * Resolve a key event to the action bound to it, or `null`.
 *
 * A modifier other than shift means the key belongs to the browser — Ctrl+R is
 * a reload, Cmd+L is the address bar — and a terminal that stole those would be
 * a worse citizen than one with fewer shortcuts.
 */
export function actionFor(event: KeyboardEvent, binds: Keybinds): string | null {
  if (event.ctrlKey || event.metaKey || event.altKey) {
    return null
  }
  const name = keyName(event)
  if (name === null) {
    return null
  }
  const hit = Object.entries(binds).find(([, bound]) => bound === name)
  return hit ? hit[0] : null
}

/**
 * Whether a key event was typed into a field rather than at the terminal.
 *
 * Without this, typing `ETH/USDT` into the subscribe box would fire the `t`
 * binding on its way past — the shortcut layer has to end where text entry
 * begins.
 */
export function isTyping(target: EventTarget | null): boolean {
  // Duck-typed rather than `instanceof HTMLElement`: that name only exists in a
  // browser, so the check would throw wherever this module is loaded without
  // one -- a test runner, a server render -- for a question that has a perfectly
  // good answer there ("no").
  const element = target as { tagName?: unknown; isContentEditable?: unknown } | null
  if (element === null || typeof element.tagName !== 'string') {
    return false
  }
  const tag = element.tagName.toUpperCase()
  return (
    tag === 'INPUT' ||
    tag === 'TEXTAREA' ||
    tag === 'SELECT' ||
    element.isContentEditable === true
  )
}
