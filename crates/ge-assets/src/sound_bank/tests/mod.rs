use super::*;

const BANK: u32 = 0x2d50_1234;
const SOUND: usize = 0x860;
const WAVE: usize = 0x8a0;
const PAIR: usize = 0x900;
const PARAMETERS: usize = 0x910;
const AUDIO: usize = 0x940;

fn be(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn relative(bytes: &mut [u8], field: usize, target: usize) {
    be(bytes, field, (target as i64 - field as i64) as i32 as u32);
}
fn fixture() -> Vec<u8> {
    let mut bytes = vec![0; 0x960];
    bytes[..4].copy_from_slice(b"MUSX");
    bytes[4..8].copy_from_slice(&BANK.to_le_bytes());
    bytes[8..12].copy_from_slice(&10u32.to_le_bytes());
    let length = bytes.len() as u32;
    bytes[12..16].copy_from_slice(&length.to_le_bytes());
    bytes[16..20].copy_from_slice(b"WII_");
    bytes[0x800..0x804].copy_from_slice(b"SBNK");
    be(&mut bytes, 0x804, 0x1e);
    be(&mut bytes, 0x80c, BANK);
    be(&mut bytes, 0x814, 2);
    relative(&mut bytes, 0x818, SOUND);
    be(&mut bytes, SOUND, 0xf000_0001);
    be(&mut bytes, SOUND + 16, 0x1000_0002); // deliberately unsorted unsigned
    bytes[SOUND + 12] = 1;
    bytes[SOUND + 28] = 21; // retained composite program
    relative(&mut bytes, SOUND + 8, PAIR);
    relative(&mut bytes, PAIR, PARAMETERS);
    relative(&mut bytes, PAIR + 4, WAVE + 40); // storage ordinal, not hash sort
    be(&mut bytes, 0x834, 2);
    relative(&mut bytes, 0x838, WAVE);
    be(&mut bytes, WAVE, 0xff00_0001);
    be(&mut bytes, WAVE + 40, 0x0100_0001);
    be(&mut bytes, WAVE + 4, 123);
    bytes[WAVE + 0x18..WAVE + 0x1a].copy_from_slice(&22050u16.to_be_bytes());
    bytes[WAVE + 0x1a] = 0xee; // residency is independent of decoder support
    bytes[WAVE + 0x1b] = 9;
    be(&mut bytes, WAVE + 0x1c, 0);
    be(&mut bytes, WAVE + 0x20, 4);
    be(&mut bytes, WAVE + 40 + 0x1c, 4);
    be(&mut bytes, WAVE + 40 + 0x20, 4);
    be(&mut bytes, 0x84c, 8);
    be(&mut bytes, 0x850, AUDIO as u32);
    bytes[AUDIO..AUDIO + 8].copy_from_slice(&[23, 19, 11, 7, 101, 103, 107, 109]);
    bytes
}

mod bounds;
mod lookup;
