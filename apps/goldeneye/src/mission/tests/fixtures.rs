//! Authored tiny EDBs for mission ownership tests; no game data is embedded.
use ge_assets::edb::Edb;

fn word(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_be_bytes());
}

fn base(file: u32, map_hash: u32) -> Vec<u8> {
    let mut bytes = vec![0; 0x480];
    for (at, value) in [
        (0, 0x4745_4f4d),
        (4, file),
        (8, 263),
        (0x14, 0x480),
        (0x18, 0x480),
        (0x50, 0x0001_0000),
        (0x54, 0x120 - 0x54),
        (0x68, 0x0001_0000),
        (0x6c, 0x140 - 0x6c),
        (0x70, 0x0001_0000),
        (0x74, 0x100 - 0x74),
        (0x100, map_hash),
        (0x108, 0x180),
        (0x120, 0x0200_00a5),
        (0x128, 0x2e0),
        (0x140, 0x0400_00a5),
        (0x148, 0x2f0),
        (0x180, 0x500),
        (0x1c8, 1),
        (0x1cc, 0x320 - 0x1cc),
        (0x1d8, 0x300 - 0x1d8),
        (0x204, 1),
        (0x2e0, 0x607),
        (0x2f0, 0x300),
        (0x350, 0x8200_0000),
    ] {
        word(&mut bytes, at, value);
    }
    for offset in [0x340, 0x344, 0x348] {
        word(&mut bytes, offset, 1f32.to_bits());
    }
    bytes
}

pub(super) fn resource(file: u32, map_hash: u32, link: Option<(u32, u32, u32)>) -> Edb {
    let mut bytes = base(file, map_hash);
    if let Some((file, map, flags)) = link {
        for (at, value) in [
            (0x300, 1),
            (0x304, 0x380 - 0x304),
            (0x30c, 0x3e0 - 0x30c),
            (0x380, 0x3a0 - 0x380),
            (0x3a8, flags),
            (0x3d0, file),
            (0x3d4, map),
            (0x3e0, 62),
        ] {
            word(&mut bytes, at, value);
        }
    }
    Edb::parse(bytes).unwrap()
}

#[derive(Clone, Copy)]
pub(super) struct Point {
    pub id: Option<u32>,
    pub flags: u32,
    pub position: [u32; 3],
    pub rotation: [u32; 3],
}

pub(super) fn startpoint_resource(file: u32, map_hash: u32, points: &[Point]) -> Edb {
    assert!(points.len() <= 16);
    let mut bytes = base(file, map_hash);
    bytes.resize(0x1000, 0);
    word(&mut bytes, 0x14, 0x1000);
    word(&mut bytes, 0x18, 0x1000);
    word(&mut bytes, 0x300, points.len() as u32);
    word(&mut bytes, 0x304, 0x400 - 0x304);
    word(&mut bytes, 0x30c, 0x480 - 0x30c);
    word(&mut bytes, 0x480, 28);
    for (index, point) in points.iter().enumerate() {
        let field = 0x400 + index * 8;
        let record = 0x500 + index * 0x40;
        word(&mut bytes, field, (record - field) as u32);
        word(&mut bytes, record + 4, point.flags);
        word(&mut bytes, record + 8, u32::from(point.id.is_some()));
        if let Some(id) = point.id {
            word(&mut bytes, record + 0x30, id);
        }
        for (axis, &bits) in point.position.iter().enumerate() {
            word(&mut bytes, record + 0xc + axis * 4, bits);
        }
        for (axis, &bits) in point.rotation.iter().enumerate() {
            word(&mut bytes, record + 0x18 + axis * 4, bits);
        }
    }
    Edb::parse(bytes).unwrap()
}
