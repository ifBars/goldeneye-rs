use super::*;

const LEN: usize = 0x600;
const ENTITIES: usize = 0x130;
const TEXTURES: usize = 0x1b0;
const BODY: usize = 0x240;
const PARTS: usize = 0x2c0;

fn word(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn half(data: &mut [u8], at: usize, value: u16) {
    data[at..at + 2].copy_from_slice(&value.to_be_bytes());
}
fn relative(data: &mut [u8], field: usize, target: usize) {
    word(data, field, (target as i64 - field as i64) as i32 as u32);
}

/// Synthetic v263 layout: one empty-topology skin, an entity directory whose
/// bodies are authored per test, and four textures with synthetic hashes.
struct Fixture {
    data: Vec<u8>,
}

impl Fixture {
    fn new(entities: u16, textures: u16) -> Self {
        let mut data = vec![0; LEN];
        for (at, value) in [
            (0, 0x4745_4f4d),
            (4, 0x010a_00c1),
            (8, 263),
            (0x14, LEN as u32),
            (0x18, LEN as u32),
            (0x60, 0x0001_0001),
        ] {
            word(&mut data, at, value);
        }
        relative(&mut data, 0x64, 0x100);
        word(&mut data, 0x100, 0x0d00_00c1);
        word(&mut data, 0x108, BODY as u32);
        word(&mut data, BODY, 0x200);
        half(&mut data, 0x50, entities);
        relative(&mut data, 0x54, ENTITIES);
        half(&mut data, 0xb8, textures);
        relative(&mut data, 0xbc, TEXTURES);
        for index in 0..usize::from(textures) {
            let at = TEXTURES + index * 0x1c;
            word(&mut data, at, 0x8600_0000 | index as u32);
            word(&mut data, at + 8, 0x5c0);
        }
        Self { data }
    }
    fn counts(&mut self, values: [u32; 3]) -> &mut Self {
        for (offset, value) in [0x68, 0x70, 0x78].into_iter().zip(values) {
            word(&mut self.data, BODY + offset, value);
        }
        relative(&mut self.data, BODY + 0x6c, PARTS);
        self
    }
    fn part(&mut self, index: usize, reference: u32) -> &mut Self {
        word(&mut self.data, PARTS + index * 0x14 + 0xc, reference);
        self
    }
    fn entity(&mut self, index: usize, object: usize) -> &mut Self {
        let at = ENTITIES + index * 0x14;
        word(&mut self.data, at, 0x8200_0000 | index as u32);
        word(&mut self.data, at + 8, object as u32);
        self
    }
    fn mesh(&mut self, object: usize, list: usize, indices: &[u16]) -> &mut Self {
        word(&mut self.data, object, 0x601);
        relative(&mut self.data, object + 0x54, list);
        half(&mut self.data, list, indices.len() as u16);
        for (lane, value) in indices.iter().enumerate() {
            half(&mut self.data, list + 2 + lane * 2, *value);
        }
        self
    }
    fn kind(&mut self, object: usize, kind: u32) -> &mut Self {
        word(&mut self.data, object, kind);
        self
    }
    fn split(&mut self, object: usize, children: &[usize]) -> &mut Self {
        word(&mut self.data, object, 0x603);
        word(&mut self.data, object + 0x54, children.len() as u32);
        for (index, child) in children.iter().enumerate() {
            relative(&mut self.data, object + 0x5c + index * 4, *child);
        }
        self
    }
    fn run(&self, budget: TextureBudget) -> Result<TextureDependencies> {
        let edb = Edb::parse(self.data.clone())?;
        edb.skin_texture_dependencies(0, budget)?
            .context("fixture skin")
    }
}

fn enumerated(result: TextureDependencies) -> EnumeratedTextures {
    match result {
        TextureDependencies::Enumerated(value) => value,
        other => panic!("expected enumeration, got {other:?}"),
    }
}

/// Part0: mesh [3,1,3]. Part1: split of mesh [0,1], an inert 0x607 and an
/// empty mesh. Part2: inert 0x607. Each source count contributes one part.
fn ordered() -> Fixture {
    let mut f = Fixture::new(3, 4);
    f.counts([1, 1, 1])
        .part(0, 0x8200_0002)
        .part(1, 0x0000_0000)
        .part(2, 0xff00_0001)
        .entity(0, 0x400)
        .entity(1, 0x340)
        .entity(2, 0x360)
        .kind(0x340, 0x607)
        .mesh(0x360, 0x500, &[3, 1, 3])
        .split(0x400, &[0x440, 0x380, 0x480])
        .mesh(0x440, 0x510, &[0, 1])
        .kind(0x380, 0x607)
        .mesh(0x480, 0x520, &[]);
    f
}

#[test]
fn first_encounter_order_spans_parts_split_children_and_lists() {
    let result = enumerated(ordered().run(TextureBudget::default()).unwrap());
    assert_eq!(result.hashes(), [0x8600_0003, 0x8600_0001, 0x8600_0000]);
    assert_eq!(
        result
            .dependencies
            .iter()
            .map(|value| (value.texture_index, value.first_part))
            .collect::<Vec<_>>(),
        [(3, 0), (1, 0), (0, 1)]
    );
    assert_eq!(result.part_counts_raw, [1, 1, 1]);
    assert_eq!(
        (
            result.groups,
            result.splits,
            result.empty_objects,
            result.indices_read,
            result.duplicate_indices
        ),
        (3, 1, 1, 5, 2)
    );
}

#[test]
fn missing_directories_are_distinct_outcomes_read_before_parts() {
    // Part list is never addressed, even though its pointer is absent.
    let f = Fixture::new(0, 4);
    assert_eq!(
        f.run(TextureBudget::default()).unwrap(),
        TextureDependencies::EntityDirectoryAbsent
    );
    let f = Fixture::new(3, 0);
    assert_eq!(
        f.run(TextureBudget::default()).unwrap(),
        TextureDependencies::TextureDirectoryAbsent
    );
}

#[test]
fn part_total_uses_wrapping_source_sum() {
    let mut f = Fixture::new(1, 1);
    f.counts([u32::MAX, 1, 0]);
    word(&mut f.data, BODY + 0x6c, 0);
    let result = enumerated(f.run(TextureBudget::default()).unwrap());
    assert_eq!(result.part_counts_raw, [u32::MAX, 1, 0]);
    assert!(result.dependencies.is_empty());
    assert_eq!(result.groups, 0);
}

#[test]
fn unreadable_native_inputs_are_errors_not_empty_lists() {
    let mut f = ordered();
    f.kind(0x340, 0x609);
    assert!(f.run(TextureBudget::default()).is_err(), "unsupported kind");

    let mut f = ordered();
    word(&mut f.data, 0x360 + 0x54, 0);
    assert!(f.run(TextureBudget::default()).is_err(), "absent list");

    let mut f = ordered();
    half(&mut f.data, 0x502, 4);
    assert!(f.run(TextureBudget::default()).is_err(), "texture index");

    let mut f = ordered();
    word(&mut f.data, 0x400 + 0x60, 0);
    assert!(f.run(TextureBudget::default()).is_err(), "split child");

    let mut f = ordered();
    f.part(1, 0x8200_0003);
    assert!(f.run(TextureBudget::default()).is_err(), "entity index");

    let mut f = ordered();
    word(&mut f.data, BODY + 0x6c, 0);
    assert!(f.run(TextureBudget::default()).is_err(), "part list");

    let mut f = ordered();
    f.entity(1, 0);
    assert!(f.run(TextureBudget::default()).is_err(), "absent object");
}

#[test]
fn budgets_bound_parts_objects_depth_and_indices() {
    let f = ordered();
    let base = TextureBudget::default();
    for budget in [
        TextureBudget { parts: 2, ..base },
        TextureBudget { objects: 4, ..base },
        TextureBudget { depth: 1, ..base },
        TextureBudget { indices: 4, ..base },
        TextureBudget { unique: 2, ..base },
    ] {
        assert!(f.run(budget).is_err(), "{budget:?}");
    }
    // A self-referencing split exhausts depth rather than looping.
    let mut f = ordered();
    f.split(0x400, &[0x400]);
    assert!(f.run(base).is_err());
}

#[test]
fn lookup_requires_the_retained_resource_identity() {
    let f = ordered();
    let edb = Edb::parse(f.data.clone()).unwrap();
    let skin = edb.skin(0, Budget { skins: 1, bones: 0 }).unwrap().unwrap();
    let mut other = f.data.clone();
    word(&mut other, 4, 0x010a_00c2);
    let other = Edb::parse(other).unwrap();
    assert!(
        skin.texture_dependencies(&other, TextureBudget::default())
            .is_err()
    );
}

#[test]
fn no_op_link_callback_and_unread_topology_do_not_require_geometry_or_bone_payloads() {
    let mut f = ordered();
    f.kind(0x340, 0x608);
    word(&mut f.data, BODY + 4, u32::MAX);
    word(&mut f.data, BODY + 0x48, 0);
    assert_eq!(
        enumerated(f.run(TextureBudget::default()).unwrap()).hashes(),
        [0x8600_0003, 0x8600_0001, 0x8600_0000]
    );
}

#[test]
fn early_directory_return_skips_even_an_unreadable_skin_body() {
    let mut f = Fixture::new(0, 4);
    word(&mut f.data, 0x108, u32::MAX);
    assert_eq!(
        f.run(TextureBudget::default()).unwrap(),
        TextureDependencies::EntityDirectoryAbsent
    );
    let mut f = Fixture::new(3, 0);
    word(&mut f.data, 0x108, u32::MAX);
    assert_eq!(
        f.run(TextureBudget::default()).unwrap(),
        TextureDependencies::TextureDirectoryAbsent
    );
}
