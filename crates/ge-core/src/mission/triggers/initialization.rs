//! Kind-specific post-common writes. The registry owns the surrounding source
//! scratch/membership/status order; provider-dependent paths remain staged.
use super::{Trigger, TriggerKind, Unsupported};

impl Trigger {
    pub(super) fn initialize_derived(
        &mut self,
        factory: &mut impl super::Factory,
    ) -> anyhow::Result<Option<Unsupported>> {
        match self.kind {
            TriggerKind::Radius88 => self
                .radius88
                .as_mut()
                .expect("constructed subtype88")
                .initialize(self.common.numeric[0]),
            TriggerKind::AuxAc => self
                .aux_ac
                .as_mut()
                .expect("constructed subtypeAC")
                .initialize(self.common.numeric[1]),
            TriggerKind::Shape71 => return self.initialize_shape71(factory),
            TriggerKind::StartPoint | TriggerKind::CommonBe | TriggerKind::Common09 => {}
            TriggerKind::Radius74 => {
                let state = super::radius74::State::from_serialized(
                    self.common.numeric[0],
                    self.common.numeric[1],
                    self.common.numeric[2],
                );
                self.runtime_numeric[0] = state.radius_bits();
                self.runtime_numeric[1] = state.file_reference();
                self.runtime_numeric[2] = state.numeric2_raw();
                self.radius74 = Some(state);
            }
            TriggerKind::Radius1b => {
                self.radius1b = Some(super::radius1b::State::from_serialized(
                    self.common.numeric[0],
                ));
                self.runtime_numeric[14] = 0;
            }
            TriggerKind::Common8A => {
                let state =
                    super::common8a::State::from_serialized(self.common.header_flags_raw, || {
                        self.common.numeric[0]
                    });
                self.common8a = Some(state);
                if state.gate() == super::common8a::Gate::RequiresHook {
                    return Ok(Some(Unsupported::Common8AHook { trigger: self.id }));
                }
            }
            TriggerKind::Radius03 => {
                let state = super::radius03::State::from_serialized(
                    self.common.numeric[0],
                    self.common.numeric[3],
                );
                self.radius03 = Some(state);
                if let Some(resource) = state.requested_resource() {
                    let request = super::radius03::Request {
                        trigger: self.id,
                        map: self.map,
                        ordinal: self.ordinal,
                        zone: self.zone,
                        resource,
                        fallback: 0,
                    };
                    match factory.radius03_resource(request)? {
                        super::actor12::ProviderOutcome::Ready(value) => self
                            .radius03
                            .as_mut()
                            .expect("owned subtype03")
                            .complete(value),
                        super::actor12::ProviderOutcome::Unavailable => {
                            return Ok(Some(Unsupported::Radius03Provider {
                                trigger: self.id,
                                resource,
                            }));
                        }
                    }
                }
            }
            TriggerKind::Generic => {
                let state = super::generic::State::from_serialized(
                    self.common.numeric[0],
                    self.common.numeric[1],
                );
                self.runtime_numeric[0] = state.radius_bits();
                self.runtime_numeric[1] = state.resource_reference();
                self.generic = Some(state);
                if state.requires_resource_provider() {
                    return Ok(Some(Unsupported::GenericResource {
                        trigger: self.id,
                        resource: state.resource_reference(),
                    }));
                }
            }
            TriggerKind::Resource08 => {
                let state = super::resource08::State::from_serialized(self.common.numeric[1]);
                self.resource08 = Some(state);
                if let Some(resource) = state.requested_resource() {
                    return Ok(Some(Unsupported::Resource08Provider {
                        trigger: self.id,
                        resource,
                    }));
                }
            }
            TriggerKind::Active91 => {
                self.runtime_flags_raw = super::active91::initialize_runtime_flags(
                    self.runtime_header_flags_raw,
                    self.runtime_flags_raw,
                );
            }
            TriggerKind::Range0c => {
                let state = super::range0c::State::from_serialized(
                    self.common.numeric[0],
                    self.common.numeric[2],
                    self.common.numeric[4],
                    self.common.numeric[13],
                );
                self.runtime_numeric[0] = state.radius_bits();
                self.runtime_numeric[2] = state.secondary_radius_bits();
                self.runtime_numeric[4] = state.animation_reference();
                self.runtime_numeric[13] = state.resource_reference();
                self.runtime_flags_raw =
                    super::range0c::initialize_runtime_flags(self.runtime_flags_raw);
                self.range0c = Some(state);
                if let Some(resource) = state.requested_resource() {
                    return Ok(Some(Unsupported::Range0cProvider {
                        trigger: self.id,
                        resource,
                    }));
                }
            }
            TriggerKind::Radius06 => {
                let state = super::radius06::State::from_serialized(
                    self.common.numeric[0],
                    self.common.numeric[4],
                );
                self.runtime_numeric[0] = state.radius_bits();
                self.runtime_numeric[4] = state.resource_reference();
                self.radius06 = Some(state);
                if let Some(resource) = state.requested_resource() {
                    return Ok(Some(Unsupported::Radius06Provider {
                        trigger: self.id,
                        resource,
                    }));
                }
            }
            TriggerKind::Actor12 => return self.initialize_actor12(factory),
            TriggerKind::Ad => {
                let state = super::ad::State::from_serialized(
                    self.common.numeric[0],
                    self.common.numeric[2],
                    self.common.numeric[15],
                );
                self.runtime_numeric[0] = state.radius_bits();
                self.runtime_numeric[2] = state.numeric2_reference();
                self.runtime_numeric[15] = state.numeric15_reference();
                self.ad = Some(state);
            }
            TriggerKind::A3 => {
                self.a3 = Some(super::a3::State::from_serialized(self.common.numeric[0]));
            }
            TriggerKind::Range27 => {
                let state = super::range27::State::from_serialized(
                    self.common.numeric[0],
                    self.common.numeric[2],
                    self.common.numeric[4],
                    self.common.numeric[13],
                );
                self.runtime_numeric[0] = state.radius_bits();
                self.runtime_numeric[2] = state.secondary_radius_bits();
                self.runtime_numeric[4] = state.animation_reference();
                self.runtime_numeric[13] = state.resource_reference();
                self.runtime_flags_raw =
                    super::range27::initialize_runtime_flags(self.runtime_flags_raw);
                self.range27 = Some(state);
                if let Some(resource) = state.requested_resource() {
                    return Ok(Some(Unsupported::Range27Provider {
                        trigger: self.id,
                        resource,
                    }));
                }
            }
            TriggerKind::Range82 => {
                let state = super::range82::State::from_serialized(&self.common.numeric);
                self.runtime_numeric[0] = state.radius_bits();
                self.runtime_numeric[2] = state.secondary_radius_bits();
                self.runtime_numeric[4] = state.animation_reference();
                self.runtime_numeric[13] = state.resource_reference();
                self.runtime_flags_raw =
                    super::range82::initialize_runtime_flags(self.runtime_flags_raw);
                self.range82 = Some(state);
                if let Some(resource) = state.requested_resource() {
                    return Ok(Some(Unsupported::Range82Provider {
                        trigger: self.id,
                        resource,
                    }));
                }
                self.runtime_numeric[5] = state
                    .numeric5_bits()
                    .expect("completed provider-free subtype82");
                self.runtime_numeric[7] = state
                    .numeric7_bits()
                    .expect("completed provider-free subtype82");
                self.runtime_numeric[8] = state
                    .numeric8_reference()
                    .expect("completed provider-free subtype82");
            }
        }
        Ok(None)
    }
}
