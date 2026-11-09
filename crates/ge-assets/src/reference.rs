//! Native directory-reference lookup. CLI hash selectors retain their separate contract.
use crate::{binary::Bytes, edb::Table};
use anyhow::{Context, Result, ensure};

pub(crate) fn resolve(
    data: &[u8],
    table: &Table,
    stride: usize,
    reference: u32,
) -> Result<Option<usize>> {
    // The original count load is signed. Top-byte zero OR bit 31 selects low-24 index mode.
    let count = table.count as i16;
    if reference >> 24 == 0 || reference & 0x8000_0000 != 0 {
        let index = (reference & 0x00ff_ffff) as usize;
        return Ok((count > 0 && index < count as usize).then_some(index));
    }
    let size = table.hash_size_raw;
    if size == 0 {
        return Ok(None);
    }
    let n = usize::from(size.unsigned_abs());
    ensure!(
        count > 0 && n <= count as usize,
        "invalid native directory hash count"
    );
    let (start, step) = if size < 0 {
        (
            table
                .offset
                .checked_sub(n * 4)
                .context("native hash prefix underflow")?,
            4,
        )
    } else {
        (table.offset, stride)
    };
    ensure!(step >= 4, "invalid native directory stride");
    let b = Bytes(data);
    b.range(
        start,
        n.checked_mul(step).context("native hash range overflow")?,
    )?;
    let key = |i: usize| b.u32(start + i * step);
    if n <= 3 {
        for i in 0..n {
            if key(i)? == reference {
                return Ok(Some(i));
            }
        }
        return Ok(None);
    }
    let (mut low, mut high, mut bias) = (0, n - 1, 1);
    loop {
        let mid = (low + high + bias) / 2;
        let value = key(mid)?;
        if value == reference {
            return Ok(Some(mid));
        }
        if mid == low {
            return Ok(None);
        }
        if value > reference {
            high = mid;
            bias = 0;
        } else {
            low = mid;
            bias = 1;
        }
    }
}

#[cfg(test)]
#[path = "tests/reference.rs"]
mod tests;
