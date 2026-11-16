//! Preload uses the same Shared allocations, actual FP wrapper and retained
//! source-order weapon registry. Temporary animation is a separate owner.
use super::{Owned as Player, loadout, world_hooks};
use crate::game::world::post_trigger_resources::FileBinding;
use anyhow::{Context, Result, ensure};
use ge_core::mission::player::{
    components::{Identity, Preparation},
    constructor::{
        initializer_prefix,
        preload::{self, Mapping, Request, Source},
    },
    world::World,
};
use std::collections::VecDeque;
mod completion;
mod temporary;

pub(super) struct CallOwned {
    pub request: Request,
    pub mapping: ge_assets::weapon_static::Remap,
    pub extraction: Option<Option<ge_assets::animation_modes::ClassFourReference>>,
}
pub(super) struct Owned {
    pub state: preload::Pending,
    pub calls: Vec<CallOwned>,
    pub temporary: Option<ge_core::mission::player::components::temporary::Pending>,
    pub post_bind: Option<preload::post_bind::PostBind>,
    pub outputs: Option<preload::bone_outputs::BoneOutputs>,
    pub teardown: Option<preload::teardown::Teardown>,
    pub pre_teardown: Option<super::evidence::PreloadSnapshot>,
    pub tombstone: Option<preload::teardown::Tombstone>,
    pub completed_temporaries: VecDeque<CompletedTemporary>,
    pub first_completed_snapshot: Option<super::evidence::PreloadSnapshot>,
    pub completed_temporary_count: usize,
    pub discarded_temporary_count: usize,
    pub found_output_count: usize,
    pub missing_output_count: usize,
    pub destroyed_temporary_count: usize,
    pub entry_commit_count: usize,
}
pub(super) struct CompletedTemporary {
    pub snapshot: super::evidence::PreloadSnapshot,
    pub tombstone: preload::teardown::Tombstone,
}
struct Sources<'a> {
    loadout: &'a loadout::Sources,
    preparation: &'a Preparation,
    fp_source: Option<&'a FileBinding>,
    receiver: Identity,
    calls: &'a mut Vec<CallOwned>,
    hook: &'a world_hooks::Attempt,
}
impl Sources<'_> {
    fn file(&self, receiver: Identity) -> Result<&FileBinding> {
        self.hook.ensure_current()?;
        ensure!(receiver == self.receiver, "preload FP receiver changed");
        let owner = self.preparation.mode_animator(receiver)?;
        let source = self
            .fp_source
            .context("preload actual FP file wrapper unavailable")?;
        ensure!(
            owner
                .component()
                .is_some_and(|binding| binding.file == source.file()),
            "preload FP wrapper differs from current receiver"
        );
        Ok(source)
    }
}
impl Source for Sources<'_> {
    fn registry_count(&mut self) -> Result<usize> {
        self.hook.ensure_current()?;
        Ok(self.loadout.preload_registry_count())
    }
    fn enabled_byte(&mut self, ordinal: usize) -> Result<u8> {
        self.hook.ensure_current()?;
        self.loadout.preload_enabled_byte(ordinal)
    }
    fn weapon(&mut self, ordinal: usize) -> Result<u32> {
        self.hook.ensure_current()?;
        self.loadout.preload_weapon(ordinal)
    }
    fn remap(&mut self, request: Request) -> Result<Mapping> {
        self.hook.ensure_current()?;
        ensure!(
            request.receiver == self.receiver && request.ordinal == self.calls.len(),
            "preload mapping receiver/ordinal changed"
        );
        self.calls.try_reserve(1)?;
        // Availability is lazy even when the actual mapping fails before it
        // needs the FP wrapper. No live action invocation is fabricated here.
        let mapping = self.loadout.map_preload(request, |mode| {
            self.file(request.receiver)?.mode_available(mode)
        })?;
        let output = Mapping {
            output: mapping.output,
            accepted: mapping.mapped,
        };
        self.calls.push(CallOwned {
            request,
            mapping,
            extraction: None,
        });
        Ok(output)
    }
    fn extract_reference(&mut self, receiver: Identity, mapped: u32) -> Result<Option<u32>> {
        self.hook.ensure_current()?;
        let call = self
            .calls
            .last()
            .context("preload extraction has no actual mapping")?;
        ensure!(
            call.request.receiver == receiver
                && call.mapping.output == mapped
                && call.extraction.is_none(),
            "preload extraction changed input or replays"
        );
        let result = self.file(receiver)?.preload_class_four_reference(mapped)?;
        self.calls
            .last_mut()
            .expect("retained mapped call")
            .extraction = Some(result);
        Ok(result.map(|reference| reference.reference))
    }
}

pub(super) fn initialize(
    player: &mut Player,
    world: &World,
    identities: &mut ge_core::mission::player::components::Registry,
    hook: &world_hooks::Attempt,
) -> Result<()> {
    hook.ensure_current()?;
    let Some(prefix) = player
        .initializer_prefix
        .as_ref()
        .filter(|prefix| prefix.stage() == initializer_prefix::Stage::AwaitingPreload)
    else {
        return Ok(());
    };
    ensure!(
        player.preload.is_none() && player.completed_initializer_evidence.is_none(),
        "preload helper entry replays"
    );
    player.completed_initializer_evidence = Some(super::evidence::capture_initializer_prefix(
        player, world, hook,
    )?);
    player.preload = Some(Owned {
        state: preload::Pending::begin(prefix, player.lease)?,
        calls: Vec::new(),
        temporary: None,
        post_bind: None,
        outputs: None,
        teardown: None,
        pre_teardown: None,
        tombstone: None,
        completed_temporaries: VecDeque::new(),
        first_completed_snapshot: None,
        completed_temporary_count: 0,
        discarded_temporary_count: 0,
        found_output_count: 0,
        missing_output_count: 0,
        destroyed_temporary_count: 0,
        entry_commit_count: 0,
    });
    let owned = player.preload.as_mut().expect("retained preload");
    let mut source = Sources {
        loadout: &player.loadout,
        preparation: &player.components.preparation,
        fp_source: player.components.first_person.component_source.as_ref(),
        receiver: owned.state.receiver(),
        calls: &mut owned.calls,
        hook,
    };
    hook.with_player_preload(|shared, caches| {
        owned
            .state
            .advance(shared, caches, &mut source, preload::Budget::default())
    })?;
    hook.ensure_current()?;
    // The global preload lock is released before binding/evaluating through
    // the same controller's animation Context and Arena.
    loop {
        hook.ensure_current()?;
        let owned = player.preload.as_ref().expect("retained preload");
        if owned.state.stage() != preload::Stage::AwaitingTemporary {
            return Ok(());
        }
        if owned.temporary.is_none() && owned.tombstone.is_none() {
            temporary::initialize(player, identities, hook)?;
        }
        if !completion::advance(player, identities, hook)? {
            return Ok(());
        }
        let owned = player.preload.as_mut().expect("retained preload");
        let mut source = Sources {
            loadout: &player.loadout,
            preparation: &player.components.preparation,
            fp_source: player.components.first_person.component_source.as_ref(),
            receiver: owned.state.receiver(),
            calls: &mut owned.calls,
            hook,
        };
        let tombstone = owned
            .tombstone
            .as_ref()
            .context("preload resume lacks destroyed temporary")?;
        let result = hook.with_player_preload(|shared, caches| {
            owned.state.resume_call(
                shared,
                caches,
                &mut source,
                preload::Budget::default(),
                tombstone,
            )
        });
        completion::retain_committed(owned)?;
        result?;
    }
}
