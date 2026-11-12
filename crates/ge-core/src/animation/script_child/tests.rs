use super::*;
struct Source {
    file: u32,
    generation: u32,
    found: bool,
    body: bool,
    fail: Option<&'static str>,
    calls: Vec<&'static str>,
}
impl Source {
    fn read(&mut self, at: &'static str) -> Result<()> {
        self.calls.push(at);
        ensure!(self.fail != Some(at), "authored failure at {at}");
        Ok(())
    }
}
impl Provider for Source {
    fn file(&self) -> u32 {
        self.file
    }
    fn generation(&mut self) -> Result<u32> {
        self.read("generation")?;
        Ok(self.generation)
    }
    fn resolve_script(&mut self, _: u32) -> Result<Option<Binding>> {
        self.read("resolve")?;
        Ok(self.found.then_some(Binding {
            directory_index: 2,
            hash: 0x0400_00ab,
        }))
    }
    fn bind_body(&mut self, _: Binding) -> Result<bool> {
        self.read("body")?;
        Ok(self.body)
    }
}
fn source() -> Source {
    Source {
        file: 0x0102_00ab,
        generation: 0,
        found: true,
        body: true,
        fail: None,
        calls: Vec::new(),
    }
}
fn fixture() -> (Store, Ids, NodeId) {
    let owner = OwnerId::new(3).unwrap();
    let mut store = Store::from_constructor(owner, 4);
    let mut ids = Ids::from_constructor(owner, 4);
    let id = store.allocate(&mut ids, 0x0102_00ab, 0x8400_0002).unwrap();
    (store, ids, id)
}
#[test]
fn constructor_preserves_unwritten_links_and_typed_pointer_roles_before_context_or_lookup() {
    let (mut store, _, id) = fixture();
    let header = store.get(id).unwrap();
    assert_eq!(header.stage(), Stage::Constructed);
    assert_eq!(header.word(0), None);
    assert_eq!(header.word(4), None);
    for at in [8, 0xc, 0x30, 0x38, 0x5c, 0x60] {
        assert_eq!(header.word(at), None);
    }
    for at in [0x14, 0x44, 0x48, 0x50, 0x88] {
        assert_eq!(header.word(at), Some(u32::MAX));
    }
    assert_eq!(header.word(0x10), Some(0x8400_0002));
    assert_eq!(header.word(0x1c), Some(0));
    assert_eq!(header.word(0x34), Some(1f32.to_bits()));
    assert_eq!(header.word(0x2c), Some(1f32.to_bits()));
    for at in [0x68, 0x84] {
        assert_eq!(header.word(at), Some((-1f32).to_bits()));
    }
    for at in [0x28, 0x3c, 0x40, 0x54, 0x64, 0x78, 0x7c, 0x80, 0x8c] {
        assert_eq!(header.word(at), Some(0));
    }
    let mut provider = source();
    assert!(store.initialize_prefix(id, &mut provider).is_err());
    assert!(provider.calls.is_empty());
    store.bind_factory_context(id, 1, 0x1234).unwrap();
    let header = store.get(id).unwrap();
    assert_eq!(header.word(0x20), Some(0x1234));
    assert_eq!(header.word(0x1c), None);
    assert_eq!(header.slot(), Some(1));
    assert!(store.bind_factory_context(id, 1, 0).is_err());
}
#[test]
fn cache_and_body_binding_are_retained_before_any_initializer_return_or_layer_insertion() {
    let (mut store, _, id) = fixture();
    store.bind_factory_context(id, 0, 0x20).unwrap();
    let mut provider = source();
    store.initialize_prefix(id, &mut provider).unwrap();
    let header = store.get(id).unwrap();
    assert_eq!(provider.calls, ["generation", "resolve", "body"]);
    assert_eq!(header.word(0x14), Some(0));
    assert_eq!(header.word(0x18), None);
    assert_eq!(header.body_bound(), Some(true));
    assert_eq!(header.stage(), Stage::AwaitingInitializer);
    assert_eq!(header.cached().unwrap().directory_index, 2);
    assert_eq!(header.word(0), None);
    assert_eq!(header.word(4), None);
    let mut words = Vec::new();
    header.visit_state_words(|word| words.push(word));
    assert!(!words.is_empty());
    let calls = provider.calls.clone();
    assert!(store.initialize_prefix(id, &mut provider).is_err());
    assert_eq!(provider.calls, calls);
}
#[test]
fn a_later_body_failure_retains_metadata_cache_and_allocation_without_replay() {
    let (mut store, ids, id) = fixture();
    store.bind_factory_context(id, 2, 7).unwrap();
    let mut provider = source();
    provider.fail = Some("body");
    assert!(store.initialize_prefix(id, &mut provider).is_err());
    let header = store.get(id).unwrap();
    assert_eq!(ids.kind(id), Some(Kind::Script));
    assert_eq!(store.len(), 1);
    assert_eq!(header.stage(), Stage::Failed);
    assert_eq!(header.word(0x14), Some(0));
    assert!(header.cached().is_some());
    assert_eq!(header.body_bound(), None);
    let calls = provider.calls.clone();
    assert!(store.initialize_prefix(id, &mut provider).is_err());
    assert_eq!(provider.calls, calls);
}
#[test]
fn matching_constructor_generation_and_missing_script_skip_unreached_body_reads() {
    for same_generation in [true, false] {
        let (mut store, _, id) = fixture();
        store.bind_factory_context(id, 0, 0).unwrap();
        let mut provider = source();
        provider.generation = if same_generation { u32::MAX } else { 7 };
        provider.found = false;
        store.initialize_prefix(id, &mut provider).unwrap();
        let header = store.get(id).unwrap();
        assert_eq!(header.stage(), Stage::ReturnedWithoutBody);
        assert_eq!(header.body_bound(), Some(false));
        assert_eq!(header.word(0x18), Some(0));
        assert_eq!(header.word(0x14), Some(u32::MAX));
        assert_eq!(
            provider.calls,
            if same_generation {
                vec!["generation"]
            } else {
                vec!["generation", "resolve"]
            }
        );
    }
}
#[test]
fn resource_owner_mismatch_cannot_read_generation_or_create_a_cached_binding() {
    let (mut store, _, id) = fixture();
    store.bind_factory_context(id, 0, 0).unwrap();
    let mut provider = source();
    provider.file = 0x0102_00ac;
    assert!(store.initialize_prefix(id, &mut provider).is_err());
    assert!(provider.calls.is_empty());
    assert_eq!(store.get(id).unwrap().cached(), None);
}
