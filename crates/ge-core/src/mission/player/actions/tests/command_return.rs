use super::*;
use crate::animation::{
    layers::command,
    mode::{
        self,
        runtime::{Cursor, Provider},
    },
};
struct Source;
impl Provider for Source {
    fn resolve_mode(&mut self, r: u32) -> Result<Option<usize>> {
        Ok(Some(if r == 0x0900_00aa { 1 } else { 0 }))
    }
    fn mode(&mut self, _: usize) -> Result<mode::Mode> {
        Ok(mode::Mode {
            hash: 0x0900_00ab,
            transition_count: 1,
        })
    }
    fn transition_target(&mut self, _: usize, _: usize) -> Result<u32> {
        Ok(1)
    }
    fn command_count(&mut self, _: usize, _: usize) -> Result<usize> {
        Ok(1)
    }
}
#[test]
fn only_a_returned_body_can_acknowledge_the_pending_invocation_and_finish_selection() {
    let mut cursor = Cursor::new(
        mode::Inputs {
            requested: 0x0900_00aa,
            previous: 0,
            fallback: 0x0900_00cc,
            resource_available: true,
        },
        mode::Budget {
            lookups: 3,
            transition_rows: 4,
            commands: 4,
            groups: 4,
            children: 4,
        },
    );
    cursor.next_command(&mut Source).unwrap().unwrap();
    // Authored caller boundary. The body helper executes real retained child
    // effects; native app probes cover the earlier identity/component bridge.
    let mut p = Pending {
        player: PlayerId::new(4).unwrap(),
        stage: Stage::AwaitingThirdPersonCommand,
        generic: 0x0900_00aa,
        components: None,
        weapon: None,
        variant: None,
        resolved: Vec::new(),
        fallback: None,
        fallback_writes: None,
        selector: Some(cursor),
        tracks_player_modes: None,
        primary_head: None,
        command: Some(command::Pending::new(0x0c00_0002)),
        completed_commands: Vec::new(),
    };
    assert!(p.acknowledge_third_person_command().is_err());
    assert_eq!(p.selector().unwrap().completed_commands(), 0);
    p.command = Some(command::tests::returned_body());
    p.acknowledge_third_person_command().unwrap();
    assert_eq!(p.completed_commands().len(), 1);
    assert_eq!(p.stage(), Stage::SelectingThirdPerson);
    assert_eq!(p.advance_third_person_selector(&mut Source).unwrap(), None);
    assert_eq!(p.stage(), Stage::AwaitingThirdPersonSetterTail);
    assert_eq!(p.selector().unwrap().return_word(), Some(0x0900_00ab));
    assert_eq!(p.command().unwrap().stage(), command::Stage::Returned);
    assert!(p.command_acknowledged());
    assert!(p.acknowledge_third_person_command().is_err());
    assert_eq!(p.completed_commands().len(), 1);
}
