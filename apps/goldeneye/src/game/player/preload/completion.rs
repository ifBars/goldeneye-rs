//! Outputs, explicit destruction and immutable diagnostics precede the key
//! commit. Live temporary access stops before its consuming final free.
use super::*;
use ge_core::mission::player::{
    components::Registry,
    constructor::preload::{
        bone_outputs::{BoneOutputs, Outcome, Stage as OutputStage},
        teardown::{Stage as TeardownStage, Teardown},
    },
};

pub(super) fn advance(
    player: &mut Player,
    identities: &mut Registry,
    hook: &world_hooks::Attempt,
) -> Result<bool> {
    hook.ensure_current()?;
    let owned = player
        .preload
        .as_mut()
        .context("preload completion owner absent")?;
    if owned.tombstone.is_some() {
        return Ok(true);
    }
    let Some(post) = owned
        .post_bind
        .as_ref()
        .filter(|post| post.stage() == preload::post_bind::Stage::AwaitingBoneOutputs)
    else {
        return Ok(false);
    };
    let temporary = owned
        .temporary
        .as_mut()
        .context("bone output temporary absent")?;
    let call = owned
        .state
        .current_call()
        .context("bone output actual call absent")?;
    if owned.outputs.is_none() {
        owned.outputs = Some(BoneOutputs::begin(call, post, temporary)?);
    }
    let outputs = owned.outputs.as_mut().expect("retained six-bone outputs");
    if outputs.stage() != OutputStage::Done {
        let skin = player
            .components
            .first_person
            .skin_source
            .as_ref()
            .context("bone output primary skin wrapper absent")?;
        let mut provider = skin.bone_output_source(player.loadout.preload_bone_tags());
        let lease = owned.state.lease();
        hook.with_player_preload(|shared, _| {
            outputs.advance(lease, shared, temporary, &mut provider)
        })?;
        hook.ensure_current()?;
        if outputs.stage() != OutputStage::Done {
            return Ok(false);
        }
    }
    if owned.pre_teardown.is_none() {
        let component = player
            .components
            .first_person
            .component_source
            .as_ref()
            .context("pre-destructor component wrapper absent")?;
        let publication = temporary
            .factory_header()
            .and_then(|header| header.cached())
            .map(|binding| component.script_publication_evidence(binding))
            .transpose()?
            .flatten();
        let process = hook.preload_scalar_facts(post.capture())?;
        let snapshot = super::super::evidence::capture_preload_snapshot(
            call,
            temporary,
            post,
            outputs,
            process,
            publication,
        )?;
        let found = outputs
            .outcomes()
            .iter()
            .filter(|outcome| matches!(outcome, Some(Outcome::Found { .. })))
            .count();
        let missing = outputs
            .outcomes()
            .iter()
            .filter(|outcome| matches!(outcome, Some(Outcome::Missing(_))))
            .count();
        owned.found_output_count = owned
            .found_output_count
            .checked_add(found)
            .context("preload output count overflow")?;
        owned.missing_output_count = owned
            .missing_output_count
            .checked_add(missing)
            .context("preload output count overflow")?;
        owned.pre_teardown = Some(snapshot);
    }
    if owned.teardown.is_none() {
        owned.teardown = Some(Teardown::begin(outputs, temporary)?);
    }
    let teardown = owned.teardown.as_mut().expect("retained destructor cursor");
    hook.ensure_current()?;
    teardown.advance(temporary)?;
    hook.ensure_current()?;
    if teardown.stage() != TeardownStage::AwaitingFree {
        return Ok(false);
    }
    let teardown = owned.teardown.take().expect("completed destructor prefix");
    let temporary = owned
        .temporary
        .take()
        .expect("live owner before final free");
    owned.tombstone = Some(teardown.finish(temporary)?);
    // Keep the tombstone even if registry bookkeeping fails after the free.
    owned.destroyed_temporary_count = owned
        .destroyed_temporary_count
        .checked_add(1)
        .context("preload destruction count overflow")?;
    let tombstone = owned.tombstone.as_ref().expect("destroyed owner receipt");
    identities.retire(tombstone.temporary())?;
    hook.ensure_current()?;
    Ok(true)
}

/// A resume may fail while advancing the NEXT call, after this key was already
/// committed. Preserve that completed prefix before propagating the error.
pub(super) fn retain_committed(owned: &mut Owned) -> Result<()> {
    let Some(tombstone) = &owned.tombstone else {
        return Ok(());
    };
    let request = tombstone.binding().request;
    let committed = owned
        .state
        .completed_calls()
        .get(request.ordinal)
        .into_iter()
        .chain(owned.state.current_call())
        .any(|call| {
            call.request() == request
                && call.return_word() == Some(1)
                && call.return_reason() == Some(preload::ReturnReason::TemporaryCompleted)
                && call.temporary_completed() == Some(tombstone.temporary())
        });
    if !committed {
        return Ok(());
    }
    let snapshot = owned
        .pre_teardown
        .take()
        .context("committed temporary lost pre-destructor snapshot")?;
    if owned.first_completed_snapshot.is_none() {
        owned.first_completed_snapshot = Some(snapshot.clone());
    }
    let tombstone = owned.tombstone.take().expect("actual committed tombstone");
    owned.completed_temporary_count = owned
        .completed_temporary_count
        .checked_add(1)
        .context("preload completed count overflow")?;
    owned.entry_commit_count = owned
        .entry_commit_count
        .checked_add(1)
        .context("preload entry commit count overflow")?;
    if owned.completed_temporaries.len() == 16 {
        owned.completed_temporaries.pop_front();
        owned.discarded_temporary_count = owned
            .discarded_temporary_count
            .checked_add(1)
            .context("preload history count overflow")?;
    }
    owned.completed_temporaries.push_back(CompletedTemporary {
        snapshot,
        tombstone,
    });
    owned.outputs = None;
    owned.post_bind = None;
    owned.teardown = None;
    Ok(())
}
