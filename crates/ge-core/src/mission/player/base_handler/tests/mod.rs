use super::*;
use crate::mission::{resource_state, source_rng};
mod runtime_reset;

struct Providers {
    rng: source_rng::SourceRngOwner,
    resources: resource_state::Owner,
    calls: Vec<&'static str>,
    fail: Option<&'static str>,
    global: Option<u8>,
}
fn resource_budget() -> resource_state::Budget {
    resource_state::Budget {
        records: 64,
        work: 128,
    }
}
impl Providers {
    fn new() -> Self {
        Self {
            rng: source_rng::SourceRngOwner::from_snapshot(0x1234_5678),
            resources: resource_state::Owner::from_constructor(2500),
            calls: Vec::new(),
            fail: None,
            global: None,
        }
    }
}
impl Provider for Providers {
    fn draw_handler_seed(&mut self) -> anyhow::Result<u16> {
        self.calls.push("random");
        anyhow::ensure!(self.fail != Some("random"), "authored RNG failure");
        Ok(self.rng.draw_u32(source_rng::Budget { draws: 20 })? as u16)
    }
    fn reset_base_global_byte(&mut self) -> anyhow::Result<()> {
        self.calls.push("global");
        anyhow::ensure!(self.fail != Some("global"), "authored global failure");
        self.global = Some(0);
        Ok(())
    }
    fn reset_resource_state(&mut self, token: u32, value: u32) -> anyhow::Result<()> {
        self.calls.push("resource");
        anyhow::ensure!(self.fail != Some("resource"), "authored setter failure");
        assert_eq!((token, value), (0x4200_050b, 0));
        assert_ne!(
            self.resources.set(token, value, resource_budget())?,
            resource_state::SetOutcome::Invalid
        );
        Ok(())
    }
}
fn prefix(provider: &mut Providers) -> Owner {
    let mut owner = Owner::new(PlayerId::new(62).unwrap());
    owner.initialize_generic(provider).unwrap();
    owner.initialize_basic().unwrap();
    owner.initialize_player_basic_prefix().unwrap();
    owner
}

#[test]
fn source_sparse_generic_constructor_consumes_existing_rng_and_preserves_unknown_holes() {
    let mut provider = Providers::new();
    provider
        .rng
        .draw_u32(source_rng::Budget { draws: 20 })
        .unwrap();
    let mut expected = provider.rng.clone();
    let seed = expected.draw_u32(source_rng::Budget { draws: 20 }).unwrap() as u16;
    let mut owner = Owner::new(PlayerId::new(3).unwrap());
    owner.initialize_generic(&mut provider).unwrap();
    assert_eq!(owner.word(0x1c8), Some(u32::from(seed)));
    assert_eq!(provider.rng.state_raw(), expected.state_raw());
    assert_eq!(provider.rng.draws_since_snapshot(), 2);
    assert_eq!(provider.calls, ["random"]);
    assert_eq!(owner.word(0x220), Some(0x17));
    assert_eq!(owner.word(0x224), None);
    assert_eq!(owner.word(0x1ac), None);
    assert_eq!(owner.word(0x168), Some(0x3f00_0000));
    assert_eq!(owner.word(0x174), Some(0x3f80_0000));
    assert_eq!(owner.word(0x17c), Some(0x3da3_d70a));
    assert_eq!(owner.word(0x214), Some(0x7f7f_ffff));
    assert_eq!(owner.word(0x218), Some(0xff7f_ffff));
}

#[test]
fn rng_failure_retains_prior_constructor_writes_and_cannot_duplicate_the_call() {
    let mut provider = Providers::new();
    provider.fail = Some("random");
    let mut owner = Owner::new(PlayerId::new(3).unwrap());
    assert!(owner.initialize_generic(&mut provider).is_err());
    assert_eq!(owner.stage(), Stage::GenericPrefix);
    assert_eq!(owner.failure(), Some(Failure::RandomSeed));
    assert_eq!(owner.word(0x1bc), Some(0x1ff));
    assert_eq!(owner.word(0x1c8), None);
    assert!(owner.initialize_generic(&mut provider).is_err());
    assert!(owner.initialize_basic().is_err());
    assert_eq!(provider.calls, ["random"]);
}

#[test]
fn base_and_nested_770_keep_the_actual_prior_read_and_unwritten_padding() {
    let mut provider = Providers::new();
    let owner = prefix(&mut provider);
    assert_eq!(owner.stage(), Stage::PlayerBasicPrefix);
    assert_eq!(owner.word(0x3ac), Some(0x42c8_0000));
    assert_eq!(owner.word(0x3a8), Some(0x42c8_0000));
    assert_eq!(owner.word(0x4b8), Some(0xc47a_0000));
    assert_eq!(owner.byte(0x4e5), Some(1));
    assert_eq!(owner.word(0x4f8), Some(0));
    assert_eq!(owner.halfword(0x4f2), Some(u16::MAX));
    assert!(owner.nested_initialized());
    assert_eq!(owner.word(0x85c), Some(0));
    assert_eq!(owner.word(0x7d0), Some(0x4974_2400));
    assert_eq!(owner.word(0x824), None); // nested +b4
    assert_eq!(owner.word(0x854), Some(0)); // caller writes nested +e4
    assert_eq!(owner.word(0x860), None); // semantic binding, not a fake pointer
    assert_eq!(owner.byte(0x8a1), Some(0)); // nested +131
    assert_eq!(owner.byte(0x8a2), None); // padding +132
    assert_eq!(provider.calls, ["random"]);
}

#[test]
fn actual_reset_mutates_persistent_store_and_preserves_explicit_global_field_bits() {
    let mut provider = Providers::new();
    provider
        .resources
        .set(0x4200_050b, 91, resource_budget())
        .unwrap();
    provider
        .resources
        .set(0x4200_0009, 7, resource_budget())
        .unwrap();
    let mut owner = prefix(&mut provider);
    let reset = owner.apply_reset(0x7fc0_0123, &mut provider).unwrap();
    assert_eq!(reset.invocation, 1);
    assert_eq!(provider.calls, ["random", "global", "resource"]);
    assert_eq!(provider.global, Some(0));
    assert_eq!(
        provider.resources.rows(),
        [
            resource_state::Record {
                key: 0x050b,
                value: 0
            },
            resource_state::Record { key: 9, value: 7 }
        ]
    );
    assert_eq!(owner.word(0x71c), Some(0x7fc0_0123));
    assert_eq!(owner.word(0x720), Some(0x7fc0_0123));
    assert_eq!(owner.word(0x998), Some(0));
    assert_eq!(owner.word(0x6ac), Some(u32::MAX));
    assert_eq!(owner.word(0x678), Some(0x3f80_0000));
    assert_eq!(owner.word(0x224), None);
    let proof = owner.finish_basic().unwrap();
    assert_eq!(proof.player(), owner.player());
    assert_eq!(owner.word(0x4ec), Some(4));
    assert_eq!(owner.word(0x998), Some(u32::MAX));
    assert!(owner.finish_basic().is_err());
}

#[test]
fn missing_resource_record_is_inserted_even_when_written_value_is_zero() {
    let mut provider = Providers::new();
    let mut owner = prefix(&mut provider);
    owner.apply_reset(0x4200_0000, &mut provider).unwrap();
    assert_eq!(
        provider.resources.rows(),
        [resource_state::Record {
            key: 0x050b,
            value: 0
        }]
    );
    assert_eq!(provider.resources.capacity(), 32);
}

#[test]
fn later_virtual_reset_calls_common_reset_again_without_redrawing_rng_or_reissuing_basic_proof() {
    let mut provider = Providers::new();
    let mut owner = prefix(&mut provider);
    owner.apply_reset(0x4200_0000, &mut provider).unwrap();
    owner.finish_basic().unwrap();
    let before = provider.rng.state_raw();
    owner.apply_reset(0x4280_0000, &mut provider).unwrap();
    assert_eq!((owner.reset_calls(), owner.completed_resets()), (2, 2));
    assert_eq!(provider.rng.state_raw(), before);
    assert_eq!(provider.rng.draws_since_snapshot(), 1);
    assert_eq!(
        provider.calls,
        ["random", "global", "resource", "global", "resource"]
    );
    assert_eq!(owner.word(0x71c), Some(0x4280_0000));
    assert!(owner.finish_basic().is_err());
    owner.finish_reset_prefix().unwrap();
    assert_eq!(owner.stage(), Stage::RuntimeResetPrefix);
    assert_eq!(owner.word(0x998), Some(u32::MAX));
}

#[test]
fn callback_errors_keep_reached_fields_and_block_false_completion_or_retries() {
    for fail in ["global", "resource"] {
        let mut provider = Providers::new();
        let mut owner = prefix(&mut provider);
        provider.fail = Some(fail);
        assert!(owner.apply_reset(0x4240_0000, &mut provider).is_err());
        assert_eq!(owner.stage(), Stage::ResetPrefix);
        assert_eq!((owner.reset_calls(), owner.completed_resets()), (1, 0));
        assert_eq!(owner.word(0x71c), Some(0x4240_0000));
        assert!(owner.finish_basic().is_err());
        let calls = provider.calls.clone();
        assert!(owner.apply_reset(0, &mut provider).is_err());
        assert_eq!(provider.calls, calls);
        if fail == "global" {
            assert_eq!(owner.failure(), Some(Failure::GlobalReset));
            assert_eq!(owner.word(0x990), Some(0));
            assert_eq!(provider.resources.rows().len(), 0);
        } else {
            assert_eq!(owner.failure(), Some(Failure::ResourceReset));
            assert_eq!(owner.word(0x990), None);
            assert_eq!(owner.word(0x998), Some(0));
            assert_eq!(provider.global, Some(0));
        }
    }
}

#[test]
fn source_order_is_checked_and_canonical_visitor_includes_partial_state() {
    let mut provider = Providers::new();
    let mut owner = Owner::new(PlayerId::new(2).unwrap());
    assert!(owner.initialize_basic().is_err());
    assert!(owner.apply_reset(0, &mut provider).is_err());
    let mut before = Vec::new();
    owner.visit_state_words(|word| before.push(word));
    owner.initialize_generic(&mut provider).unwrap();
    let mut after = Vec::new();
    owner.visit_state_words(|word| after.push(word));
    assert_ne!(before, after);
    assert!(owner.known_words().any(|(offset, _)| offset == 0x1c8));
    assert!(!owner.known_words().any(|(offset, _)| offset == 0x224));
}
