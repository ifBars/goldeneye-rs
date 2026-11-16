use super::*;
use crate::{
    animation::{scratch, skeleton::LocalPose},
    mission::player::{
        components::{
            Animator, Registry,
            temporary::fixtures::{PRIMARY, Skin, bind_from, commit_temporary_pose, receiver},
        },
        constructor::{Budget, preload::Mapping, preload::post_bind::ProcessScalar},
        identity::PlayerId,
    },
};

// Authored synthetic owners, hashes and tags; no original data.
pub(in crate::mission::player::constructor::preload) const REFERENCE: u32 = 0x8400_0007;
pub(in crate::mission::player::constructor::preload) const MAPPED: u32 = 0x0900_0123;
pub(in crate::mission::player::constructor::preload) struct World {
    pub shared: Shared,
    pub lease: Lease,
    pub registry: Registry,
    pub context: scratch::Context,
    pub arena: scratch::Arena,
    pub fp: Animator,
    pub temporary: temporary::Pending,
    pub call: Call,
    pub post: PostBind,
}
pub(in crate::mission::player::constructor::preload) fn shared() -> (Shared, Lease) {
    let mut shared = Shared::from_boot();
    let lease = shared
        .acquire(
            PlayerId::new(1).unwrap(),
            Budget {
                native_bytes: 63_680,
                leases: 4,
            },
            || Ok(()),
        )
        .unwrap();
    (shared, lease)
}
/// A call at its temporary with returned extraction and a post-bind suffix
/// already at AwaitingBoneOutputs, all on the same retained owners.
pub(in crate::mission::player::constructor::preload) fn world(entry: u8) -> World {
    let (mut shared, lease) = shared();
    shared.clear_preload_entry_key(lease, entry).unwrap();
    let mut registry = Registry::from_constructor(64);
    let mut context = scratch::Context::default();
    let mut arena = scratch::Arena::from_constructor(31).unwrap();
    let fp = receiver(&mut registry);
    let mut temporary = bind_from(&fp, &mut registry, REFERENCE, &mut context, &mut arena);
    let mut call = Call::begin(Request {
        player: fp.player(),
        receiver: fp.identity(),
        ordinal: 0,
        source_ordinal: 0,
        weapon: 0x55,
        alternate: false,
        variant: 0,
    })
    .unwrap();
    call.mapping = Some(Mapping {
        output: MAPPED,
        accepted: true,
    });
    call.entry_index = Some(entry);
    call.extraction_returned = true;
    call.reference = Some(REFERENCE);
    call.stage = CallStage::AwaitingTemporary;
    let mut post = PostBind::begin(&call, &temporary).unwrap();
    let mut scalar = ProcessScalar::from_startup(5).unwrap();
    post.advance(
        &mut temporary,
        &mut scalar,
        &mut context,
        &mut arena,
        &mut Skin::resident(),
    )
    .unwrap();
    World {
        shared,
        lease,
        registry,
        context,
        arena,
        fp,
        temporary,
        call,
        post,
    }
}
/// Tag provider over the same skin pair. Each lookup/body/tag answer can be
/// queued; defaults are a present descriptor, a resident body and `found`.
pub(in crate::mission::player::constructor::preload) struct Tags {
    pub hashes: Vec<u32>,
    pub lookups: Vec<SkinLookup>,
    pub bodies: Vec<SkinBody>,
    pub walks: Vec<TagWalk>,
    pub found: TagWalk,
    pub generation: u32,
    pub fail: Option<&'static str>,
    pub log: Vec<String>,
}
impl Tags {
    pub fn found(hashes: [u32; 6]) -> Self {
        Self {
            hashes: hashes.to_vec(),
            lookups: Vec::new(),
            bodies: Vec::new(),
            walks: Vec::new(),
            found: TagWalk::Found { bone: 0 },
            generation: 9,
            fail: None,
            log: Vec::new(),
        }
    }
    fn read(&mut self, name: String) -> Result<()> {
        ensure!(
            self.fail != name.split(':').next(),
            "authored provider failure at {name}"
        );
        self.log.push(name);
        Ok(())
    }
    fn descriptor() -> SkinDescriptor {
        SkinDescriptor {
            file: PRIMARY.file,
            reference: PRIMARY.reference,
            generation: 9,
        }
    }
}
impl SkinSource for Tags {
    fn lookup(&mut self, primary: ResourceBinding) -> Result<SkinLookup> {
        ensure!(primary == PRIMARY, "tag lookup owner differs");
        self.read("lookup".into())?;
        Ok(if self.lookups.is_empty() {
            SkinLookup::Present(Self::descriptor())
        } else {
            self.lookups.remove(0)
        })
    }
    fn generation(&mut self, _: ResourceBinding) -> Result<u32> {
        self.read("generation".into())?;
        Ok(self.generation)
    }
    fn body(&mut self, _: SkinDescriptor) -> Result<SkinBody> {
        self.read("body".into())?;
        Ok(if self.bodies.is_empty() {
            SkinBody::Resident
        } else {
            self.bodies.remove(0)
        })
    }
}
impl BoneTagSource for Tags {
    fn hash(&mut self, index: u8) -> Result<u32> {
        self.read(format!("hash:{index}"))?;
        self.hashes
            .get(usize::from(index))
            .copied()
            .context("authored hash outside table")
    }
    fn tag(&mut self, _: SkinDescriptor, hash: u32) -> Result<TagWalk> {
        self.read(format!("tag:{hash:#x}"))?;
        Ok(if self.walks.is_empty() {
            self.found
        } else {
            self.walks.remove(0)
        })
    }
}
pub(in crate::mission::player::constructor::preload) const HASHES: [u32; 6] = [
    0x7100_0101,
    0x7100_0102,
    0x7100_0103,
    0x7100_0104,
    0x7100_0105,
    0x7100_0106,
];
fn run(w: &mut World, outputs: &mut BoneOutputs, tags: &mut Tags) -> Result<()> {
    outputs.advance(w.lease, &mut w.shared, &w.temporary, tags)
}
fn index(w: &World, entry: u8, ordinal: u8) -> Option<u16> {
    w.shared
        .preload_entry_bone_index(w.lease, entry, ordinal)
        .unwrap()
}
fn lanes(w: &World, entry: u8, region: PoseRegion, ordinal: u8) -> [Option<u32>; 4] {
    std::array::from_fn(|lane| {
        w.shared
            .preload_entry_pose_word(w.lease, entry, region, ordinal, lane as u8)
            .unwrap()
    })
}
fn bone_bits(w: &World) -> ([u32; 4], [u32; 4]) {
    let bone = &w
        .temporary
        .animator()
        .skeleton()
        .unwrap()
        .skeleton()
        .bones()[0];
    (bone.local_translation_bits(), bone.local_quaternion_bits())
}

#[test]
fn six_found_tags_copy_the_actual_current_bone_record_into_the_selected_entry() {
    let mut w = world(3);
    let mut outputs = BoneOutputs::begin(&w.call, &w.post, &w.temporary).unwrap();
    let mut tags = Tags::found(HASHES);
    run(&mut w, &mut outputs, &mut tags).unwrap();
    assert_eq!(outputs.stage(), Stage::Done);
    assert_eq!(outputs.dependency(), Some(Dependency::Destructor));
    assert_eq!(outputs.stores(), 30);
    assert_eq!((outputs.hash_reads(), outputs.hash_cursor()), (6, 6));
    let (translation, rotation) = bone_bits(&w);
    for ordinal in 0..6 {
        assert_eq!(index(&w, 3, ordinal), Some(0));
        assert_eq!(
            lanes(&w, 3, PoseRegion::Translation, ordinal),
            translation.map(Some)
        );
        assert_eq!(
            lanes(&w, 3, PoseRegion::Rotation, ordinal),
            rotation.map(Some)
        );
    }
    // The key is still the cleared zero; neighbors stay unwritten.
    assert_eq!(w.shared.preload_entry_key(w.lease, 3).unwrap(), 0);
    assert_eq!(index(&w, 4, 0), None);
    // Source order per iteration: lookup, body, tag; no matrix provider exists.
    assert_eq!(
        &tags.log[..4],
        ["hash:0", "lookup", "body", "tag:0x71000101"]
    );
    // Nothing later happened: temporary alive, no key commit, no flag change.
    assert!(!w.temporary.teardown_started());
    assert_eq!(w.temporary.bind_return(), Some(1));
}

#[test]
fn a_real_committed_pose_flows_through_instead_of_a_fixed_identity() {
    let mut w = world(0);
    let sentinel = LocalPose {
        translation_bits: [0x3f00_0000, 0xbf80_0000, 0x4000_0000, 0x3f80_0000],
        quaternion_bits: [0x3e80_0000, 0, 0xbe80_0000, 0x3f40_0000],
    };
    commit_temporary_pose(&mut w.temporary, &[sentinel]).unwrap();
    let mut outputs = BoneOutputs::begin(&w.call, &w.post, &w.temporary).unwrap();
    run(&mut w, &mut outputs, &mut Tags::found(HASHES)).unwrap();
    assert_eq!(
        lanes(&w, 0, PoseRegion::Translation, 5),
        sentinel.translation_bits.map(Some)
    );
    assert_eq!(
        lanes(&w, 0, PoseRegion::Rotation, 0),
        sentinel.quaternion_bits.map(Some)
    );
}

#[test]
fn source_null_results_store_only_the_missing_index() {
    let mut w = world(0);
    let mut outputs = BoneOutputs::begin(&w.call, &w.post, &w.temporary).unwrap();
    let mut tags = Tags::found(HASHES);
    tags.lookups = vec![SkinLookup::Present(Tags::descriptor()), SkinLookup::Absent];
    tags.bodies = vec![SkinBody::Resident, SkinBody::Absent];
    tags.walks = vec![TagWalk::Found { bone: 0 }, TagWalk::Sentinel];
    run(&mut w, &mut outputs, &mut tags).unwrap();
    let outcomes = outputs.outcomes();
    assert_eq!(outcomes[0], Some(Outcome::Found { bone: 0 }));
    assert_eq!(
        outcomes[1],
        Some(Outcome::Missing(Missing::DescriptorAbsent))
    );
    assert_eq!(outcomes[2], Some(Outcome::Missing(Missing::BodyAbsent)));
    assert_eq!(outcomes[3], Some(Outcome::Missing(Missing::Sentinel)));
    for ordinal in 1..4 {
        assert_eq!(index(&w, 0, ordinal), Some(0xffff));
        assert_eq!(lanes(&w, 0, PoseRegion::Translation, ordinal), [None; 4]);
        assert_eq!(lanes(&w, 0, PoseRegion::Rotation, ordinal), [None; 4]);
    }
    // Missing results advance the hash cursor like found ones.
    assert_eq!(outputs.hash_cursor(), 6);
}

#[test]
fn zero_hash_stores_missing_and_rereads_the_same_word_without_lookups() {
    let mut w = world(0);
    let mut outputs = BoneOutputs::begin(&w.call, &w.post, &w.temporary).unwrap();
    let mut tags = Tags::found([
        0x7100_0101,
        0,
        0x7100_0103,
        0x7100_0104,
        0x7100_0105,
        0x7100_0106,
    ]);
    run(&mut w, &mut outputs, &mut tags).unwrap();
    let hashes: Vec<_> = tags.log.iter().filter(|l| l.starts_with("hash")).collect();
    assert_eq!(
        hashes,
        ["hash:0", "hash:1", "hash:1", "hash:1", "hash:1", "hash:1"]
    );
    assert_eq!(outputs.hash_cursor(), 1);
    assert_eq!(outputs.lookups(), 1);
    assert_eq!(outputs.outcomes()[0], Some(Outcome::Found { bone: 0 }));
    for ordinal in 1..6 {
        assert_eq!(
            outputs.outcomes()[usize::from(ordinal)],
            Some(Outcome::Missing(Missing::ZeroHash))
        );
        assert_eq!(index(&w, 0, ordinal), Some(0xffff));
    }
    assert!(!tags.log.iter().any(|l| l == "tag:0x71000103"));
}

#[test]
fn pending_providers_pause_without_stores_and_generation_change_fails() {
    let mut w = world(0);
    let mut outputs = BoneOutputs::begin(&w.call, &w.post, &w.temporary).unwrap();
    let mut tags = Tags::found(HASHES);
    tags.lookups = vec![SkinLookup::Pending];
    run(&mut w, &mut outputs, &mut tags).unwrap();
    assert_eq!(outputs.dependency(), Some(Dependency::SkinLookup));
    assert_eq!((outputs.stores(), index(&w, 0, 0)), (0, None));
    tags.bodies = vec![SkinBody::Pending];
    run(&mut w, &mut outputs, &mut tags).unwrap();
    assert_eq!(outputs.dependency(), Some(Dependency::SkinBody));
    tags.walks = vec![TagWalk::Pending];
    run(&mut w, &mut outputs, &mut tags).unwrap();
    assert_eq!(outputs.dependency(), Some(Dependency::Tag));
    assert_eq!(outputs.hash_reads(), 1);
    // The completed hash read is not repeated; a changed generation fails.
    tags.generation = 10;
    assert!(run(&mut w, &mut outputs, &mut tags).is_err());
    assert_eq!(outputs.stage(), Stage::Failed);
    assert_eq!((outputs.stores(), index(&w, 0, 0)), (0, None));
    assert!(run(&mut w, &mut outputs, &mut Tags::found(HASHES)).is_err());
}

#[test]
fn provider_errors_and_out_of_range_bones_latch_without_missing_stores() {
    for fail in ["lookup", "tag"] {
        let mut w = world(0);
        let mut outputs = BoneOutputs::begin(&w.call, &w.post, &w.temporary).unwrap();
        let mut tags = Tags::found(HASHES);
        tags.fail = Some(fail);
        assert!(run(&mut w, &mut outputs, &mut tags).is_err());
        assert_eq!(index(&w, 0, 0), None);
    }
    let mut w = world(0);
    let mut outputs = BoneOutputs::begin(&w.call, &w.post, &w.temporary).unwrap();
    let mut tags = Tags::found(HASHES);
    tags.walks = vec![TagWalk::Found { bone: 0 }, TagWalk::Found { bone: 1 }];
    assert!(run(&mut w, &mut outputs, &mut tags).is_err());
    assert_eq!(outputs.stores(), 5);
    assert_eq!((index(&w, 0, 0), index(&w, 0, 1)), (Some(0), None));
}

#[test]
fn begin_requires_this_calls_post_bind_temporary_and_abort_keeps_prefix() {
    let mut w = world(0);
    let mut early = Call::begin(w.call.request()).unwrap();
    early.stage = CallStage::AwaitingTemporary;
    early.extraction_returned = true;
    early.reference = w.call.reference();
    early.mapping = w.call.mapping();
    // Unselected entry.
    assert!(BoneOutputs::begin(&early, &w.post, &w.temporary).is_err());
    early.entry_index = Some(1);
    early.request.ordinal = 1;
    assert!(BoneOutputs::begin(&early, &w.post, &w.temporary).is_err());
    let mut outputs = BoneOutputs::begin(&w.call, &w.post, &w.temporary).unwrap();
    let mut tags = Tags::found(HASHES);
    tags.walks = vec![TagWalk::Found { bone: 0 }, TagWalk::Pending];
    run(&mut w, &mut outputs, &mut tags).unwrap();
    let words = |o: &BoneOutputs| {
        let mut v = Vec::new();
        o.visit_state_words(|x| v.push(x));
        v
    };
    let paused = words(&outputs);
    outputs.abort();
    assert_eq!(outputs.stage(), Stage::Aborted);
    assert_ne!(words(&outputs), paused);
    assert_eq!(index(&w, 0, 0), Some(0));
    assert!(run(&mut w, &mut outputs, &mut Tags::found(HASHES)).is_err());
    assert_eq!(index(&w, 0, 1), None);
    let _ = &w.fp;
}

#[test]
fn paused_state_words_distinguish_captured_hash_and_descriptor() {
    let paused = |hash: u32, generation: u32| {
        let mut w = world(0);
        let mut outputs = BoneOutputs::begin(&w.call, &w.post, &w.temporary).unwrap();
        let mut tags = Tags::found([hash, 1, 2, 3, 4, 5]);
        tags.lookups = vec![SkinLookup::Present(SkinDescriptor {
            generation,
            ..Tags::descriptor()
        })];
        tags.bodies = vec![SkinBody::Pending];
        run(&mut w, &mut outputs, &mut tags).unwrap();
        assert_eq!(outputs.dependency(), Some(Dependency::SkinBody));
        let mut words = Vec::new();
        outputs.visit_state_words(|word| words.push(word));
        words
    };
    let base = paused(0x7100_0101, 9);
    assert_eq!(base, paused(0x7100_0101, 9));
    assert_ne!(base, paused(0x7100_0202, 9));
    assert_ne!(base, paused(0x7100_0101, 8));
}
