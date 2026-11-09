use super::registry::{budget, common, finish, map, row};
use crate::mission::{resource_state, triggers::*};
use actor12::ProviderOutcome;

struct Provider {
    owner: Option<resource_state::Owner>,
    requests: Vec<radius03::Request>,
    lookups: Vec<resource_state::Lookup>,
    malformed: bool,
}

impl Factory for Provider {
    fn radius03_resource(
        &mut self,
        request: radius03::Request,
    ) -> anyhow::Result<ProviderOutcome<u32>> {
        self.requests.push(request);
        anyhow::ensure!(!self.malformed, "authored malformed resource owner");
        Ok(match &self.owner {
            Some(owner) => {
                let lookup = owner.lookup(
                    request.resource,
                    request.fallback,
                    resource_state::Budget {
                        records: 64,
                        work: 64,
                    },
                )?;
                self.lookups.push(lookup);
                ProviderOutcome::Ready(lookup.value)
            }
            None => ProviderOutcome::Unavailable,
        })
    }
    fn initialize_animator(&mut self, _: AnimatorRequest) -> anyhow::Result<AnimatorId> {
        panic!("subtype03 status1 does not construct an animator")
    }
}

fn provider(owner: Option<resource_state::Owner>) -> Provider {
    Provider {
        owner,
        requests: Vec::new(),
        lookups: Vec::new(),
        malformed: false,
    }
}
fn radius(ordinal: usize) -> RowInput {
    let mut row = row(ordinal);
    row.descriptor = Descriptor {
        native_type_index: 2,
        class: 0x1e00_0001,
        subtype: 0x1f00_0003,
        enable_byte_raw: 1,
    };
    row
}

#[test]
fn radius03_keeps_zero_and_raw_inputs_and_stores_real_shared_lookup_low_byte() {
    assert_eq!(
        radius03::State::from_serialized(0, 0).radius_squared_bits(),
        0
    );
    assert_eq!(
        radius03::State::from_serialized(1, 0).radius_squared_bits(),
        0x3c23_d70b
    );
    assert_eq!(
        radius03::State::from_serialized(u32::MAX, 0).radius_squared_bits(),
        0x3c23_d70b
    );
    let owner = resource_state::Owner::from_snapshot(
        vec![
            resource_state::Record {
                key: 7,
                value: 0x1234,
            },
            resource_state::Record {
                key: 7,
                value: 0xffff,
            },
            resource_state::Record {
                key: 8,
                value: 0xff99,
            },
        ],
        32,
        2,
        resource_state::Budget {
            records: 32,
            work: 3,
        },
    )
    .unwrap();
    let mut provider = provider(Some(owner));
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 2, None).unwrap();
    for (ordinal, token) in [0x4200_0007, 0x4200_0008].into_iter().enumerate() {
        let mut payload = common(0);
        payload.numeric[3] = token;
        payload.header_flags_raw = 1;
        payload.animator_present = true;
        payload.property_mask_raw |= (1 << 3) | (1 << 26);
        registry
            .visit(radius(ordinal), || Ok(payload), &mut provider)
            .unwrap();
    }
    assert_eq!(provider.requests.len(), 2);
    assert!(
        provider
            .requests
            .iter()
            .all(|request| request.fallback == 0)
    );
    assert_eq!(
        provider
            .lookups
            .iter()
            .map(|lookup| lookup.scanned)
            .collect::<Vec<_>>(),
        [1, 3]
    );
    for (index, low) in [0x34, 0x99].into_iter().enumerate() {
        let object = &registry.objects()[index];
        let state = object.radius03().unwrap();
        assert_eq!(object.runtime_numeric(), &object.common().numeric);
        assert_eq!(state.radius_squared_bits(), 0);
        assert_eq!(state.mode_low_byte(), Some(low));
        assert_eq!(state.mode_high_byte(), 0);
        assert!(state.requested_resource().is_some());
        assert_eq!(object.registration_status(), Some(FactoryStatus::Status1));
        assert_eq!(object.animator(), AnimatorState::StoppedAbsent);
    }
    finish(&mut registry);
    assert!(registry.is_complete());
}

#[test]
fn rejected_tokens_are_lazy_while_owned_fresh_miss_is_a_real_completed_query() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 3, None).unwrap();
    let mut unavailable = provider(None);
    for (ordinal, token) in [0, 0x4200_0000, 0x4300_0007].into_iter().enumerate() {
        let mut payload = common(10);
        payload.numeric[3] = token;
        payload.property_mask_raw |= 1 << 3;
        registry
            .visit(radius(ordinal), || Ok(payload), &mut unavailable)
            .unwrap();
        let object = &registry.objects()[ordinal];
        assert_eq!(object.runtime_numeric()[3], token);
        assert_eq!(object.radius03().unwrap().mode_low_byte(), Some(0));
    }
    assert!(unavailable.requests.is_empty());
    finish(&mut registry);
    assert!(registry.is_complete());
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, None).unwrap();
    let mut fresh = provider(Some(resource_state::Owner::from_constructor(2500)));
    let mut payload = common(0);
    payload.numeric[3] = 0x4200_0009;
    payload.property_mask_raw |= 1 << 3;
    registry
        .visit(radius(0), || Ok(payload), &mut fresh)
        .unwrap();
    assert_eq!(
        fresh.lookups[0],
        resource_state::Lookup {
            value: 0,
            matched: false,
            scanned: 0
        }
    );
    assert_eq!(
        registry.objects()[0].radius03().unwrap().mode_low_byte(),
        Some(0)
    );
    assert!(registry.objects()[0].is_registered());
}

#[test]
fn missing_or_malformed_provider_keeps_staged_scratch_original_slot_and_blocked_references() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 2, None).unwrap();
    let mut provider = provider(None);
    let mut payload = common(0);
    payload.numeric[3] = 0x4200_0007;
    payload.property_mask_raw |= 1 << 3;
    let original = payload.clone();
    assert!(matches!(
        registry
            .visit(radius(0), || Ok(payload), &mut provider)
            .unwrap(),
        Slot::Unsupported(Unsupported::Radius03Provider {
            resource: 0x4200_0007,
            ..
        })
    ));
    assert_eq!(registry.scratch().len(), 1);
    assert!(registry.membership().is_empty());
    assert_eq!(
        registry.objects()[0].radius03().unwrap().mode_low_byte(),
        None
    );
    assert_eq!(registry.objects()[0].common().numeric, original.numeric);
    let mut other = common(0);
    other.reference_ordinals[0] = 0;
    other.property_mask_raw |= 1 << 16;
    // Use another inactive03 object so no StartPoint animator policy is involved.
    registry
        .visit(radius(1), || Ok(other), &mut provider)
        .unwrap();
    finish(&mut registry);
    assert_eq!(
        registry.objects()[1].references()[0],
        Reference::Blocked { ordinal: 0 }
    );
    assert!(!registry.is_complete());
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, None).unwrap();
    provider.malformed = true;
    assert!(
        registry
            .visit(radius(0), || Ok(original), &mut provider)
            .is_err()
    );
    assert!(matches!(
        registry.slots(0).unwrap()[0],
        Slot::Unsupported(Unsupported::Radius03Provider { .. })
    ));
    assert_eq!(registry.scratch().len(), 1);
    assert!(registry.membership().is_empty());
}
