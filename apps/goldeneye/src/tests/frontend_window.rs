use super::*;
#[test]
fn native_key_adapter_selects_and_returns_from_unavailable_screen() {
    use ge_core::frontend::{Action, Screen};
    let mut menu = ge_core::frontend::State::new(false);
    for key in [NamedKey::ArrowDown, NamedKey::Enter] {
        menu.input(key_input(&Key::Named(key)).unwrap());
    }
    assert_eq!(menu.screen(), Screen::Unavailable(Action::SelectProfile));
    menu.input(key_input(&Key::Named(NamedKey::Escape)).unwrap());
    assert_eq!(menu.screen(), Screen::MainMenu);
    assert_eq!(menu.selected_index(), 2);
    assert_eq!(key_input(&Key::Character("W".into())), Some(Input::Up));
    assert_eq!(key_input(&Key::Character("s".into())), Some(Input::Down));
    assert_eq!(key_input(&Key::Named(NamedKey::Home)), None);
}
