use super::*;

mod new_game;

fn open(state: &mut State, index: usize, screen: Screen) {
    assert!(state.select_index(index));
    assert_eq!(state.input(Input::Confirm), Effect::Accept);
    assert_eq!(state.screen(), screen);
    assert_eq!(state.selected_index(), 0);
}

#[test]
fn main_menu_keeps_source_order_and_explicit_save_availability() {
    let state = State::new(false);
    assert_eq!(state.screen(), Screen::MainMenu);
    assert_eq!(state.selected_index(), 0);
    let items = state.items();
    assert_eq!(
        items.iter().map(|item| item.label).collect::<Vec<_>>(),
        [
            "New Game",
            "Continue Game",
            "Select Profile",
            "Mission Select",
            "Multiplayer",
            "Options",
            "Extras"
        ]
    );
    assert!(!items[1].enabled);
    assert!(
        items
            .iter()
            .enumerate()
            .all(|(index, item)| index == 1 || item.enabled)
    );
    assert!(State::new(true).items()[1].enabled);
}

#[test]
fn navigation_wraps_skips_disabled_and_emits_only_actual_changes() {
    let mut state = State::new(false);
    assert_eq!(state.input(Input::Down), Effect::Navigate);
    assert_eq!(state.selected_index(), 2);
    assert_eq!(state.input(Input::Up), Effect::Navigate);
    assert_eq!(state.selected_index(), 0);
    assert_eq!(state.input(Input::Up), Effect::Navigate);
    assert_eq!(state.selected_index(), 6);
    assert_eq!(state.input(Input::Down), Effect::Navigate);
    assert_eq!(state.selected_index(), 0);
    let before = state.clone();
    assert_eq!(state.input(Input::Back), Effect::None);
    assert_eq!(state.input(Input::Left), Effect::None);
    assert_eq!(state.input(Input::Right), Effect::None);
    assert!(!state.select_index(1));
    assert!(!state.select_index(7));
    assert!(!state.select_index(usize::MAX));
    assert!(state.select_index(0));
    assert_eq!(state, before);
}

#[test]
fn unavailable_gameplay_restores_selection_and_rejects_other_input() {
    let mut state = State::new(true);
    open(&mut state, 1, Screen::Unavailable(Action::ContinueGame));
    let before = state.clone();
    for input in [
        Input::Up,
        Input::Down,
        Input::Left,
        Input::Right,
        Input::Confirm,
    ] {
        assert_eq!(state.input(input), Effect::None);
    }
    assert!(!state.select_index(0));
    assert_eq!(state, before);
    assert_eq!(state.input(Input::Back), Effect::Back);
    assert_eq!(state.screen(), Screen::MainMenu);
    assert_eq!(state.selected_index(), 1);
}

#[test]
fn options_and_sound_preserve_each_parent_selection_and_live_values() {
    let mut state = State::new(false);
    open(&mut state, 5, Screen::Options);
    assert_eq!(
        state
            .items()
            .iter()
            .map(|item| item.label)
            .collect::<Vec<_>>(),
        ["Control Settings", "Screen Settings", "Sound Settings"]
    );
    open(&mut state, 2, Screen::SoundSettings);
    assert_eq!(
        state
            .items()
            .iter()
            .map(|item| item.label)
            .collect::<Vec<_>>(),
        ["Music Volume", "Sound Volume", "Dialog Volume"]
    );
    let initial = state.settings().music_volume();
    assert_eq!(state.input(Input::Left), Effect::Adjust);
    assert_eq!(state.settings().music_volume(), initial - 4);
    assert_eq!(state.items()[0].value, Some(initial - 4));
    assert_eq!(state.input(Input::Confirm), Effect::None);
    assert_eq!(state.input(Input::Back), Effect::Back);
    assert_eq!(state.screen(), Screen::Options);
    assert_eq!(state.selected_index(), 2);
    assert_eq!(state.input(Input::Back), Effect::Back);
    assert_eq!(state.screen(), Screen::MainMenu);
    assert_eq!(state.selected_index(), 5);
    open(&mut state, 5, Screen::Options);
    open(&mut state, 2, Screen::SoundSettings);
    assert_eq!(state.items()[0].value, Some(initial - 4));
}

#[test]
fn sound_boundaries_are_silent_and_selected_channel_only_changes() {
    let mut state = State::new(false);
    open(&mut state, 5, Screen::Options);
    open(&mut state, 2, Screen::SoundSettings);
    assert!(state.select_index(1));
    let before = *state.settings();
    assert_eq!(state.input(Input::Right), Effect::None);
    assert_eq!(*state.settings(), before);
    assert_eq!(state.input(Input::Left), Effect::Adjust);
    assert_eq!(state.settings().sound_volume(), 96);
    assert_eq!(state.settings().music_volume(), before.music_volume());
    assert_eq!(state.settings().dialog_volume(), before.dialog_volume());
    for _ in 0..24 {
        state.input(Input::Left);
    }
    assert_eq!(state.settings().sound_volume(), 0);
    assert_eq!(state.input(Input::Left), Effect::None);
}

#[test]
fn single_brightness_row_does_not_fake_navigation_or_confirmation() {
    let mut state = State::new(false);
    open(&mut state, 5, Screen::Options);
    open(&mut state, 1, Screen::ScreenSettings);
    assert_eq!(state.items().len(), 1);
    assert_eq!(state.items()[0].action, Action::Brightness);
    assert_eq!(state.input(Input::Up), Effect::None);
    assert_eq!(state.input(Input::Down), Effect::None);
    assert_eq!(state.input(Input::Confirm), Effect::None);
    let before = *state.settings();
    assert_eq!(state.input(Input::Right), Effect::Adjust);
    assert!(state.settings().brightness() > before.brightness());
    assert_eq!(state.settings().music_volume(), before.music_volume());
}

#[test]
fn controls_and_extras_expose_source_rows_without_fabricating_runtime() {
    let mut state = State::new(false);
    open(&mut state, 5, Screen::Options);
    open(&mut state, 0, Screen::ControlSettings);
    assert_eq!(
        state
            .items()
            .iter()
            .map(|item| item.label)
            .collect::<Vec<_>>(),
        [
            "Button Layout",
            "Wii Remote Presets",
            "Controller Settings",
            "ADS Snap"
        ]
    );
    open(&mut state, 3, Screen::Unavailable(Action::AdsSnap));
    assert_eq!(state.input(Input::Back), Effect::Back);
    assert_eq!(state.screen(), Screen::ControlSettings);
    assert_eq!(state.selected_index(), 3);
    state.input(Input::Back);
    state.input(Input::Back);
    open(&mut state, 6, Screen::Extras);
    assert_eq!(
        state
            .items()
            .iter()
            .map(|item| item.label)
            .collect::<Vec<_>>(),
        ["Credits", "Cheat Codes"]
    );
    open(&mut state, 1, Screen::Unavailable(Action::CheatCodes));
    state.input(Input::Back);
    assert_eq!(state.screen(), Screen::Extras);
    assert_eq!(state.selected_index(), 1);
    state.input(Input::Back);
    assert_eq!(state.screen(), Screen::MainMenu);
    assert_eq!(state.selected_index(), 6);
}

#[test]
fn resetting_a_page_preserves_other_settings_and_selection() {
    let settings = Settings::from_values(20, 40, 60, 80).unwrap();
    let mut state = State::with_settings(false, settings);
    assert_eq!(state.input(Input::Reset), Effect::None);
    open(&mut state, 5, Screen::Options);
    open(&mut state, 2, Screen::SoundSettings);
    assert!(state.select_index(2));
    assert_eq!(state.input(Input::Reset), Effect::Adjust);
    assert_eq!(state.settings().music_volume(), 66);
    assert_eq!(state.settings().sound_volume(), 100);
    assert_eq!(state.settings().dialog_volume(), 100);
    assert_eq!(state.settings().brightness(), 80);
    assert_eq!(state.selected_index(), 2);
    assert_eq!(state.input(Input::Reset), Effect::None);
    state.input(Input::Back);
    open(&mut state, 1, Screen::ScreenSettings);
    assert_eq!(state.input(Input::Reset), Effect::Adjust);
    assert_eq!(*state.settings(), Settings::default());
    assert_eq!(state.input(Input::Reset), Effect::None);
}
