use super::*;
use crate::mission::player::components::Registry;
struct Source;
impl Provider for Source {
    fn resolve_mode(&mut self, reference: u32) -> Result<Option<usize>> {
        Ok(Some(usize::from(reference == 0x0900_00aa)))
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
        Ok(2)
    }
}
fn call() -> Call {
    let player = PlayerId::new(4).unwrap();
    let receiver = Registry::from_constructor(2).allocate(player).unwrap();
    Call {
        inputs: Inputs {
            target: Target::ThirdPerson,
            receiver,
            generic: 0x0900_0765,
            requested: 0x0900_00aa,
            parameter_bits: f32::MAX.to_bits(),
            tracks: false,
            fallback: (0x0900_00cc, 5),
        },
        previous: 0,
        stage: Stage::Selecting,
        cursor: Cursor::new(
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
        ),
        command: None,
        completed: Vec::new(),
        setter: None,
    }
}
#[test]
fn next_pending_invocation_never_reuses_the_prior_completed_body() {
    let mut call = call();
    let first = call.advance(&mut Source).unwrap().unwrap();
    call.command = Some(command::tests::returned_body());
    call.acknowledge().unwrap();
    assert_eq!(call.completed_commands().len(), 1);
    let second = call.advance(&mut Source).unwrap().unwrap();
    assert_ne!(first, second);
    assert_eq!(call.stage(), Stage::CommandPending);
    assert!(call.command().is_none());
    assert!(!call.has_pending_command());
    assert_eq!(call.completed_commands().len(), 1);
    assert_eq!(call.selector().completed_commands(), 1);
}
#[test]
fn only_a_returned_body_can_acknowledge_the_real_pending_cursor() {
    let mut call = call();
    let invocation = call.advance(&mut Source).unwrap().unwrap();
    assert!(call.acknowledge().is_err());
    call.command = Some(command::Pending::new(0x0c00_0002));
    assert!(call.acknowledge().is_err());
    assert_eq!(call.selector().pending(), Some(invocation));
    assert_eq!(call.selector().completed_commands(), 0);
    assert!(call.completed_commands().is_empty());
    call.command = Some(command::tests::returned_body());
    call.acknowledge().unwrap();
    assert_eq!(call.selector().completed_commands(), 1);
    assert_eq!(call.stage(), Stage::Selecting);
    assert!(call.acknowledge().is_err());
}
