use anyhow::{Context, Result, ensure};

#[derive(Clone, Copy)]
pub(crate) struct Bytes<'a>(pub &'a [u8]);

impl<'a> Bytes<'a> {
    pub fn range(self, start: usize, count: usize) -> Result<&'a [u8]> {
        self.0
            .get(start..start.checked_add(count).context("range overflow")?)
            .context("binary range out of bounds")
    }
    pub fn u16(self, offset: usize) -> Result<u16> {
        Ok(u16::from_be_bytes(self.range(offset, 2)?.try_into()?))
    }
    pub fn u8(self, offset: usize) -> Result<u8> {
        Ok(self.range(offset, 1)?[0])
    }
    pub fn u32(self, offset: usize) -> Result<u32> {
        Ok(u32::from_be_bytes(self.range(offset, 4)?.try_into()?))
    }
    pub fn f32(self, offset: usize) -> Result<f32> {
        let value = f32::from_bits(self.u32(offset)?);
        ensure!(value.is_finite(), "non-finite float at {offset:#x}");
        Ok(value)
    }
    pub fn relative(self, offset: usize) -> Result<usize> {
        let target = offset as i64 + self.u32(offset)? as i32 as i64;
        ensure!(
            target >= 0 && target <= self.0.len() as i64,
            "relative pointer out of bounds at {offset:#x}"
        );
        Ok(target as usize)
    }
}
