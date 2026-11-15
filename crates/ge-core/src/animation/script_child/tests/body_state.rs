use super::*;
use crate::animation::script_child::{forward_tick, update};

struct PendingBody(Source);
impl Provider for PendingBody {
    fn file(&self) -> u32 {
        self.0.file()
    }
    fn generation(&mut self) -> Result<u32> {
        self.0.generation()
    }
    fn resolve_script(&mut self, reference: u32) -> Result<Option<Binding>> {
        self.0.resolve_script(reference)
    }
    fn bind_body(&mut self, _: Binding) -> Result<bool> {
        panic!("unknown section must not be converted to a known body boolean")
    }
    fn body_state(&mut self, _: Binding) -> Result<BodyState> {
        Ok(BodyState::Pending)
    }
}
impl forward_tick::Source for PendingBody {
    fn duration(&mut self, _: Binding) -> Result<u32> {
        panic!("pending body duration")
    }
    fn fps_bits(&mut self, _: Binding) -> Result<u32> {
        panic!("pending body rate")
    }
}

#[test]
fn pending_body_retains_actual_descriptor_without_claiming_null_or_factory_return() {
    let (mut store, _, id) = fixture();
    store.bind_factory_context(id, 0, 3).unwrap();
    store
        .initialize_prefix(id, &mut PendingBody(source()))
        .unwrap();
    let header = store.get(id).unwrap();
    assert_eq!(header.stage(), Stage::AwaitingScriptBody);
    assert_eq!(header.cached().unwrap().directory_index, 2);
    assert_eq!(header.observed_generation(), Some(0));
    assert_eq!(header.body_bound(), None);
    assert!(!header.initializer_returned());
    assert!(header.initialization().is_none());
}

#[test]
fn pending_update_body_cannot_advance_to_rate_or_claim_return() {
    let (mut store, _, id) = fixture();
    store.bind_factory_context(id, 0, 3).unwrap();
    let mut initial = source();
    initial.body = false;
    store.initialize_prefix(id, &mut initial).unwrap();
    store
        .begin_initflag_update(id, &mut PendingBody(source()))
        .unwrap();
    let header = store.get(id).unwrap();
    assert_eq!(header.stage(), Stage::ReturnedWithoutBody);
    let trace = &header.updates()[0];
    assert_eq!(trace.stage, update::Stage::Pending);
    assert_eq!(trace.boundary, Some(Stage::AwaitingScriptBody));
    assert_eq!(trace.body_bound, None);
    assert!(trace.rate.is_none());
}

#[test]
fn pending_forward_body_keeps_reached_phase_clear_but_does_not_read_rate() {
    let (mut store, _, id) = fixture();
    store.bind_factory_context(id, 0, 3).unwrap();
    let mut initial = source();
    initial.body = false;
    store.initialize_prefix(id, &mut initial).unwrap();
    let mut trace = forward_tick::Trace::default();
    store
        .forward_zero(id, &mut PendingBody(source()), &mut trace)
        .unwrap();
    assert_eq!(trace.stage(), forward_tick::Stage::AwaitingBody);
    assert_eq!(trace.body_bound(), None);
    assert_eq!(trace.duration_raw(), None);
    assert_eq!(trace.result_bits(), None);
    assert_eq!(store.get(id).unwrap().word(0x84), Some(0));
}
