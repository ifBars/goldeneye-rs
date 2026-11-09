use anyhow::{Context, Result, ensure};

pub(crate) struct Data {
    pub address: u32,
    pub bytes: Box<[u8]>,
}

impl Data {
    pub fn u8(&self, address: u32) -> Result<u8> {
        let offset = usize::try_from(
            address
                .checked_sub(self.address)
                .context("supplied pointer is outside retained DATA")?,
        )?;
        self.bytes
            .get(offset)
            .copied()
            .context("supplied pointer is outside retained DATA")
    }
    pub fn u32(&self, address: u32) -> Result<u32> {
        ensure!(address & 3 == 0, "unaligned supplied DATA word");
        let offset = address
            .checked_sub(self.address)
            .context("supplied pointer is outside retained DATA")?;
        let offset = usize::try_from(offset)?;
        let end = offset
            .checked_add(4)
            .context("supplied DATA range overflow")?;
        let bytes = self
            .bytes
            .get(offset..end)
            .context("supplied pointer is outside retained DATA")?;
        Ok(u32::from_be_bytes(bytes.try_into()?))
    }
}

pub(crate) fn field(address: u32, offset: u32) -> Result<u32> {
    address
        .checked_add(offset)
        .context("supplied field address overflow")
}
