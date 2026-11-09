//! Mission resources use the main Filelist archive. Check the small indexed
//! header against the caller's byte limit before asking GameAssets to allocate
//! the complete EDB. GameAssets still owns payload/index overlap validation.
use anyhow::{Context, Result, ensure};
use ge_assets::{
    edb::{Edb, Header},
    resource::GameAssets,
};

pub(crate) fn load(assets: &mut GameAssets, hash: u32, maximum: u32) -> Result<Edb> {
    let (archive, offset, version, base_bytes) = {
        let mut entries = assets
            .index
            .entries
            .iter()
            .filter(|entry| entry.hash == hash);
        let entry = entries
            .next()
            .with_context(|| format!("mission resource {hash:#010x} is absent"))?;
        ensure!(
            entries.next().is_none(),
            "ambiguous mission resource {hash:#010x}"
        );
        ensure!(
            entry.path.to_ascii_lowercase().ends_with(".edb"),
            "mission resource is not an EDB"
        );
        let location = entry
            .locations
            .first()
            .context("mission resource has no location")?;
        (
            format!("Filelist.{:03}", location.archive),
            u64::from(location.offset),
            entry.version,
            entry.base_bytes,
        )
    };
    let prefix = assets.disc.read(&archive, offset, Edb::HEADER_BYTES)?;
    let header = checked_header(&prefix, hash, version, base_bytes, maximum)?;
    let edb = assets.load_edb(&format!("0x{hash:08x}"))?;
    ensure!(
        edb.header.hash == header.hash
            && edb.header.version == header.version
            && edb.header.base_bytes == header.base_bytes
            && edb.header.payload_bytes == header.payload_bytes
            && edb.header.payload_bytes <= maximum,
        "mission resource changed after bounded header inspection"
    );
    Ok(edb)
}

fn checked_header(
    prefix: &[u8],
    hash: u32,
    version: u32,
    base_bytes: u32,
    maximum: u32,
) -> Result<Header> {
    let header = Edb::read_header(prefix)?;
    ensure!(
        header.hash == hash && header.version == version && header.base_bytes == base_bytes,
        "mission resource header differs from index"
    );
    ensure!(
        header.payload_bytes <= maximum,
        "mission resource exceeds host byte budget"
    );
    Ok(header)
}

#[cfg(test)]
#[path = "tests/resource.rs"]
mod tests;
