use super::{Identity, LoadBudget, SuppliedData, data::Data};
use crate::{binary::Bytes, disc::DiscIdentity};
use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};
use std::io::{Read, Seek, SeekFrom};

const HEADER_BYTES: usize = 0x100;
const HASH_CHUNK: usize = 64 * 1024;

pub(crate) struct Profile<'a> {
    pub game_id: &'a [u8; 6],
    pub revision: u8,
    pub dol_bytes: u64,
    pub dol_sha256: &'a str,
    pub data_index: usize,
    pub data_offset: u32,
    pub data_address: u32,
    pub data_bytes: u32,
    pub data_sha256: &'a str,
}

const SUPPORTED: Profile<'static> = Profile {
    game_id: b"SJBE52",
    revision: 0,
    dol_bytes: 6_087_648,
    dol_sha256: "0d101e5eba1aca38847c03d7a353f5156f3ec2d9025d6c6773d105561532208b",
    data_index: 5,
    data_offset: 5_667_968,
    data_address: 0x8056_bba0,
    data_bytes: 383_136,
    data_sha256: "76d3456f33f013817ea80ddec93635c466d25bba233929800399b4d3e703836d",
};

#[derive(Debug, Clone, Copy)]
struct Section {
    offset: u32,
    address: u32,
    bytes: u32,
}

impl Section {
    fn file_end(self) -> Result<u32> {
        self.offset
            .checked_add(self.bytes)
            .context("DOL file range overflow")
    }

    fn address_end(self) -> Result<u32> {
        self.address
            .checked_add(self.bytes)
            .context("DOL virtual range overflow")
    }
}

pub(crate) fn load<R: Read + Seek + ?Sized>(
    partition: &mut R,
    identity: &DiscIdentity,
    budget: LoadBudget,
) -> Result<SuppliedData> {
    load_profile(partition, identity, budget, &SUPPORTED)
}

pub(crate) fn load_profile<R: Read + Seek + ?Sized>(
    partition: &mut R,
    identity: &DiscIdentity,
    budget: LoadBudget,
    profile: &Profile<'_>,
) -> Result<SuppliedData> {
    ensure!(
        identity.game_id.as_bytes() == profile.game_id && identity.revision == profile.revision,
        "unsupported supplied-data disc identity"
    );
    // PartitionBase is already the owned, decoded Wii data partition. Its End
    // seek is unsupported; all following ranges use budgets and read_exact.
    partition.seek(SeekFrom::Start(0))?;
    let mut boot = [0u8; 0x440];
    partition
        .read_exact(&mut boot)
        .context("reading supplied partition boot header")?;
    ensure!(
        &boot[..6] == profile.game_id
            && boot[7] == profile.revision
            && boot[0x18..0x1c] == [0x5d, 0x1c, 0x9e, 0xa3],
        "supplied partition identity is not the supported Wii revision"
    );
    let dol_offset = u64::from(Bytes(&boot).u32(0x420)?)
        .checked_mul(4)
        .context("Wii DOL offset overflow")?;
    ensure!(
        dol_offset >= 0x440,
        "DOL overlaps the partition boot header"
    );
    partition.seek(SeekFrom::Start(dol_offset))?;
    let mut raw_header = [0u8; HEADER_BYTES];
    partition
        .read_exact(&mut raw_header)
        .context("reading supplied DOL header")?;
    let (data, extent) = header(&raw_header, profile, budget)?;
    let dol_end = dol_offset
        .checked_add(extent)
        .context("partition DOL range overflow")?;
    ensure!(dol_end > dol_offset, "empty supplied DOL extent");

    partition.seek(SeekFrom::Start(dol_offset))?;
    let mut digest = Sha256::new();
    let mut scratch = [0u8; HASH_CHUNK];
    let mut remaining = extent;
    while remaining != 0 {
        let amount = usize::try_from(remaining.min(HASH_CHUNK as u64))?;
        partition
            .read_exact(&mut scratch[..amount])
            .context("hashing supplied DOL")?;
        digest.update(&scratch[..amount]);
        remaining -= amount as u64;
    }
    let dol_sha256 = format!("{:x}", digest.finalize());
    ensure!(
        dol_sha256 == profile.dol_sha256,
        "unsupported supplied DOL SHA256"
    );

    let data_offset = dol_offset
        .checked_add(u64::from(data.offset))
        .context("partition DATA offset overflow")?;
    partition.seek(SeekFrom::Start(data_offset))?;
    let mut bytes = vec![0; usize::try_from(data.bytes)?];
    partition
        .read_exact(&mut bytes)
        .context("retaining supplied DOL DATA")?;
    let data_sha256 = format!("{:x}", Sha256::digest(&bytes));
    ensure!(
        data_sha256 == profile.data_sha256,
        "supplied DATA SHA256 differs"
    );
    let metadata = Identity {
        game_id: identity.game_id.clone(),
        revision: identity.revision,
        main_dol_partition_offset: dol_offset,
        dol_bytes: extent,
        dol_sha256,
        data_section_index: profile.data_index,
        data_virtual_address: data.address,
        data_bytes: bytes.len(),
        data_sha256,
    };
    Ok(SuppliedData::initialize(
        Data {
            address: data.address,
            bytes: bytes.into_boxed_slice(),
        },
        metadata,
    ))
}

fn header(
    raw: &[u8; HEADER_BYTES],
    profile: &Profile<'_>,
    budget: LoadBudget,
) -> Result<(Section, u64)> {
    let reader = Bytes(raw);
    let mut sections = Vec::with_capacity(18);
    let mut selected = None;
    for (data, count, offsets, addresses, sizes) in
        [(false, 7, 0, 0x48, 0x90), (true, 11, 0x1c, 0x64, 0xac)]
    {
        for index in 0..count {
            let bytes = reader.u32(sizes + index * 4)?;
            if bytes == 0 {
                continue;
            }
            let section = Section {
                offset: reader.u32(offsets + index * 4)?,
                address: reader.u32(addresses + index * 4)?,
                bytes,
            };
            ensure!(
                section.offset >= HEADER_BYTES as u32,
                "DOL section overlaps header"
            );
            let file_end = section.file_end()?;
            let address_end = section.address_end()?;
            ensure!(
                u64::from(file_end) <= budget.max_dol_bytes,
                "DOL extent budget exhausted"
            );
            for other in &sections {
                let other: &Section = other;
                ensure!(
                    file_end <= other.offset || other.file_end()? <= section.offset,
                    "overlapping DOL file sections"
                );
                ensure!(
                    address_end <= other.address || other.address_end()? <= section.address,
                    "overlapping DOL virtual sections"
                );
            }
            if data && index == profile.data_index {
                selected = Some(section);
            }
            sections.push(section);
        }
    }
    let selected = selected.context("supported DOL DATA section is absent")?;
    ensure!(
        selected.offset == profile.data_offset
            && selected.address == profile.data_address
            && selected.bytes == profile.data_bytes,
        "unsupported DOL DATA layout"
    );
    ensure!(
        usize::try_from(selected.bytes)? <= budget.retained_data_bytes,
        "retained DATA budget exhausted"
    );
    let extent = sections
        .iter()
        .try_fold(HEADER_BYTES as u64, |end, section| -> Result<u64> {
            Ok(end.max(u64::from(section.file_end()?)))
        })?;
    ensure!(
        extent == profile.dol_bytes && extent <= budget.max_dol_bytes,
        "unsupported DOL extent or exhausted budget"
    );
    Ok((selected, extent))
}
