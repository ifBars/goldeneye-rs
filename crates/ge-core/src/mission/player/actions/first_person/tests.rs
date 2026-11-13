use super::*;
use crate::mission::player::components::Registry;
struct Reader {
    calls: usize,
    found: bool,
}
impl Provider for Reader {
    fn resolve_mode(&mut self, r: u32) -> Result<Option<usize>> {
        self.calls += 1;
        Ok(self.found.then_some(usize::from(r == 0x0900_00aa)))
    }
    fn mode(&mut self, _: usize) -> Result<mode::Mode> {
        self.calls += 1;
        Ok(mode::Mode {
            hash: 0x0900_00ab,
            transition_count: 1,
        })
    }
    fn transition_target(&mut self, _: usize, _: usize) -> Result<u32> {
        self.calls += 1;
        Ok(1)
    }
    fn command_count(&mut self, _: usize, _: usize) -> Result<usize> {
        self.calls += 1;
        Ok(1)
    }
}
fn fixture() -> FirstPersonDispatch {
    let player = PlayerId::new(9).unwrap();
    let receiver = Registry::from_constructor(4).allocate(player).unwrap();
    FirstPersonDispatch {
        player,
        receiver,
        generic: 0x0900_00aa,
        requested: 0x0900_00aa,
        stage: FirstPersonStage::Selecting,
        fallback: Some((0x0900_065e, 4)),
        primary_head: Some((None, 0)),
        initial_heads_empty: true,
        selector: Some(Cursor::new(
            mode::Inputs {
                requested: 0x0900_00aa,
                previous: 0,
                fallback: 0x0900_065e,
                resource_available: true,
            },
            mode::Budget {
                lookups: 3,
                transition_rows: 4,
                commands: 4,
                groups: 4,
                children: 4,
            },
        )),
        command: None,
        completed_commands: Vec::new(),
        setter: None,
        coordinate_written: None,
    }
}
#[test]
fn fp_cursor_requires_real_body_return_before_acknowledgement_and_keeps_it_after_selection() {
    let mut fp = fixture();
    let mut source = Reader {
        calls: 0,
        found: true,
    };
    let invocation = fp.advance(&mut source).unwrap().unwrap();
    assert_eq!(invocation.ordinal, 0);
    assert_eq!(fp.stage(), FirstPersonStage::CommandPending);
    assert!(fp.acknowledge_command().is_err());
    fp.command = Some(command::Pending::new(0x0c00_0002));
    assert!(fp.acknowledge_command().is_err());
    // Authored caller boundary after the real command helper executes its
    // retained child effects. Native probes cover receiver/source binding.
    fp.command = Some(command::tests::returned_body());
    fp.acknowledge_command().unwrap();
    assert_eq!(fp.stage(), FirstPersonStage::Selecting);
    assert!(fp.command().is_none());
    assert_eq!(fp.completed_commands().len(), 1);
    assert_eq!(fp.advance(&mut source).unwrap(), None);
    assert_eq!(fp.stage(), FirstPersonStage::SetterTail);
    assert_eq!(fp.selector().unwrap().return_word(), Some(0x0900_00ab));
    assert_eq!(fp.command().unwrap().stage(), command::Stage::Returned);
    assert!(fp.command_acknowledged());
    assert!(fp.acknowledge_command().is_err());
    assert!(fp.setter().is_none());
    assert_eq!(fp.coordinate_written(), None);
}
#[test]
fn failed_fp_invocation_cannot_retry_provider_or_consume_later_commands() {
    let mut fp = fixture();
    let mut source = Reader {
        calls: 0,
        found: true,
    };
    fp.advance(&mut source).unwrap().unwrap();
    fp.fail().unwrap();
    let before = source.calls;
    assert_eq!(fp.stage(), FirstPersonStage::Failed);
    assert!(fp.advance(&mut source).is_err());
    assert_eq!(source.calls, before);
    assert!(fp.acknowledge_command().is_err());
    assert!(fp.fail().is_err());
    assert!(fp.completed_commands().is_empty());
}
#[test]
fn absent_fp_target_returns_zero_without_creating_commands_or_claiming_setter_completion() {
    let mut fp = fixture();
    let mut source = Reader {
        calls: 0,
        found: false,
    };
    assert_eq!(fp.advance(&mut source).unwrap(), None);
    assert_eq!(source.calls, 1);
    assert_eq!(fp.stage(), FirstPersonStage::SetterTail);
    assert_eq!(fp.selector().unwrap().return_word(), Some(0));
    assert!(fp.command().is_none());
    assert!(fp.completed_commands().is_empty());
    assert!(!fp.command_acknowledged());
    assert!(fp.setter().is_none());
}
