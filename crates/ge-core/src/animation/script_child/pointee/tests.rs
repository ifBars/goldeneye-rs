use super::*;

// Independent synthetic header fixture; no original payload or address.
fn returned() -> (Store, NodeId) {
    let (mut store, _, id) = super::super::tests::fixture();
    store.bind_factory_context(id, 0, 3).unwrap();
    store.headers.get_mut(&id).unwrap().stage = super::super::Stage::ReturnedWithoutBody;
    (store, id)
}

#[test]
fn constructor_role_writes_the_same_header_parameter_word() {
    let (mut store, id) = returned();
    let header = store.get(id).unwrap();
    assert_eq!(
        header.parameter_pointer(),
        ParameterPointer::ConstructorInlinePhase
    );
    assert_eq!(header.word(0x60), None);
    let write = store.store_through_parameter(id, 0x3f00_0000).unwrap();
    let ParameterWrite::Stored(receipt) = write else {
        panic!("constructor role must store")
    };
    assert_eq!(receipt.previous, Some(0));
    assert_eq!(receipt.offset, 0x64);
    let header = store.get(id).unwrap();
    assert_eq!(header.word(0x64), Some(0x3f00_0000));
    // The pointer bytes stay address-free; no clip or phase field moved.
    assert_eq!(header.word(0x60), None);
    assert_eq!(header.word(0x44), Some(u32::MAX));
    assert_eq!(header.word(0x80), Some(0));
}

#[test]
fn unknown_role_pauses_without_write_and_null_fails() {
    let (mut store, id) = returned();
    store.headers.get_mut(&id).unwrap().parameter = ParameterPointer::Unknown;
    assert_eq!(
        store.store_through_parameter(id, 0x3f00_0000).unwrap(),
        ParameterWrite::Pending(ParameterPointer::Unknown)
    );
    assert_eq!(store.get(id).unwrap().word(0x64), Some(0));
    store.headers.get_mut(&id).unwrap().parameter = ParameterPointer::Null;
    assert!(store.store_through_parameter(id, 0x3f00_0000).is_err());
    assert_eq!(store.get(id).unwrap().word(0x64), Some(0));
    let (mut store, _, id) = super::super::tests::fixture();
    assert!(store.store_through_parameter(id, 0x3f00_0000).is_err());
}

#[test]
fn dispatch_coordinate_store_reaffirms_the_constructor_role() {
    let (mut store, id) = returned();
    store.headers.get_mut(&id).unwrap().parameter = ParameterPointer::Unknown;
    store.set_dispatch_coordinate(id, 7).unwrap();
    assert_eq!(
        store.get(id).unwrap().parameter_pointer(),
        ParameterPointer::ConstructorInlinePhase
    );
}
