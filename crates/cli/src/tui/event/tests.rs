use super::*;
use crossterm::event::{KeyEvent, MouseEvent, MouseEventKind};

#[test]
fn key_press_navigation_quit_resize_and_non_key_events_have_stable_actions() {
    for (code, expected) in [
        (KeyCode::Char('q'), Action::Quit),
        (KeyCode::Esc, Action::Quit),
        (KeyCode::Char('?'), Action::Help),
        (KeyCode::F(1), Action::Help),
        (KeyCode::Tab, Action::Next),
        (KeyCode::BackTab, Action::Previous),
        (KeyCode::Down, Action::Scroll(1)),
        (KeyCode::Char('j'), Action::Scroll(1)),
        (KeyCode::Up, Action::Scroll(-1)),
        (KeyCode::Char('k'), Action::Scroll(-1)),
        (KeyCode::PageDown, Action::Scroll(10)),
        (KeyCode::PageUp, Action::Scroll(-10)),
        (KeyCode::Left, Action::Horizontal(-1)),
        (KeyCode::Char('h'), Action::Horizontal(-1)),
        (KeyCode::Right, Action::Horizontal(1)),
        (KeyCode::Char('l'), Action::Horizontal(1)),
        (KeyCode::Home, Action::Home),
        (KeyCode::Char('r'), Action::Reload),
        (KeyCode::Char('x'), Action::None),
    ] {
        assert_eq!(
            action(&Event::Key(KeyEvent::new(code, KeyModifiers::NONE))),
            expected
        );
    }
    assert_eq!(
        action(&Event::Key(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL
        ))),
        Action::Interrupt
    );
    assert_eq!(
        action(&Event::Key(KeyEvent::new_with_kind(
            KeyCode::Char('q'),
            KeyModifiers::NONE,
            KeyEventKind::Release
        ))),
        Action::None
    );
    assert_eq!(action(&Event::Resize(1, 1)), Action::Resize);
    assert_eq!(action(&Event::FocusGained), Action::None);
    assert_eq!(
        action(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Moved,
            column: 1,
            row: 1,
            modifiers: KeyModifiers::NONE
        })),
        Action::None
    );
}
