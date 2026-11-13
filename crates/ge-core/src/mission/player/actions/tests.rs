use super::*;
use crate::mission::player::components::Registry;
mod command_return;

#[test]
fn remapping_preserves_fp_tp_secondary_order_and_rejects_wrong_receivers_or_replay() {
    let player = PlayerId::new(3).unwrap();
    let mut registry = Registry::from_constructor(2);
    let tp = registry.allocate(player).unwrap();
    let fp = registry.allocate(player).unwrap();
    // Authored boundary after actual component/inventory lookup. App integration
    // exercises that earlier chain using real retained constructor owners.
    let mut pending = Pending {
        player,
        stage: Stage::Remapping,
        generic: 0x0900_00ab,
        parameter_bits: 0,
        components: Some([fp, tp]),
        weapon: Some(0x0200_00ab),
        variant: Some(1),
        resolved: Vec::new(),
        fallback: None,
        fallback_writes: None,
        selector: None,
        tracks_player_modes: None,
        primary_head: None,
        command: None,
        completed_commands: Vec::new(),
        dispatch: None,
        first_person: None,
        suffix: None,
    };
    for (ordinal, target, component) in [
        (0, Target::FirstPerson, fp),
        (1, Target::ThirdPerson, tp),
        (2, Target::ThirdPersonSecondary, tp),
    ] {
        let request = pending.next_remap().unwrap().unwrap();
        assert_eq!(
            (request.ordinal, request.target, request.component),
            (ordinal, target, component)
        );
        let wrong = Request {
            component: if component == tp { fp } else { tp },
            ..request
        };
        assert!(pending.mapped(wrong, 5, true).is_err());
        assert_eq!(pending.resolved().len(), ordinal);
        pending
            .mapped(request, 0x0900_00ac + ordinal as u32, true)
            .unwrap();
        assert!(pending.mapped(request, 5, true).is_err());
    }
    assert_eq!(pending.stage(), Stage::AwaitingThirdPersonDispatch);
    assert_eq!(pending.next_remap().unwrap(), None);
    assert_eq!(pending.resolved().len(), 3);
}
