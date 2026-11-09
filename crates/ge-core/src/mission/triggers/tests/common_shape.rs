use crate::mission::triggers::*;

use super::registry::{FactoryProbe, budget, common, finish, map, row};

struct ShapeFactory {
    calls: Vec<ShapeRequest>,
    outcome: ShapeOutcome,
    malformed: bool,
}

impl Factory for ShapeFactory {
    fn initialize_common_shape(&mut self, request: ShapeRequest) -> anyhow::Result<ShapeOutcome> {
        self.calls.push(request);
        anyhow::ensure!(!self.malformed, "authored malformed reached shape byte");
        Ok(self.outcome)
    }
    fn initialize_animator(&mut self, _: AnimatorRequest) -> anyhow::Result<AnimatorId> {
        anyhow::bail!("shape binding must not construct an animator")
    }
}

fn factory(selector: u32, type_raw: u8) -> ShapeFactory {
    ShapeFactory {
        calls: Vec::new(),
        outcome: ShapeOutcome::Bound(ShapeBinding::SelectorRetained { selector, type_raw }),
        malformed: false,
    }
}

fn radius_row(ordinal: usize) -> RowInput {
    let mut row = row(ordinal);
    row.descriptor.native_type_index = 3;
    row.descriptor.class = 0x1e00_0004;
    row.descriptor.subtype = 0x1f00_0006;
    row
}

fn shaped(selector: u32) -> CommonInput {
    let mut common = common(10);
    common.property_mask_raw |= 1 << 27;
    common.shape_index_raw = Some(selector);
    common.numeric[4] = 0x4200_0000;
    common
}

#[test]
fn selector_zero_is_checked_before_retention_and_owns_no_invented_geometry() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, None).unwrap();
    let mut factory = factory(0, 0);
    let payload = shaped(0);
    let original_numeric = payload.numeric;
    registry
        .visit(radius_row(0), || Ok(payload), &mut factory)
        .unwrap();
    assert_eq!(
        factory.calls,
        [ShapeRequest {
            map: map(1),
            ordinal: 0,
            selector: 0,
            kind: TriggerKind::Radius06,
            zone: 0x1234
        }]
    );
    let object = &registry.objects()[0];
    // The complete binding schema has just selector and consumed type byte.
    assert_eq!(
        object.common_shape(),
        Some(&ShapeBinding::SelectorRetained {
            selector: 0,
            type_raw: 0
        })
    );
    assert_eq!(object.common().shape_index_raw, Some(0));
    assert_eq!(object.common().numeric, original_numeric);
    assert_eq!(object.common().position_bits[3], 0);
    assert_eq!(object.radius06().unwrap().radius_bits(), 0x3f80_0000);
    assert_eq!(object.registration_status(), Some(FactoryStatus::Status1));
    assert_eq!(object.animator(), AnimatorState::StoppedAbsent);
    assert_eq!(registry.scratch(), registry.membership());
    finish(&mut registry);
    assert!(registry.is_complete());
}

#[test]
fn absent_shape_never_calls_factory_and_stays_distinct_from_present_zero() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, None).unwrap();
    let mut factory = factory(0, 0);
    factory.malformed = true;
    registry
        .visit(radius_row(0), || Ok(common(0)), &mut factory)
        .unwrap();
    assert!(factory.calls.is_empty());
    assert_eq!(registry.objects()[0].common_shape(), None);
    assert_eq!(registry.objects()[0].common().shape_index_raw, None);
}

#[test]
fn default_unsupported_and_nonzero_type_keep_original_slot_without_scratch() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 2, None).unwrap();
    let mut old_factory = FactoryProbe::default();
    assert_eq!(
        registry
            .visit(radius_row(0), || Ok(shaped(0)), &mut old_factory)
            .unwrap(),
        Slot::Unsupported(Unsupported::Shape { index: 0 })
    );
    let mut nonzero = factory(7, 3);
    assert_eq!(
        registry
            .visit(radius_row(1), || Ok(shaped(7)), &mut nonzero)
            .unwrap(),
        Slot::Unsupported(Unsupported::Shape { index: 7 })
    );
    assert_eq!(nonzero.calls.len(), 1);
    assert!(
        registry.objects().is_empty()
            && registry.scratch().is_empty()
            && registry.membership().is_empty()
    );
    finish(&mut registry);
    assert!(!registry.is_complete());
}

#[test]
fn malformed_factory_error_and_binding_mismatch_propagate_without_relabeling_unsupported() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, None).unwrap();
    let mut malformed = factory(2, 0);
    malformed.malformed = true;
    let error = registry
        .visit(radius_row(0), || Ok(shaped(2)), &mut malformed)
        .unwrap_err();
    assert!(error.to_string().contains("malformed reached shape byte"));
    assert_eq!(registry.slots(0).unwrap(), &[Slot::Unvisited]);
    let mut mismatch = factory(3, 0);
    assert!(
        registry
            .visit(radius_row(0), || Ok(shaped(2)), &mut mismatch)
            .is_err()
    );
    assert_eq!(registry.slots(0).unwrap(), &[Slot::Unvisited]);
    assert!(registry.objects().is_empty() && registry.scratch().is_empty());
}

#[test]
fn negative_raw_selectors_are_unsupported_not_absent_and_do_not_address_factory() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, None).unwrap();
    let mut factory = factory(u32::MAX, 0);
    assert_eq!(
        registry
            .visit(radius_row(0), || Ok(shaped(u32::MAX)), &mut factory)
            .unwrap(),
        Slot::Unsupported(Unsupported::Shape { index: u32::MAX })
    );
    assert!(factory.calls.is_empty());
    assert!(registry.objects().is_empty());
}

#[test]
fn actual_bound_shape_objects_resolve_original_references_without_changing_status_or_catalog_kind()
{
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 2, None).unwrap();
    let mut factory = factory(5, 0);
    let mut start = common(1);
    start.reference_ordinals[0] = 1;
    registry.visit(row(0), || Ok(start), &mut factory).unwrap();
    registry
        .visit(radius_row(1), || Ok(shaped(5)), &mut factory)
        .unwrap();
    finish(&mut registry);
    let source = &registry.objects()[0];
    let target = &registry.objects()[1];
    assert_eq!(source.references()[0], Reference::Trigger(target.id()));
    assert_eq!(source.registration_status(), Some(FactoryStatus::Status2));
    assert_eq!(target.registration_status(), Some(FactoryStatus::Status1));
    assert_eq!(target.runtime_flags_raw(), 0);
    let source_id = source.id();
    registry.rebuild_catalog().unwrap();
    assert_eq!(registry.catalog(), &[source_id]);
}
