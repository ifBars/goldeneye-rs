use super::{
    data::Data,
    loader::{Profile, load_profile},
    *,
};
use crate::disc::DiscIdentity;
use anyhow::Result;
use sha2::{Digest, Sha256};
use std::io::{self, Cursor, Read, Seek, SeekFrom};
#[path = "tests/secondary.rs"]
mod secondary;

const DOL: usize = 0x800;
const DATA_OFFSET: usize = 0x100;
const DATA_BYTES: usize = 0x200;
const TEXT_OFFSET: usize = 0x300;
const BASE: u32 = 0x8000_1000;
const TEXT: u32 = 0x8000_2000;
const WEAPON_A: u32 = 0x0200_00a1;
const WEAPON_B: u32 = 0x0200_00a2;
const GENERIC_A: u32 = 0x0900_00a1;
const GENERIC_B: u32 = 0x0900_00a2;
const MODE_A: u32 = 0x0900_00b1;
const MODE_B: u32 = 0x0900_00b2;

fn word(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

struct Fixture {
    bytes: Vec<u8>,
    dol_hash: String,
    data_hash: String,
}

impl Fixture {
    fn new() -> Self {
        let mut bytes = vec![0; DOL + TEXT_OFFSET + 0x20];
        bytes[..6].copy_from_slice(b"TEST00");
        bytes[0x18..0x1c].copy_from_slice(&[0x5d, 0x1c, 0x9e, 0xa3]);
        word(&mut bytes, 0x420, (DOL / 4) as u32);
        word(&mut bytes, DOL, TEXT_OFFSET as u32);
        word(&mut bytes, DOL + 0x48, TEXT);
        word(&mut bytes, DOL + 0x90, 0x20);
        word(&mut bytes, DOL + 0x1c + 5 * 4, DATA_OFFSET as u32);
        word(&mut bytes, DOL + 0x64 + 5 * 4, BASE);
        word(&mut bytes, DOL + 0xac + 5 * 4, DATA_BYTES as u32);
        word(&mut bytes, DOL + 0xd8, BASE + DATA_BYTES as u32);
        word(&mut bytes, DOL + 0xdc, 0x100);
        let data = &mut bytes[DOL + DATA_OFFSET..DOL + DATA_OFFSET + DATA_BYTES];
        word(data, 0, WEAPON_A);
        word(data, 8, WEAPON_B);
        word(data, 0xc, BASE + 0x80);
        word(data, 0x14, TEXT); // Unselected TP pointer is deliberately unsupported.
        word(data, 0x18, BASE + DATA_BYTES as u32); // Unselected BSS pointer.
        word(data, 0x2c, WEAPON_B);
        word(data, 0x2c + 0xc, BASE + 0xc0);
        word(data, 0x80, GENERIC_A);
        word(data, 0x84, MODE_A);
        word(data, 0xc0, GENERIC_B);
        word(data, 0xc4, MODE_B);
        for (index, key) in [WEAPON_B, WEAPON_A, WEAPON_B, 0].into_iter().enumerate() {
            word(data, 0x100 + index * 4, key);
        }
        let mut result = Self {
            bytes,
            dol_hash: String::new(),
            data_hash: String::new(),
        };
        result.seal();
        result
    }

    fn identity(&self) -> DiscIdentity {
        DiscIdentity {
            game_id: "TEST00".into(),
            title: "Authored".into(),
            revision: 0,
            format: "Authored".into(),
            logical_bytes: self.bytes.len() as u64,
        }
    }

    fn profile(&self) -> Profile<'_> {
        Profile {
            game_id: b"TEST00",
            revision: 0,
            dol_bytes: (self.bytes.len() - DOL) as u64,
            dol_sha256: &self.dol_hash,
            data_index: 5,
            data_offset: DATA_OFFSET as u32,
            data_address: BASE,
            data_bytes: DATA_BYTES as u32,
            data_sha256: &self.data_hash,
            registry_root: BASE,
            order_root: BASE + 0x100,
        }
    }

    fn seal(&mut self) {
        self.dol_hash = format!("{:x}", Sha256::digest(&self.bytes[DOL..]));
        self.data_hash = format!(
            "{:x}",
            Sha256::digest(&self.bytes[DOL + DATA_OFFSET..DOL + DATA_OFFSET + DATA_BYTES])
        );
    }

    fn patch(&mut self, offset: usize, value: u32) {
        word(&mut self.bytes, DOL + DATA_OFFSET + offset, value);
        self.seal();
    }

    fn load(&self) -> Result<WeaponTables> {
        load_profile(
            &mut Cursor::new(&self.bytes),
            &self.identity(),
            load_budget(),
            &self.profile(),
        )
    }
}

fn load_budget() -> LoadBudget {
    LoadBudget {
        max_dol_bytes: 1024 * 1024,
        retained_data_bytes: 1024,
        registry_keys: 16,
    }
}

fn query_budget() -> QueryBudget {
    QueryBudget {
        map_rows: 32,
        remap_steps: 8,
    }
}

fn error<T>(result: Result<T>) -> String {
    match result {
        Ok(_) => panic!("expected authored failure"),
        Err(error) => format!("{error:#}"),
    }
}

struct Traced {
    cursor: Cursor<Vec<u8>>,
    maximum_read: usize,
    reads: usize,
}
impl Read for Traced {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.maximum_read = self.maximum_read.max(output.len());
        self.reads += 1;
        self.cursor.read(output)
    }
}
impl Seek for Traced {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        assert!(
            !matches!(position, SeekFrom::End(_)),
            "decoded Wii partition has no End seek"
        );
        self.cursor.seek(position)
    }
}

#[test]
fn verified_loader_uses_wii_boot_word_and_retains_only_typed_data() {
    let fixture = Fixture::new();
    let tables = fixture.load().unwrap();
    assert_eq!(tables.identity().main_dol_partition_offset, DOL as u64);
    assert_eq!(tables.identity().data_bytes, DATA_BYTES);
    assert_eq!(tables.identity().dol_sha256, fixture.dol_hash);
    assert_eq!(tables.identity().data_sha256, fixture.data_hash);
    assert_eq!(tables.registry_count(), 2);
    assert_eq!(tables.record(WEAPON_A).unwrap().source_index(), 0);
    assert_eq!(
        tables.record(WEAPON_B).unwrap().source_address(),
        BASE + 0x2c
    );
    assert_eq!(tables.record(WEAPON_A).unwrap().key(), WEAPON_A);
    assert!(tables.record(99).is_none());
}

#[test]
fn wrong_identity_digest_or_short_input_never_produces_a_table_owner() {
    let fixture = Fixture::new();
    let mut outer = fixture.identity();
    outer.revision = 1;
    let mut reader = Traced {
        cursor: Cursor::new(fixture.bytes.clone()),
        maximum_read: 0,
        reads: 0,
    };
    assert!(
        error(load_profile(
            &mut reader,
            &outer,
            load_budget(),
            &fixture.profile()
        ))
        .contains("identity")
    );
    assert_eq!(reader.reads, 0);
    let mut inner = fixture.bytes.clone();
    inner[7] = 1;
    assert!(
        error(load_profile(
            &mut Cursor::new(inner),
            &fixture.identity(),
            load_budget(),
            &fixture.profile()
        ))
        .contains("partition identity")
    );
    let mut changed = fixture.bytes.clone();
    changed[DOL + TEXT_OFFSET] = 9;
    assert!(
        error(load_profile(
            &mut Cursor::new(changed),
            &fixture.identity(),
            load_budget(),
            &fixture.profile()
        ))
        .contains("DOL SHA256")
    );
    let mut profile = fixture.profile();
    profile.data_sha256 = "wrong";
    assert!(
        error(load_profile(
            &mut Cursor::new(&fixture.bytes),
            &fixture.identity(),
            load_budget(),
            &profile
        ))
        .contains("DATA SHA256")
    );
    for length in [0x420, DOL + 0x80, fixture.bytes.len() - 1] {
        assert!(
            load_profile(
                &mut Cursor::new(&fixture.bytes[..length]),
                &fixture.identity(),
                load_budget(),
                &fixture.profile()
            )
            .is_err()
        );
    }
}

#[test]
fn malformed_headers_and_load_limits_fail_before_large_reads_or_allocation() {
    let fixture = Fixture::new();
    for (offset, value) in [
        (0, u32::MAX - 3),       // TEXT file extent wraps.
        (0x48, u32::MAX - 3),    // TEXT virtual extent wraps.
        (0, 0x80),               // Section overlaps header.
        (0, DATA_OFFSET as u32), // File overlap with DATA.
        (0x48, BASE),            // Virtual overlap with DATA.
        (0xac + 5 * 4, 0),       // Supported DATA absent.
    ] {
        let mut bytes = fixture.bytes.clone();
        word(&mut bytes, DOL + offset, value);
        let mut reader = Traced {
            cursor: Cursor::new(bytes),
            maximum_read: 0,
            reads: 0,
        };
        assert!(
            load_profile(
                &mut reader,
                &fixture.identity(),
                load_budget(),
                &fixture.profile()
            )
            .is_err()
        );
        assert_eq!(
            reader.reads, 2,
            "only fixed boot/header reads precede header rejection"
        );
    }
    for limit in [
        LoadBudget {
            max_dol_bytes: 0x100,
            ..load_budget()
        },
        LoadBudget {
            retained_data_bytes: DATA_BYTES - 1,
            ..load_budget()
        },
    ] {
        assert!(
            load_profile(
                &mut Cursor::new(&fixture.bytes),
                &fixture.identity(),
                limit,
                &fixture.profile()
            )
            .is_err()
        );
    }
    // An unused section's offset/address is not a section or an allocation size.
    let mut unused = fixture;
    word(&mut unused.bytes, DOL + 4, u32::MAX);
    unused.seal();
    assert!(unused.load().is_ok());
}

#[test]
fn full_dol_identity_hash_streams_in_bounded_chunks_without_end_seek() {
    let mut fixture = Fixture::new();
    let text_bytes = 3 * 64 * 1024 + 13;
    fixture.bytes.resize(DOL + TEXT_OFFSET + text_bytes, 0xa5);
    word(&mut fixture.bytes, DOL + 0x90, text_bytes as u32);
    // Keep synthetic TEXT away from DATA while its source bytes are hashed.
    fixture.seal();
    let mut reader = Traced {
        cursor: Cursor::new(fixture.bytes.clone()),
        maximum_read: 0,
        reads: 0,
    };
    let tables = load_profile(
        &mut reader,
        &fixture.identity(),
        load_budget(),
        &fixture.profile(),
    )
    .unwrap();
    assert!(reader.maximum_read <= 64 * 1024);
    assert!(reader.reads >= 7);
    assert_eq!(tables.identity().data_bytes, DATA_BYTES);
    assert!(tables.identity().dol_bytes > tables.identity().data_bytes as u64);
}

#[test]
fn retained_data_word_mapping_rejects_text_bss_gaps_unaligned_and_end_crossings() {
    let data = Data {
        address: BASE,
        bytes: vec![0; 16].into_boxed_slice(),
    };
    assert_eq!(data.u32(BASE).unwrap(), 0);
    assert_eq!(data.u32(BASE + 12).unwrap(), 0);
    for address in [
        BASE - 4,
        BASE + 1,
        BASE + 16,
        TEXT,
        BASE + DATA_BYTES as u32,
        u32::MAX,
    ] {
        assert!(data.u32(address).is_err());
    }
}

#[test]
fn registry_and_order_sentinels_have_explicit_budgets_and_preserve_source_order() {
    let fixture = Fixture::new();
    assert!(
        error(load_profile(
            &mut Cursor::new(&fixture.bytes),
            &fixture.identity(),
            LoadBudget {
                registry_keys: 2,
                ..load_budget()
            },
            &fixture.profile()
        ))
        .contains("registry key budget")
    );
    let tables = fixture.load().unwrap();
    let order = tables.weapon_order(4).unwrap();
    assert_eq!(order.len(), 3);
    assert!(!order.is_empty());
    assert_eq!(order.key_at(0), Some(WEAPON_B));
    assert_eq!(order.key_at(1), Some(WEAPON_A));
    assert_eq!(order.key_at(2), Some(WEAPON_B));
    assert_eq!(order.key_at(3), None);
    assert_eq!(order.first_index(WEAPON_B), Some(0));
    assert_eq!(order.first_index(99), None);
    assert!(error(tables.weapon_order(3)).contains("order word budget"));
    let mut duplicate = fixture;
    duplicate.patch(0x2c, WEAPON_A);
    assert!(error(duplicate.load()).contains("unsupported duplicate"));
}

#[test]
fn map_reads_only_first_matching_variant_and_reached_pointer_fields() {
    let mut fixture = Fixture::new();
    // A later duplicate must not replace or invalidate the first match.
    fixture.patch(0x94, GENERIC_A);
    fixture.patch(0x98, 0x0900_00ff);
    let tables = fixture.load().unwrap();
    let record = tables.record(WEAPON_A).unwrap();
    assert_eq!(
        tables
            .map_lookup(record.normal_map().unwrap(), GENERIC_A, 0, 1)
            .unwrap(),
        MapLookup {
            matched_source_ordinal: Some(0),
            mapped_raw: MODE_A
        }
    );
    assert_eq!(
        tables
            .map_lookup(record.normal_map().unwrap(), GENERIC_A, 1, 1)
            .unwrap()
            .mapped_raw,
        0
    );
    assert!(
        tables
            .map_lookup(record.tp_map().unwrap(), GENERIC_A, 0, 1)
            .is_err()
    );
    assert!(
        tables
            .map_lookup(record.secondary_map().unwrap(), GENERIC_A, 0, 1)
            .is_err()
    );
    assert_eq!(
        tables.map_lookup(None, GENERIC_A, 0, 0).unwrap().mapped_raw,
        0
    );
    // A sentinel at the final word requires no full row. For a matching key,
    // invalid variants likewise do not read absent output fields beyond DATA.
    fixture.patch(0xc, BASE + DATA_BYTES as u32 - 4);
    let tables = fixture.load().unwrap();
    assert_eq!(
        tables
            .map_lookup(
                tables.record(WEAPON_A).unwrap().normal_map().unwrap(),
                GENERIC_A,
                0,
                1
            )
            .unwrap()
            .matched_source_ordinal,
        None
    );
    fixture.patch(DATA_BYTES - 4, GENERIC_A);
    let tables = fixture.load().unwrap();
    let map = tables.record(WEAPON_A).unwrap().normal_map().unwrap();
    assert_eq!(
        tables.map_lookup(map, GENERIC_A, 4, 1).unwrap().mapped_raw,
        0
    );
    assert!(tables.map_lookup(map, GENERIC_A, 0, 1).is_err());
}

#[test]
fn normal_fp_remap_preserves_availability_fallback_and_unmapped_generic() {
    let tables = Fixture::new().load().unwrap();
    let mut candidates = Vec::new();
    let mapped = tables
        .map_fp_normal(WEAPON_A, GENERIC_A, 0, query_budget(), |candidate| {
            candidates.push(candidate);
            Ok(true)
        })
        .unwrap();
    assert_eq!(mapped.output, MODE_A);
    assert!(mapped.mapped);
    assert_eq!(mapped.steps.len(), 1);
    assert_eq!(candidates, [MODE_A]);
    let fallback = tables
        .map_fp_normal(WEAPON_A, GENERIC_B, 0, query_budget(), |candidate| {
            Ok(candidate == MODE_B)
        })
        .unwrap();
    assert_eq!(fallback.output, MODE_B);
    assert!(fallback.mapped);
    assert_eq!(fallback.steps.len(), 2);
    assert_eq!(fallback.steps[0].weapon, WEAPON_A);
    assert_eq!(fallback.steps[1].weapon, WEAPON_B);
    let missed = tables
        .map_fp_normal(WEAPON_A, GENERIC_A, 0, query_budget(), |_| Ok(false))
        .unwrap();
    assert_eq!(missed.output, GENERIC_A);
    assert!(!missed.mapped);
    let mut calls = 0;
    let missing = tables
        .map_fp_normal(99, GENERIC_B, 0, query_budget(), |_| {
            calls += 1;
            Ok(true)
        })
        .unwrap();
    assert_eq!(missing.output, GENERIC_B);
    assert!(!missing.mapped);
    assert_eq!(calls, 0);
}

#[test]
fn reached_cycles_callback_errors_and_aggregate_budgets_never_become_native_misses() {
    let mut fixture = Fixture::new();
    fixture.patch(0x2c + 8, WEAPON_A);
    let tables = fixture.load().unwrap();
    assert!(
        error(tables.map_fp_normal(WEAPON_A, 99, 0, query_budget(), |_| Ok(false)))
            .contains("cycle")
    );
    assert!(
        tables
            .map_fp_normal(WEAPON_A, GENERIC_A, 0, query_budget(), |_| Ok(true))
            .unwrap()
            .mapped,
        "unreached parent cycles do not invalidate an available first mapping"
    );
    assert!(
        error(
            tables.map_fp_normal(WEAPON_A, GENERIC_A, 0, query_budget(), |_| anyhow::bail!(
                "availability failed"
            ))
        )
        .contains("availability failed")
    );
    for limits in [
        QueryBudget {
            map_rows: 2,
            remap_steps: 8,
        },
        QueryBudget {
            map_rows: 32,
            remap_steps: 1,
        },
    ] {
        assert!(
            error(tables.map_fp_normal(WEAPON_A, GENERIC_B, 0, limits, |_| Ok(true)))
                .contains("budget")
        );
    }
    fixture.patch(0xc, TEXT);
    let invalid = fixture.load().unwrap();
    assert!(
        error(invalid.map_fp_normal(WEAPON_A, GENERIC_A, 0, query_budget(), |_| Ok(true)))
            .contains("outside retained DATA")
    );
}
