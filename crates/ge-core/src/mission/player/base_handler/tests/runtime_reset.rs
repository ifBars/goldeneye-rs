use super::*;
use anyhow::Context;

struct RuntimeProvider {
    mode: Option<u32>,
    fail: Option<&'static str>,
    calls: Vec<(&'static str, u32)>,
}
impl RuntimeProvider {
    fn new(mode: Option<u32>) -> Self {
        Self {
            mode,
            fail: None,
            calls: Vec::new(),
        }
    }
    fn call(&mut self, name: &'static str, value: u32) -> anyhow::Result<()> {
        self.calls.push((name, value));
        anyhow::ensure!(self.fail != Some(name), "authored callback failure");
        Ok(())
    }
}
impl RuntimeResetProvider for RuntimeProvider {
    fn reset_runtime_global_float(&mut self, bits: u32) -> anyhow::Result<()> {
        self.call("float", bits)
    }
    fn session_descriptor_present(&mut self) -> anyhow::Result<bool> {
        self.call("present", 0)?;
        Ok(self.mode.is_some())
    }
    fn session_descriptor_mode(&mut self) -> anyhow::Result<u32> {
        self.call("mode", 0)?;
        self.mode.context("authored descriptor disappeared")
    }
    fn set_runtime_multiplayer_byte(&mut self, value: u8) -> anyhow::Result<()> {
        self.call("byte", u32::from(value))
    }
}
fn later_prefix() -> Owner {
    let mut source = Providers::new();
    let mut owner = prefix(&mut source);
    owner.apply_reset(0x4244_0000, &mut source).unwrap();
    owner.finish_basic().unwrap();
    owner.apply_reset(0x4244_0000, &mut source).unwrap();
    owner.finish_reset_prefix().unwrap();
    owner
}

#[test]
fn later_reset_keeps_growth_and_sparse_holes_and_binds_its_real_self_identity() {
    let mut owner = later_prefix();
    owner.w(0x608, 37); // authored retained growth; reset must not change it.
    let mut provider = RuntimeProvider::new(Some(1));
    owner.finish_runtime_reset(&mut provider).unwrap();
    assert_eq!(
        provider.calls,
        [("float", 0), ("present", 0), ("mode", 0), ("byte", 0)]
    );
    assert_eq!(owner.stage(), Stage::RuntimeResetComplete);
    assert_eq!(owner.word(0x608), Some(37));
    assert_eq!(owner.word(0x708), Some(1));
    assert_eq!(owner.byte(0x57c), Some(1));
    assert_eq!(owner.word(0x748), Some(0x4080_0000));
    assert_eq!(owner.word(0x74c), Some(0x4080_0000));
    assert_eq!(owner.bindings.get(&0x86c), Some(&Binding::SelfPlayer));
    assert_eq!(owner.word(0x86c), None);
    assert_eq!(owner.word(0x224), None);
    assert_eq!((owner.reset_calls(), owner.completed_resets()), (2, 2));
    let calls = provider.calls.clone();
    assert!(owner.finish_runtime_reset(&mut provider).is_err());
    assert_eq!(provider.calls, calls);
}

#[test]
fn absent_descriptor_is_lazy_and_other_actual_modes_write_one() {
    for (mode, byte) in [(None, 0), (Some(1), 0), (Some(0), 1), (Some(u32::MAX), 1)] {
        let mut owner = later_prefix();
        let mut provider = RuntimeProvider::new(mode);
        owner.finish_runtime_reset(&mut provider).unwrap();
        assert_eq!(provider.calls.last(), Some(&("byte", byte)));
        assert_eq!(
            provider.calls.iter().any(|&(name, _)| name == "mode"),
            mode.is_some()
        );
    }
}

#[test]
fn failed_or_cancelled_callbacks_retain_reached_fields_and_never_replay() {
    for fail in ["float", "present", "mode", "byte"] {
        let mut owner = later_prefix();
        owner.w(0x960, 91); // distinguish the final reached store.
        let mut provider = RuntimeProvider::new(Some(2));
        provider.fail = Some(fail); // also models a cancelled generation lease.
        assert!(owner.finish_runtime_reset(&mut provider).is_err());
        assert_eq!(owner.stage(), Stage::RuntimeResetTail);
        assert_eq!(owner.word(0x708), Some(1));
        assert_eq!(owner.word(0x960), Some(91));
        assert_eq!(
            owner.failure(),
            Some(match fail {
                "float" => Failure::RuntimeGlobalFloat,
                "byte" => Failure::RuntimeGlobalByte,
                _ => Failure::RuntimeDescriptor,
            })
        );
        if fail == "float" {
            assert_ne!(owner.bindings.get(&0x86c), Some(&Binding::SelfPlayer));
        } else {
            assert_eq!(owner.bindings.get(&0x86c), Some(&Binding::SelfPlayer));
            assert_eq!(owner.word(0x748), Some(0x4080_0000));
        }
        let calls = provider.calls.clone();
        assert!(owner.finish_runtime_reset(&mut provider).is_err());
        assert_eq!(provider.calls, calls);
    }
}

#[test]
fn actual_nonnull_backing_or_interface_requires_a_real_provider() {
    for offset in [0x610, 0x4f8] {
        let mut owner = later_prefix();
        owner.w(offset, 99);
        let mut provider = RuntimeProvider::new(Some(1));
        assert!(owner.finish_runtime_reset(&mut provider).is_err());
        assert_eq!(owner.failure(), Some(Failure::RuntimeDependency));
        if offset == 0x610 {
            assert!(provider.calls.is_empty());
            assert_eq!(owner.word(0x610), Some(99));
        } else {
            assert_eq!(provider.calls, [("float", 0)]);
            assert_eq!(owner.word(0x6e8), Some(0));
        }
    }
}

#[test]
fn cannot_run_runtime_tail_on_initial_basic_constructor_or_second_reset_without_prefix() {
    let mut source = Providers::new();
    let mut owner = prefix(&mut source);
    owner.apply_reset(0, &mut source).unwrap();
    let mut provider = RuntimeProvider::new(None);
    assert!(owner.finish_runtime_reset(&mut provider).is_err());
    owner.finish_basic().unwrap();
    assert!(owner.finish_runtime_reset(&mut provider).is_err());
    owner.apply_reset(0, &mut source).unwrap();
    assert!(owner.finish_runtime_reset(&mut provider).is_err());
    assert!(provider.calls.is_empty());
}
