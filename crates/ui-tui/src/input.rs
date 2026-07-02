//! Keymap: a key event plus the data-driven [`Keybinds`] map to an [`Action`].
//!
//! The keymap itself lives in the config (shared by both renderers), so this
//! module only turns a physical key into its config key-name and looks up the
//! bound action. That keeps key bindings data-driven rather than hard-coded.

use crossterm::event::{KeyCode, KeyEvent};
use terminal_core::Keybinds;

/// A resolved user intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Quit the terminal.
    Quit,
    /// Focus the next panel.
    NextPanel,
    /// Focus the previous panel.
    PrevPanel,
    /// Open the source menu (add/remove a source).
    SourceMenu,
    /// Focus the next watched symbol.
    NextSymbol,
    /// Focus the previous watched symbol.
    PrevSymbol,
    /// No bound action for this key.
    None,
}

/// The config key-name for a physical key code (the spelling used in
/// [`Keybinds`]), or `None` for keys with no name.
#[must_use]
pub fn key_name(code: KeyCode) -> Option<String> {
    match code {
        KeyCode::Char(c) => Some(c.to_ascii_lowercase().to_string()),
        KeyCode::Tab => Some("tab".to_string()),
        KeyCode::BackTab => Some("backtab".to_string()),
        KeyCode::Left => Some("left".to_string()),
        KeyCode::Right => Some("right".to_string()),
        KeyCode::Up => Some("up".to_string()),
        KeyCode::Down => Some("down".to_string()),
        KeyCode::Enter => Some("enter".to_string()),
        KeyCode::Esc => Some("esc".to_string()),
        _ => None,
    }
}

/// Resolve a key event to an action using the config keymap.
#[must_use]
pub fn map_key(key: KeyEvent, binds: &Keybinds) -> Action {
    let Some(name) = key_name(key.code) else {
        return Action::None;
    };
    let action = binds
        .bindings
        .iter()
        .find_map(|(action, bound)| (bound == &name).then_some(action.as_str()));
    match action {
        Some("quit") => Action::Quit,
        Some("next_panel") => Action::NextPanel,
        Some("prev_panel") => Action::PrevPanel,
        Some("source_menu") => Action::SourceMenu,
        Some("next_symbol") => Action::NextSymbol,
        Some("prev_symbol") => Action::PrevSymbol,
        _ => Action::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn default_keymap_resolves_quit_and_navigation() {
        let binds = Keybinds::default();
        assert_eq!(map_key(key(KeyCode::Char('q')), &binds), Action::Quit);
        assert_eq!(map_key(key(KeyCode::Tab), &binds), Action::NextPanel);
        assert_eq!(map_key(key(KeyCode::BackTab), &binds), Action::PrevPanel);
        assert_eq!(map_key(key(KeyCode::Char('s')), &binds), Action::SourceMenu);
        assert_eq!(map_key(key(KeyCode::Right), &binds), Action::NextSymbol);
        assert_eq!(map_key(key(KeyCode::Left), &binds), Action::PrevSymbol);
    }

    #[test]
    fn uppercase_maps_like_lowercase() {
        let binds = Keybinds::default();
        assert_eq!(map_key(key(KeyCode::Char('Q')), &binds), Action::Quit);
    }

    #[test]
    fn unbound_key_is_none() {
        let binds = Keybinds::default();
        assert_eq!(map_key(key(KeyCode::Char('z')), &binds), Action::None);
        assert_eq!(map_key(key(KeyCode::F(5)), &binds), Action::None);
    }

    #[test]
    fn key_event_kind_is_available() {
        // Sanity: the loop filters on Press; ensure the type is in scope.
        let ev = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(ev.kind, KeyEventKind::Press);
    }
}
