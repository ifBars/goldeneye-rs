use super::*;
use crate::mission::player::identity::PlayerId;
use std::collections::BTreeMap;

#[test]
fn repeated_source_clear_has_exact_boundaries_and_preserves_reset_completion() {
    let mut reset = PendingReset {
        player: PlayerId::new(6).unwrap(),
        bytes: BTreeMap::new(),
        stage: Stage::Complete,
        character: None,
    };
    for at in 0xfeb..=0x1150 {
        reset.bytes.insert(at, 0xa5);
    }
    reset.clear_component_entry().unwrap();
    assert_eq!(reset.byte(0xfeb), Some(0xa5));
    assert_eq!(reset.byte(0x1150), Some(0xa5));
    assert!((0xfec..0x1150).all(|at| reset.byte(at) == Some(0)));
    assert!(reset.completion().is_ok());
    reset.stage = Stage::TailInProgress;
    assert!(reset.clear_component_entry().is_err());
}
