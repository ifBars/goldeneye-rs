//! Test-only builders that drive the real temporary bind path. Authored
//! synthetic resources; no original skin, script or animation payload.
use super::*;
use crate::{animation::skeleton::BoneInput, mission::player::identity::PlayerId};

pub(crate) const PRIMARY: ResourceBinding = ResourceBinding {
    file: 0x010a_0037,
    reference: 0x0d00_016f,
};
pub(crate) struct NullBody;
impl script_child::Provider for NullBody {
    fn file(&self) -> u32 {
        0x0102_0016
    }
    fn generation(&mut self) -> Result<u32> {
        Ok(4)
    }
    fn resolve_script(&mut self, _: u32) -> Result<Option<script_child::Binding>> {
        Ok(None)
    }
    fn bind_body(&mut self, _: script_child::Binding) -> Result<bool> {
        anyhow::bail!("unresolved body must not bind")
    }
}
impl script_child::forward_tick::Source for NullBody {
    fn duration(&mut self, _: script_child::Binding) -> Result<u32> {
        anyhow::bail!("absent body duration")
    }
    fn fps_bits(&mut self, _: script_child::Binding) -> Result<u32> {
        anyhow::bail!("absent body FPS")
    }
}
/// Owner-bound skin publication with a call log.
pub(crate) struct Skin {
    pub lookups: Vec<SkinLookup>,
    pub bodies: Vec<SkinBody>,
    pub generation: u32,
    pub calls: Vec<&'static str>,
}
impl Skin {
    pub(crate) fn resident() -> Self {
        Self {
            lookups: vec![SkinLookup::Present(SkinDescriptor {
                file: PRIMARY.file,
                reference: PRIMARY.reference,
                generation: 9,
            })],
            bodies: vec![SkinBody::Resident],
            generation: 9,
            calls: Vec::new(),
        }
    }
}
impl SkinSource for Skin {
    fn lookup(&mut self, primary: ResourceBinding) -> Result<SkinLookup> {
        ensure!(primary == PRIMARY, "authored skin lookup owner differs");
        self.calls.push("lookup");
        ensure!(!self.lookups.is_empty(), "authored skin lookup exhausted");
        Ok(self.lookups.remove(0))
    }
    fn generation(&mut self, _: ResourceBinding) -> Result<u32> {
        self.calls.push("generation");
        Ok(self.generation)
    }
    fn body(&mut self, _: SkinDescriptor) -> Result<SkinBody> {
        self.calls.push("body");
        ensure!(!self.bodies.is_empty(), "authored skin body exhausted");
        Ok(self.bodies.remove(0))
    }
}
fn bones() -> [BoneInput; 1] {
    [BoneInput {
        parent: u16::MAX,
        range_end: 1,
        auxiliary_raw: [0; 2],
    }]
}
fn input(bones: &[BoneInput]) -> SkeletonInput<'_> {
    SkeletonInput {
        primary: PRIMARY,
        bones,
        auxiliary_words: 0,
        captured_frame_counter: None,
    }
}
/// An FP receiver whose captured secondary pair is `PRIMARY`.
pub(crate) fn receiver(registry: &mut Registry) -> Animator {
    let identity = registry.allocate(PlayerId::new(1).unwrap()).unwrap();
    let mut fp = Animator::from_constructor(identity);
    fp.bind_temporary_skeleton(
        input(&bones()),
        ResourceBinding {
            file: 0x0102_0016,
            reference: 0x8400_0022,
        },
        SkeletonBudget::default(),
    )
    .unwrap();
    fp
}
/// A temporary of `fp` whose actual bind returned 1. Reference class four
/// takes the nonempty NULL-body path; reference 0 the empty list.
pub(crate) fn bind_from(
    fp: &Animator,
    registry: &mut Registry,
    reference: u32,
    context: &mut scratch::Context,
    arena: &mut scratch::Arena,
) -> Pending {
    let mut pending = Pending::begin(fp, registry, reference).unwrap();
    pending
        .bind_skeleton(input(&bones()), SkeletonBudget::default())
        .unwrap();
    pending.initialize_factory(&mut NullBody).unwrap();
    if pending.stage() == Stage::AwaitingInsertion {
        pending.finish_factory().unwrap();
    }
    pending.forward_zero(&mut NullBody).unwrap();
    if pending.head().is_some() {
        pending
            .evaluate_bind_zero_time(context, arena, &mut Skin::resident())
            .unwrap();
    } else {
        pending.finish_bind_zero_time(context, arena).unwrap();
    }
    assert_eq!(pending.bind_return(), Some(1));
    pending
}
/// The FP receiver and a temporary, both allocated from the supplied registry.
pub(crate) fn bind_returned(
    registry: &mut Registry,
    reference: u32,
    context: &mut scratch::Context,
    arena: &mut scratch::Arena,
) -> (Animator, Pending) {
    let fp = receiver(registry);
    let pending = bind_from(&fp, registry, reference, context, arena);
    (fp, pending)
}
/// Authored owner-word store on the temporary (test only), to reach the
/// destructor's populated/unknown-state dependencies.
pub(crate) fn store_animator_word(pending: &mut Pending, at: usize, bits: Option<u32>) {
    crate::mission::player::components::animator::fixtures::store_word(
        &mut pending.animator,
        at,
        bits,
    );
}

/// Commits authored locals into the temporary's actual skeleton (test only).
pub(crate) fn commit_temporary_pose(
    pending: &mut Pending,
    locals: &[crate::animation::skeleton::LocalPose],
) -> Result<usize> {
    let runtime =
        crate::mission::player::components::animator::fixtures::skeleton_mut(&mut pending.animator)
            .context("temporary skeleton absent")?;
    crate::mission::player::components::skeleton::fixtures::commit_pose(runtime, locals)
}

/// Authored sparse word store on the temporary's current head header.
pub(crate) fn store_head_word(pending: &mut Pending, at: usize, bits: Option<u32>) {
    let head = pending.head.expect("authored head present");
    store_script_word(pending, head, at, bits);
}

/// Authored store on a retained header, including an unlinked destructor owner.
pub(crate) fn store_script_word(
    pending: &mut Pending,
    node: crate::animation::layers::NodeId,
    at: usize,
    bits: Option<u32>,
) {
    crate::mission::player::components::animator::fixtures::store_header_word(
        &mut pending.animator,
        node,
        at,
        bits,
    );
}
