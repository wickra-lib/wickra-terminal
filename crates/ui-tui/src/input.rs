//! Keymap: a key event plus the data-driven [`Keybinds`] map to an [`Action`].
//!
//! The keymap itself lives in the config (shared by both renderers), so this
//! module only turns a physical key into its config key-name and looks up the
//! bound action. That keeps key bindings data-driven rather than hard-coded.

use crossterm::event::{KeyCode, KeyEvent};
use wickra_terminal_core::Keybinds;

/// A resolved user intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    /// Quit the terminal.
    Quit,
    /// Focus the next panel.
    NextPanel,
    /// Focus the previous panel.
    PrevPanel,
    /// Open the source menu (add a source).
    SourceMenu,
    /// Prompt for a symbol to subscribe on the focused source.
    AddSymbol,
    /// Unsubscribe the focused symbol.
    RemoveSymbol,
    /// Remove the focused source and everything it owns.
    RemoveSource,
    /// Focus the next watched symbol.
    NextSymbol,
    /// Focus the previous watched symbol.
    PrevSymbol,
    /// Prompt for an indicator to add to every market.
    AddIndicator,
    /// Prompt for the label of an indicator to stop tracking.
    RemoveIndicator,
    /// Prompt for the bar size the candle indicators are fed at.
    SetTimeframe,
    /// Prompt for a filter over the registry catalogue.
    ListIndicators,
    /// Rewind a replayable source (the time-machine).
    SeekBack,
    /// Advance a replayable source.
    SeekForward,
    /// Scroll the focused panel towards the top of what it carries.
    ScrollUp,
    /// Scroll the focused panel towards the bottom.
    ScrollDown,
    /// Write the recorded events to a file.
    SaveRecording,
    /// Prompt for a panel to place on the layout.
    AddPanel,
    /// Take the focused panel off the layout.
    RemovePanel,
    /// Prompt for a new rectangle for the focused panel.
    MovePanel,
    /// No bound action for this key.
    None,
}

/// The config key-name for a physical key code (the spelling used in
/// [`Keybinds`]), or `None` for keys with no name.
#[must_use]
pub(crate) fn key_name(code: KeyCode) -> Option<String> {
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
pub(crate) fn map_key(key: KeyEvent, binds: &Keybinds) -> Action {
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
        Some("add_symbol") => Action::AddSymbol,
        Some("remove_symbol") => Action::RemoveSymbol,
        Some("remove_source") => Action::RemoveSource,
        Some("next_symbol") => Action::NextSymbol,
        Some("prev_symbol") => Action::PrevSymbol,
        Some("add_indicator") => Action::AddIndicator,
        Some("remove_indicator") => Action::RemoveIndicator,
        Some("set_timeframe") => Action::SetTimeframe,
        Some("list_indicators") => Action::ListIndicators,
        Some("seek_back") => Action::SeekBack,
        Some("seek_forward") => Action::SeekForward,
        Some("scroll_up") => Action::ScrollUp,
        Some("scroll_down") => Action::ScrollDown,
        Some("save_recording") => Action::SaveRecording,
        Some("add_panel") => Action::AddPanel,
        Some("remove_panel") => Action::RemovePanel,
        Some("move_panel") => Action::MovePanel,
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
    fn every_default_binding_resolves_to_an_action() {
        // Every arm of the keymap, driven from the default `Keybinds` rather
        // than a list written here: a binding added to the config and forgotten
        // in this match resolves to `Action::None`, which is a key that looks
        // bound and does nothing -- exactly what panel focus was for months.
        let binds = Keybinds::default();
        for (action, bound) in &binds.bindings {
            let code = match bound.as_str() {
                "tab" => KeyCode::Tab,
                "backtab" => KeyCode::BackTab,
                "left" => KeyCode::Left,
                "right" => KeyCode::Right,
                "up" => KeyCode::Up,
                "down" => KeyCode::Down,
                "enter" => KeyCode::Enter,
                "esc" => KeyCode::Esc,
                other => KeyCode::Char(
                    other
                        .chars()
                        .next()
                        .unwrap_or_else(|| panic!("{action} is bound to an empty key")),
                ),
            };
            assert_ne!(
                map_key(key(code), &binds),
                Action::None,
                "{action} is bound to {bound:?} and the keymap does not resolve it"
            );
        }
    }

    #[test]
    fn the_actions_added_for_the_registry_and_the_recording_resolve() {
        // Named individually as well, because the sweep above would still pass
        // if two of them resolved to each other's action.
        let binds = Keybinds::default();
        for (code, expected) in [
            (KeyCode::Char('i'), Action::AddIndicator),
            (KeyCode::Char('k'), Action::RemoveIndicator),
            (KeyCode::Char('t'), Action::SetTimeframe),
            (KeyCode::Char('l'), Action::ListIndicators),
            (KeyCode::Char(','), Action::SeekBack),
            (KeyCode::Char('.'), Action::SeekForward),
            (KeyCode::Up, Action::ScrollUp),
            (KeyCode::Down, Action::ScrollDown),
            (KeyCode::Char('w'), Action::SaveRecording),
            (KeyCode::Char('p'), Action::AddPanel),
            (KeyCode::Char('o'), Action::RemovePanel),
            (KeyCode::Char('m'), Action::MovePanel),
        ] {
            assert_eq!(map_key(key(code), &binds), expected, "for {code:?}");
        }
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
