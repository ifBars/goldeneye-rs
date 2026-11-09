use super::*;

#[test]
fn auxiliary_constructor_owns_four_initialized_records_but_extra_words_are_unknown() {
    let state = State::constructor();
    assert_eq!(state.stage(), Stage::Constructor);
    assert_eq!(state.radius_squared_bits(), 0);
    assert_eq!(state.extra_words(), &[None; 6]);
    assert_eq!(state.auxiliary_records().len(), 4);
    for auxiliary in state.auxiliary_records() {
        assert_eq!(auxiliary.words(), &[0; 6]);
        assert_eq!(auxiliary.flag_bytes(), [0; 2]);
        assert_eq!(auxiliary.padding(), [None; 2]);
    }
}

#[test]
fn post_common_reinitializes_existing_records_and_never_invents_padding_bytes() {
    let mut state = State::constructor();
    // Authored pre-reset contents make the actual second reset observable.
    for (index, auxiliary) in state.auxiliary_records.iter_mut().enumerate() {
        auxiliary.words = [index as u32 + 1; 6];
        auxiliary.flags = [7, 8];
    }
    state.initialize((-20i32) as u32);
    assert_eq!(state.stage(), Stage::PostCommon);
    assert_eq!(state.radius_squared_bits(), 4.0f32.to_bits());
    assert_eq!(state.extra_words(), &[Some(0); 6]);
    for auxiliary in state.auxiliary_records() {
        assert_eq!(auxiliary.words(), &[0; 6]);
        assert_eq!(auxiliary.flag_bytes(), [0; 2]);
        assert_eq!(auxiliary.padding(), [None; 2]);
    }
}
