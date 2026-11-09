use super::registry::{budget, common, finish, map, row};
use crate::mission::triggers::*;
use actor12::ProviderOutcome;
use shape71::{Geometry, GeometryInput, Stage};

struct ShapeFactory {
    input: Option<GeometryInput>,
    metadata: Vec<ShapeRequest>,
    geometry: Vec<shape71::Request>,
    malformed: bool,
}
impl Factory for ShapeFactory {
    fn initialize_common_shape(&mut self, request: ShapeRequest) -> anyhow::Result<ShapeOutcome> {
        self.metadata.push(request);
        Ok(ShapeOutcome::Bound(ShapeBinding::SelectorRetained {
            selector: request.selector,
            type_raw: 0,
        }))
    }
    fn shape71_geometry(
        &mut self,
        request: shape71::Request,
    ) -> anyhow::Result<ProviderOutcome<GeometryInput>> {
        self.geometry.push(request);
        anyhow::ensure!(!self.malformed, "authored malformed geometry reader");
        Ok(self
            .input
            .map_or(ProviderOutcome::Unavailable, ProviderOutcome::Ready))
    }
    fn initialize_animator(&mut self, _: AnimatorRequest) -> anyhow::Result<AnimatorId> {
        panic!("status1 shape constructor must not allocate an animator")
    }
}
fn factory() -> ShapeFactory {
    ShapeFactory {
        input: Some(GeometryInput {
            type_raw: 0,
            word_04_raw: 7,
            center_bits: [0; 3],
            orientation_bits: [0, 0, 0, 0x3f80_0000],
            dimensions_bits: [0x4000_0000; 3],
        }),
        metadata: Vec::new(),
        geometry: Vec::new(),
        malformed: false,
    }
}
fn shape(ordinal: usize) -> RowInput {
    let mut row = row(ordinal);
    row.descriptor = Descriptor {
        native_type_index: 35,
        class: 0x1e00_0009,
        subtype: 0x1f00_0071,
        enable_byte_raw: 1,
    };
    row
}
fn present() -> CommonInput {
    let mut payload = common(0);
    payload.position_bits = [0; 4];
    payload.rotation_bits = [0; 4];
    payload.shape_index_raw = Some(0);
    payload.property_mask_raw |= 1 << 27;
    payload
}

#[test]
fn present_selector_zero_requires_geometry_and_transform_before_status1_membership() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, None).unwrap();
    let mut factory = factory();
    let mut payload = present();
    payload.numeric[15] = 0xdead_beef;
    payload.property_mask_raw |= (1 << 15) | (1 << 26);
    payload.animator_present = true;
    payload.position_bits = [1.0f32.to_bits(), 2.0f32.to_bits(), 3.0f32.to_bits(), 0];
    payload.rotation_bits[1] = 0.5f32.to_bits();
    registry
        .visit(shape(0), || Ok(payload), &mut factory)
        .unwrap();
    assert_eq!((factory.metadata.len(), factory.geometry.len()), (1, 1));
    assert_eq!(factory.geometry[0].selector, 0);
    assert_eq!(factory.geometry[0].map, map(1));
    let object = &registry.objects()[0];
    let state = object.shape71().unwrap();
    assert_eq!(state.geometry_stage(), Stage::Transformed);
    assert_eq!(state.geometry().tag(), 2);
    assert!(state.placement_matrix_bits().is_some());
    let Geometry::Type2 {
        center_bits,
        dimensions_bits,
        ..
    } = state.geometry()
    else {
        panic!("type2 missing")
    };
    assert_eq!(
        center_bits,
        &[1.0f32.to_bits(), 2.0f32.to_bits(), 3.0f32.to_bits(), 0]
    );
    assert_eq!(dimensions_bits, &[0x4000_0000; 3]);
    assert_eq!(state.radius_squared_bits(), 0x461c_3c00);
    assert_eq!(object.runtime_numeric()[15], 0xdead_beef);
    assert_eq!(object.registration_status(), Some(FactoryStatus::Status1));
    assert_eq!(object.animator(), AnimatorState::StoppedAbsent);
    finish(&mut registry);
    assert!(registry.is_complete());
}

#[test]
fn metadata_binding_and_partial_materialization_remain_real_staged_objects() {
    for unsupported_placement in [false, true] {
        let mut registry = Registry::new(budget());
        registry.begin_map(map(1), 2, None).unwrap();
        let mut factory = factory();
        let mut payload = present();
        if unsupported_placement {
            payload.rotation_bits[0] = f32::MAX.to_bits();
        } else {
            factory.input = None;
        }
        let slot = registry
            .visit(shape(0), || Ok(payload), &mut factory)
            .unwrap();
        assert_eq!(slot.retained_trigger(), Some(registry.objects()[0].id()));
        assert!(matches!(
            slot,
            Slot::Unsupported(Unsupported::Shape71Geometry { .. })
        ));
        let state = registry.objects()[0].shape71().unwrap();
        assert_eq!(
            state.geometry_stage(),
            if unsupported_placement {
                Stage::Materialized
            } else {
                Stage::Uninitialized
            }
        );
        assert_eq!(
            state.geometry().tag(),
            if unsupported_placement { 2 } else { -1 }
        );
        assert!(registry.objects()[0].common_shape().is_some());
        assert!(registry.membership().is_empty());
        let mut fallback = common(0);
        fallback.reference_ordinals[0] = 0;
        fallback.property_mask_raw |= 1 << 16;
        registry
            .visit(shape(1), || Ok(fallback), &mut factory)
            .unwrap();
        finish(&mut registry);
        assert_eq!(
            registry.objects()[1].references()[0],
            Reference::Blocked { ordinal: 0 }
        );
        assert!(!registry.is_complete());
    }
}

#[test]
fn absent_property_owns_sparse_source_type0_without_geometry_lookup_or_transform() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, None).unwrap();
    let mut factory = factory();
    factory.malformed = true;
    let mut payload = common(0);
    payload.position_bits = [0x8000_0000, 0x7fc0_1234, 3, u32::MAX];
    registry
        .visit(shape(0), || Ok(payload), &mut factory)
        .unwrap();
    assert!(factory.metadata.is_empty() && factory.geometry.is_empty());
    let state = registry.objects()[0].shape71().unwrap();
    assert_eq!(
        state.geometry(),
        &Geometry::Type0 {
            center_bits: [0x8000_0000, 0x7fc0_1234, 3, 0],
            radius_bits: 0
        }
    );
    assert_eq!(state.geometry_stage(), Stage::FallbackComplete);
    assert_eq!(state.placement_matrix_bits(), None);
    finish(&mut registry);
    assert!(registry.is_complete());
}

#[test]
fn malformed_provider_propagates_and_retains_original_slot_without_fabricated_geometry() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, None).unwrap();
    let mut factory = factory();
    factory.malformed = true;
    assert!(
        registry
            .visit(shape(0), || Ok(present()), &mut factory)
            .is_err()
    );
    let slot = registry.slots(0).unwrap()[0];
    assert_eq!(slot.retained_trigger(), Some(registry.objects()[0].id()));
    assert_eq!(
        registry.objects()[0].shape71().unwrap().geometry(),
        &Geometry::Uninitialized
    );
    assert_eq!(registry.scratch().len(), 1);
    assert!(registry.membership().is_empty());
    assert_eq!(
        Slot::Unsupported(Unsupported::Shape { index: 0 }).retained_trigger(),
        None
    );
    assert_eq!(Slot::Filtered.retained_trigger(), None);
}
