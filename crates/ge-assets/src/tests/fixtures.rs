pub fn edb_fixture() -> Vec<u8> {
    let mut d = vec![0; 0x1a0];
    for (o, v) in [
        (0, 0x47454f4du32),
        (4, 0x1000001),
        (8, 263),
        (20, 0x1a0),
        (24, 0x140),
        (0xb8, 0x10000),
        (0xbc, 0x14),
        (0xd0, 0x86000001),
        (0xd8, 0x100),
        (0xe0, 0x10001),
        (0x100, 0x10001),
        (0x104, 0x10000),
        (0x11c, u32::MAX),
        (0x128, 96),
        (0x12c, 0x14),
    ] {
        d[o..o + 4].copy_from_slice(&v.to_be_bytes());
    }
    d[0x10c] = 1;
    d[0x10d] = 1;
    d[0x113] = 1;
    for (o, v) in [
        (0x140, 0x0020af30u32),
        (0x144, 1),
        (0x148, 12),
        (0x14c, 20),
        (0x154, 0x10001),
        (0x158, 1),
        (0x15c, 64),
    ] {
        d[o..o + 4].copy_from_slice(&v.to_be_bytes());
    }
    d[0x180..].fill(128);
    d
}

fn put_graph(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn relative_graph(data: &mut [u8], at: usize, target: usize) {
    put_graph(data, at, (target as i32 - at as i32) as u32);
}
pub fn graph_fixture() -> Vec<u8> {
    let mut b = vec![0; 0x268];
    for (p, v) in [
        (0, 0x47454f4d),
        (4, 0x01000001),
        (8, 263),
        (20, 0x268),
        (24, 0x268),
        (0x50, 0x10000),
        (0xd0, 0x82000010),
        (0xd4, 0x10002),
        (0xd8, 0x100),
        (0xdc, 0x1234),
        (0xe0, 0xabcd),
        (0x100, 0x607),
        (0x120, 2f32.to_bits()),
        (0x128, 2f32.to_bits()),
        (0x16c, 4),
        (0x170, 2),
        (0x174, 1),
        (0x1b0, 0x0badf00d),
        (0x1f0, 0),
        (0x1f4, 1),
        (0x1f8, 2),
        (0x1fc, 0x123),
        (0x200, 2),
        (0x204, 1),
        (0x208, 3),
        (0x20c, 0x456),
        (0x210, 0x000f_ffff),
        (0x214, 0x000f_ffff),
        (0x218, 1),
        (0x21c, 0),
        (0x220, 0x000f_ffff),
        (0x224, 0x000f_ffff),
        (0x260, 2),
    ] {
        put_graph(&mut b, p, v);
    }
    for (p, target) in [
        (0x54, 0xd0),
        (0x178, 0x1c0),
        (0x17c, 0x1f0),
        (0x180, 0x210),
        (0x184, 0x260),
        (0x188, 0x22c),
        (0x190, 0x244),
    ] {
        relative_graph(&mut b, p, target);
    }
    for (i, v) in [[0f32, 0., 0.], [2., 0., 0.], [0., 0., 2.], [2., 0., 2.]]
        .into_iter()
        .enumerate()
    {
        for (j, c) in v.into_iter().enumerate() {
            put_graph(&mut b, 0x1c0 + i * 12 + j * 4, c.to_bits());
        }
    }
    let distance = (8f32 / 9.).sqrt();
    for (i, v) in [
        [2f32 / 3., 0., 2. / 3.],
        [4. / 3., 0., 4. / 3.],
        [1e8, 1e8, distance],
        [distance, 1e8, 1e8],
    ]
    .into_iter()
    .enumerate()
    {
        for (j, c) in v.into_iter().enumerate() {
            put_graph(&mut b, 0x22c + i * 12 + j * 4, c.to_bits());
        }
    }
    b
}
