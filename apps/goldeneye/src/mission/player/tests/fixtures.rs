//! Authored tiny registry/component/skin owners; no original data is embedded.
use super::*;
pub(in crate::mission) const CHARACTER: u32 = 0x4d00_00a5;
pub(super) const SKIN_FILE: u32 = 0x010a_00a5;

fn word(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn half(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_be_bytes());
}
fn relative(bytes: &mut [u8], at: usize, target: usize) {
    word(bytes, at, (target as i32 - at as i32) as u32);
}
fn edb(mut bytes: Vec<u8>, hash: u32) -> Edb {
    let length = bytes.len() as u32;
    for (at, value) in [
        (0, 0x4745_4f4d),
        (4, hash),
        (8, 263),
        (0x14, length),
        (0x18, length),
    ] {
        word(&mut bytes, at, value);
    }
    Edb::parse(bytes).unwrap()
}
pub(super) fn resources(camera_bone: Option<u16>) -> Vec<Edb> {
    let mut registry = vec![0; 0x300];
    half(&mut registry, 0x98, 1);
    half(&mut registry, 0x9a, 1);
    relative(&mut registry, 0x9c, 0xd0);
    word(&mut registry, 0xd0, character::CHARACTER_SHEET);
    word(&mut registry, 0xd8, 0x100);
    relative(&mut registry, 0x108, 0x120);
    word(&mut registry, 0x120, 1);
    word(&mut registry, 0x124, CHARACTER);
    word(&mut registry, 0x128, SKIN_FILE);
    let component = edb(vec![0; 0x100], COMPONENT);
    let mut skin = vec![0; 0x400];
    half(&mut skin, 0x60, 1);
    half(&mut skin, 0x62, 1);
    relative(&mut skin, 0x64, 0xd0);
    word(&mut skin, 0xd0, SKIN);
    word(&mut skin, 0xd8, 0x100);
    word(&mut skin, 0x100, 0x200);
    word(&mut skin, 0x104, 3);
    relative(&mut skin, 0x148, 0x200);
    relative(&mut skin, 0x15c, 0x240);
    for (index, parent) in [u16::MAX, 0, 1].into_iter().enumerate() {
        half(&mut skin, 0x200 + index * 8, parent);
        half(&mut skin, 0x202 + index * 8, 2);
    }
    if let Some(bone) = camera_bone {
        half(&mut skin, 0x240, bone);
        half(&mut skin, 0x242, 0x28);
        half(&mut skin, 0x246, u16::MAX);
    } else {
        // The sentinel's bone word is deliberately invalid and must stay unread.
        half(&mut skin, 0x240, u16::MAX);
        half(&mut skin, 0x242, u16::MAX);
    }
    vec![
        edb(registry, character::CATALOG_RESOURCE),
        component,
        edb(skin, SKIN_FILE),
    ]
}
pub(super) fn bind_resources(requested: u32, resources: Vec<Edb>) -> Result<PlayerComponents> {
    let mut owners = resources.into_iter().map(Some).collect::<Vec<_>>();
    PlayerComponents::from_loader(requested, |hash, _| {
        let owner = owners
            .iter_mut()
            .find(|owner| owner.as_ref().is_some_and(|e| e.header.hash == hash))
            .context("authored component owner absent or requested twice")?;
        owner
            .take()
            .context("authored component owner already moved")
    })
}
pub(in crate::mission) fn binding() -> Result<PlayerComponents> {
    bind_resources(CHARACTER, resources(Some(2)))
}
