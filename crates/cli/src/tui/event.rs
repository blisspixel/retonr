//! Keyboard mapping independent of terminal ownership.

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Action {
    Quit,
    Interrupt,
    Help,
    Next,
    Previous,
    Scroll(i16),
    Horizontal(i16),
    Home,
    Reload,
    Resize,
    None,
}

pub(super) fn action(event: &Event) -> Action {
    match event {
        Event::Resize(_, _) => Action::Resize,
        Event::Key(key) if key.kind != KeyEventKind::Release => {
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                return Action::Interrupt;
            }
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
                KeyCode::Char('?') | KeyCode::F(1) => Action::Help,
                KeyCode::Tab => Action::Next,
                KeyCode::BackTab => Action::Previous,
                KeyCode::Down | KeyCode::Char('j') => Action::Scroll(1),
                KeyCode::Up | KeyCode::Char('k') => Action::Scroll(-1),
                KeyCode::PageDown => Action::Scroll(10),
                KeyCode::PageUp => Action::Scroll(-10),
                KeyCode::Right | KeyCode::Char('l') => Action::Horizontal(1),
                KeyCode::Left | KeyCode::Char('h') => Action::Horizontal(-1),
                KeyCode::Home => Action::Home,
                KeyCode::Char('r') => Action::Reload,
                _ => Action::None,
            }
        }
        _ => Action::None,
    }
}

#[cfg(test)]
mod tests;
