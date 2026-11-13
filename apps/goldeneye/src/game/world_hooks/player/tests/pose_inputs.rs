use crate::game::world_hooks::Shared;

#[test]
fn constructor_vectors_have_distinct_owners_and_survive_attempt_cancellation() {
    let shared = Shared::from_constructor();
    let old = shared.begin(1).unwrap();
    assert_eq!(old.player_reset_quad_bits().unwrap(), [0; 4]);
    assert_eq!(old.player_pose_ones_bits().unwrap(), [1f32.to_bits(); 3]);
    assert_eq!(old.player_pose_weapon_byte().unwrap(), None);
    shared.invalidate();
    let current = shared.begin(2).unwrap();
    assert!(old.player_pose_ones_bits().is_err());
    assert!(old.player_pose_weapon_byte().is_err());
    assert_eq!(
        current.player_pose_ones_bits().unwrap(),
        [1f32.to_bits(); 3]
    );
    assert_eq!(current.player_pose_weapon_byte().unwrap(), None);
    let state = current.shared.0.state.lock().unwrap();
    assert_eq!(state.player.math_defaults.reset_quad_reads, 1);
    assert_eq!(state.player.math_defaults.ones_reads, 2);
    assert_eq!(state.player.pose_globals.weapon_reads, 2);
}
