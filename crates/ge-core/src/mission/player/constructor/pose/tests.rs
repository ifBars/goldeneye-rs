use super::*;
fn character() -> CharacterBinding {
    CharacterBinding {
        registry_resource: 0x0109_0013,
        key: 0x4d00_0002,
        source_index: 5,
    }
}
pub(crate) struct Inputs {
    pub(crate) calls: Vec<&'static str>,
    fail_user: bool,
    global: Option<u8>,
    pub(crate) multiple: bool,
    pub(crate) character_lookup: Option<CharacterLookup>,
}
impl Source for Inputs {
    fn constructor_character(
        &mut self,
        binding: Option<CharacterBinding>,
    ) -> Result<CharacterLookup> {
        if let Some(lookup) = self.character_lookup {
            Ok(lookup)
        } else {
            Ok(CharacterLookup::Bound(
                binding.context("actual character unknown")?,
            ))
        }
    }
    fn character_scalar(
        &mut self,
        binding: CharacterBinding,
        field: CharacterField,
    ) -> Result<u32> {
        ensure!(binding == character(), "wrong character binding");
        self.calls.push(match field {
            CharacterField::At10 => "char10",
            CharacterField::At14 => "char14",
        });
        Ok(match field {
            CharacterField::At10 => 2f32,
            CharacterField::At14 => 3f32,
        }
        .to_bits())
    }
    fn multiple_local_users_in_mode_three(&mut self) -> Result<bool> {
        self.calls.push("session");
        Ok(self.multiple)
    }
    fn ones_vector(&mut self) -> Result<[u32; 3]> {
        self.calls.push("ones");
        Ok([1f32.to_bits(); 3])
    }
    fn local_user_for_player(&mut self, _: PlayerId) -> Result<Option<UserId>> {
        self.calls.push("user");
        ensure!(!self.fail_user, "actual user association unavailable");
        Ok(None)
    }
    fn weapon_global_byte(&mut self) -> Result<Option<u8>> {
        self.calls.push("global");
        Ok(self.global)
    }
}
pub(crate) fn fixture() -> (
    Pending,
    base_handler::Owner,
    PendingReset,
    Preparation,
    Item,
    Inputs,
) {
    let player = PlayerId::new(9).unwrap();
    let mut item = Item::pose_fixture(player);
    let prep = Preparation::pose_fixture(player, &mut item, character());
    let base = base_handler::Owner::pose_fixture(player, item.id(), 0);
    let reset = PendingReset::pose_fixture(player, Some(character()));
    (
        Pending::new(player, 1),
        base,
        reset,
        prep,
        item,
        Inputs {
            calls: Vec::new(),
            fail_user: false,
            global: None,
            multiple: false,
            character_lookup: None,
        },
    )
}
#[test]
fn actual_null_route_returns_with_unknown_byte_retained_and_argument_unread() {
    let (mut p, mut base, mut reset, mut prep, mut item, mut inputs) = fixture();
    p.advance(&mut base, &mut reset, &mut prep, &mut item, &mut inputs)
        .unwrap();
    assert!(p.helper_returned());
    assert_eq!(p.stage(), Stage::Returned);
    assert_eq!(
        inputs.calls,
        [
            "char10", "char14", "session", "ones", "user", "global", "session"
        ]
    );
    assert_eq!(
        p.character_vector(),
        Some([2f32.to_bits(), 3f32.to_bits(), 2f32.to_bits(), 0])
    );
    assert_eq!(
        item.scale_bits(),
        [1f32.to_bits(), 1f32.to_bits(), 1f32.to_bits(), 0]
    );
    assert!(p.scale_stored() && p.scale_invalidated());
    assert_eq!(p.enable_writes()[0].unwrap().flags_after, Some(0xa));
    assert_eq!(p.enable_writes()[1].unwrap().flags_after, Some(0xb));
    assert_eq!(p.enable_writes()[0].unwrap().reference, 0x8400_0021);
    assert_eq!(p.enable_writes()[1].unwrap().reference, 0x8400_0022);
    assert_eq!(p.force_nodes_checked(), 2);
    assert_eq!(p.backlink_nodes_checked(), 2);
    assert_eq!(p.user_lookup(), Some(UserLookup::NotFound));
    assert!(p.weapon_global_read() && p.global_equivalence_used());
    assert_eq!(p.weapon_global_byte(), None);
    assert_eq!(
        p.global_argument(),
        Some(GlobalArgument::ArgumentUnreadBecauseReceiverAbsent)
    );
    assert_eq!(p.fade_argument_bits(), Some(1f32.to_bits()));
    assert_eq!(reset.byte(0xbab), Some(1));
    assert_eq!(reset.word(0xca4), Some(0x0e00_0070));
    assert_eq!(base.word(0x6e0), Some(0)); // Stale numeric zero was never consumed.
}
#[test]
fn user_provider_failure_retains_prefix_and_resumes_without_reenabling_components() {
    let (mut p, mut base, mut reset, mut prep, mut item, mut inputs) = fixture();
    inputs.fail_user = true;
    assert!(
        p.advance(&mut base, &mut reset, &mut prep, &mut item, &mut inputs)
            .is_err()
    );
    assert_eq!(p.stage(), Stage::UserLookup);
    assert!(p.ca4_written() && p.scale_stored());
    assert_eq!(p.user_lookup(), None);
    assert!(!p.weapon_global_read());
    let writes = *p.enable_writes();
    inputs.fail_user = false;
    p.advance(&mut base, &mut reset, &mut prep, &mut item, &mut inputs)
        .unwrap();
    assert_eq!(*p.enable_writes(), writes);
    assert_eq!(inputs.calls.iter().filter(|v| **v == "char10").count(), 1);
    assert!(p.helper_returned());
}
#[test]
fn early_gate_skips_all_later_state_and_provider_reads() {
    let (mut p, _, mut reset, _, mut item, mut inputs) = fixture();
    let mut base = base_handler::Owner::pose_fixture(p.player(), item.id(), 1);
    let mut prep = Preparation::new();
    p.advance(&mut base, &mut reset, &mut prep, &mut item, &mut inputs)
        .unwrap();
    assert!(p.helper_returned());
    assert!(!p.bab_written());
    assert!(inputs.calls.is_empty());
    assert_eq!(reset.byte(0xbab), Some(7));
}
#[test]
fn blend_pending_keeps_bab_but_never_reads_character_or_component_owners() {
    let (mut p, mut base, mut reset, _, mut item, mut inputs) = fixture();
    reset.constructor_tail_test_byte(0xba8, Some(1));
    let mut prep = Preparation::new();
    p.advance(&mut base, &mut reset, &mut prep, &mut item, &mut inputs)
        .unwrap();
    assert_eq!(p.dependency(), Some(Dependency::BlendThreshold));
    assert_eq!(reset.byte(0xbab), Some(1));
    assert!(inputs.calls.is_empty());
    assert_eq!(p.receiver_searches(), [false; 2]);
}
#[test]
fn missing_semantic_character_does_not_use_numeric_zero_to_skip_vector_reads() {
    let (mut p, mut base, _, mut prep, mut item, mut inputs) = fixture();
    let mut reset = PendingReset::pose_fixture(p.player(), None);
    assert!(
        p.advance(&mut base, &mut reset, &mut prep, &mut item, &mut inputs)
            .is_err()
    );
    assert_eq!(p.stage(), Stage::CharacterBinding);
    assert!(inputs.calls.is_empty());
    assert_eq!(p.receiver_searches(), [true; 2]);
    assert!(!p.scale_stored());
}
#[test]
fn multiplayer_route_stops_after_both_character_reads_before_startup_ones() {
    let (mut p, mut base, mut reset, mut prep, mut item, mut inputs) = fixture();
    inputs.multiple = true;
    p.advance(&mut base, &mut reset, &mut prep, &mut item, &mut inputs)
        .unwrap();
    assert_eq!(p.dependency(), Some(Dependency::MultiplayerScale));
    assert_eq!(inputs.calls, ["char10", "char14", "session"]);
    assert!(!p.scale_stored());
}
#[test]
fn known_global_byte_records_its_real_branch_instead_of_claiming_equivalence() {
    for (byte, argument) in [(0, 1), (1, 0), (0x80, 0)] {
        let (mut p, mut base, mut reset, mut prep, mut item, mut inputs) = fixture();
        inputs.global = Some(byte);
        p.advance(&mut base, &mut reset, &mut prep, &mut item, &mut inputs)
            .unwrap();
        assert_eq!(p.global_argument(), Some(GlobalArgument::Known(argument)));
        assert_eq!(p.weapon_global_byte(), Some(byte));
        assert!(!p.global_equivalence_used());
    }
}

#[test]
fn missing_blend_gate_does_not_store_bab() {
    let (mut p, mut base, mut reset, mut prep, mut item, mut inputs) = fixture();
    reset.constructor_tail_test_byte(0xba8, None);
    assert!(
        p.advance(&mut base, &mut reset, &mut prep, &mut item, &mut inputs)
            .is_err()
    );
    assert_eq!(p.stage(), Stage::BlendGate);
    assert!(!p.bab_written());
    assert_eq!(reset.byte(0xbab), Some(7));
    assert!(inputs.calls.is_empty());
}

#[test]
fn disabled_fp_uses_actual_character_vector_without_reading_ones() {
    let (_, mut base, mut reset, mut prep, mut item, mut inputs) = fixture();
    let mut p = Pending::new(base.player(), 0);
    p.advance(&mut base, &mut reset, &mut prep, &mut item, &mut inputs)
        .unwrap();
    assert!(p.helper_returned());
    assert_eq!(p.tp_enabled(), Some(true));
    assert_eq!(p.fp_enabled(), Some(false));
    assert_eq!(
        item.scale_bits(),
        [2f32.to_bits(), 3f32.to_bits(), 2f32.to_bits(), 0]
    );
    assert!(!inputs.calls.contains(&"ones"));
}

#[test]
fn unknown_global_cannot_use_argument_equivalence_for_nonnull_receiver() {
    let (mut p, mut base, mut reset, mut prep, mut item, mut inputs) = fixture();
    // Authored boundary just before this global read, after earlier helper calls.
    p.stage = Stage::GlobalByte;
    reset.constructor_tail_test_word(0xafc, Some(11));
    p.advance(&mut base, &mut reset, &mut prep, &mut item, &mut inputs)
        .unwrap();
    assert_eq!(p.dependency(), Some(Dependency::GlobalArgument));
    assert!(p.weapon_global_read());
    assert_eq!(p.weapon_global_byte(), None);
    assert!(!p.global_equivalence_used());
    assert_eq!(p.global_argument(), None);
    assert!(!p.helper_returned());
}
