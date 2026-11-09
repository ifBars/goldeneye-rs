use super::*;
use crate::mission::{Difficulty, session::NewGameRequest};

#[test]
fn new_game_opens_source_tutorial_difficulties_without_dispatching_a_load() {
    let mut state = State::new(false);
    assert_eq!(state.take_command(), None);
    open(&mut state, 0, Screen::NewGameDifficulty);
    assert_eq!(state.title(), "Select Difficulty");
    let items = state.items();
    assert_eq!(
        items.iter().map(|row| row.label).collect::<Vec<_>>(),
        ["Operative", "Agent", "007", "007 Classic"]
    );
    assert!(items.iter().all(|row| row.enabled && row.value.is_none()));
    assert_eq!(state.take_command(), None);
    assert!(!state.select_index(4));
    assert_eq!(state.input(Input::Left), Effect::None);
    assert_eq!(state.input(Input::Right), Effect::None);
}

#[test]
fn every_difficulty_dispatches_its_typed_tutorial_request_exactly_once() {
    for (index, difficulty) in [
        Difficulty::Operative,
        Difficulty::Agent,
        Difficulty::DoubleOSeven,
        Difficulty::Classic,
    ]
    .into_iter()
    .enumerate()
    {
        let mut state = State::new(false);
        open(&mut state, 0, Screen::NewGameDifficulty);
        assert!(state.select_index(index));
        assert_eq!(state.items()[index].action.difficulty(), Some(difficulty));
        assert_eq!(state.input(Input::Confirm), Effect::Accept);
        assert_eq!(state.input(Input::Confirm), Effect::None);
        let Some(Command::NewGame(request)) = state.take_command() else {
            panic!("difficulty did not produce a typed New Game request")
        };
        assert_eq!(request, NewGameRequest::new(difficulty));
        assert_eq!(request.difficulty().value(), index as u8);
        assert_eq!(request.level_hash(), 0x0181_016b);
        assert_eq!(request.start_point_id(), 1);
        assert_eq!(request.local_user_count(), 1);
        assert_eq!(state.take_command(), None);
        assert_eq!(state.screen(), Screen::NewGameDifficulty);
        assert_eq!(state.selected_index(), index);
        assert!(state.items().iter().all(|row| !row.enabled));
        for input in [
            Input::Confirm,
            Input::Up,
            Input::Down,
            Input::Left,
            Input::Right,
            Input::Reset,
        ] {
            assert_eq!(state.input(input), Effect::None);
        }
        assert!(!state.select_index(index));
        assert_eq!(state.take_command(), None);
    }
}

#[test]
fn keyboard_and_pointer_select_the_same_source_difficulty_request() {
    let mut keyboard = State::new(false);
    open(&mut keyboard, 0, Screen::NewGameDifficulty);
    assert_eq!(keyboard.input(Input::Up), Effect::Navigate);
    assert_eq!(keyboard.selected_index(), 3);
    assert_eq!(keyboard.input(Input::Down), Effect::Navigate);
    assert_eq!(keyboard.selected_index(), 0);
    assert_eq!(keyboard.input(Input::Down), Effect::Navigate);
    assert_eq!(keyboard.selected_index(), 1);
    assert_eq!(keyboard.input(Input::Confirm), Effect::Accept);

    let mut pointer = State::new(false);
    open(&mut pointer, 0, Screen::NewGameDifficulty);
    assert!(pointer.select_index(1));
    assert!(pointer.select_index(1));
    assert_eq!(pointer.input(Input::Confirm), Effect::Accept);
    assert_eq!(pointer.take_command(), keyboard.take_command());
    assert_eq!(pointer.take_command(), None);
    assert_eq!(keyboard.take_command(), None);
}

#[test]
fn back_cancels_pending_work_and_restores_main_selection_and_settings() {
    let settings = Settings::from_values(20, 40, 60, 80).unwrap();
    let mut state = State::with_settings(false, settings);
    open(&mut state, 0, Screen::NewGameDifficulty);
    assert!(state.select_index(2));
    assert_eq!(state.input(Input::Confirm), Effect::Accept);
    assert_eq!(state.input(Input::Back), Effect::Back);
    assert_eq!(state.screen(), Screen::MainMenu);
    assert_eq!(state.selected_index(), 0);
    assert_eq!(state.take_command(), None);
    assert_eq!(*state.settings(), settings);
    open(&mut state, 0, Screen::NewGameDifficulty);
    assert_eq!(state.selected_index(), 0);
    assert!(state.select_index(3));
    assert_eq!(state.input(Input::Confirm), Effect::Accept);
    assert_eq!(
        state.take_command(),
        Some(Command::NewGame(NewGameRequest::new(Difficulty::Classic)))
    );
}

#[test]
fn consumed_request_does_not_rearm_until_difficulty_page_is_left() {
    let mut state = State::new(false);
    open(&mut state, 0, Screen::NewGameDifficulty);
    assert_eq!(state.input(Input::Confirm), Effect::Accept);
    assert!(state.take_command().is_some());
    for _ in 0..3 {
        assert_eq!(state.input(Input::Confirm), Effect::None);
        assert_eq!(state.take_command(), None);
    }
    assert_eq!(state.input(Input::Back), Effect::Back);
    open(&mut state, 0, Screen::NewGameDifficulty);
    assert_eq!(state.input(Input::Confirm), Effect::Accept);
    assert!(state.take_command().is_some());
}
