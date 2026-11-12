use super::*;
pub(super) fn metadata(
    data: &[u8],
    table: &Table,
    reference: u32,
) -> Result<Option<AnimationScriptReference>> {
    let Some(directory_index) = crate::reference::resolve(data, table, 16, reference)? else {
        return Ok(None);
    };
    let at = table
        .offset
        .checked_add(
            directory_index
                .checked_mul(16)
                .context("script descriptor offset overflow")?,
        )
        .context("script descriptor offset overflow")?;
    let b = Bytes(data);
    b.range(at, 16)?;
    Ok(Some(AnimationScriptReference {
        directory_index,
        hash: b.u32(at)?,
        section: b.u16(at + 4)?,
        debug: b.u16(at + 6)?,
        address: b.u32(at + 8)?,
        runtime_pointer_raw: b.u32(at + 12)?,
    }))
}

#[cfg(test)]
#[path = "tests/script_reference.rs"]
mod tests;
