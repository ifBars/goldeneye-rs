use super::{Budget, Work};
use anyhow::{Context, Result, ensure};

pub(super) struct Stream<'a> {
    payload: &'a [u8],
    metadata: usize,
    floats: usize,
    float_start: usize,
    budget: Budget,
    work: Work,
}
impl<'a> Stream<'a> {
    pub fn new(payload: &'a [u8], budget: Budget) -> Result<Self> {
        let float_start = usize::from(u16_at(payload, 0)?)
            .checked_mul(4)
            .context("skeletal float offset overflow")?;
        ensure!(
            float_start >= 2 && float_start <= payload.len(),
            "skeletal coefficient array is outside payload"
        );
        Ok(Self {
            payload,
            metadata: 2,
            floats: float_start,
            float_start,
            budget,
            work: Work::default(),
        })
    }
    pub fn work(&self) -> Work {
        self.work
    }
    pub fn scalar(&mut self, frame: u32, powers: [f32; 5], requested: bool) -> Result<Option<f32>> {
        charge(
            &mut self.work.scalar_records,
            1,
            self.budget.scalar_records,
            "scalar record",
        )?;
        let count = usize::from(u16_at(self.payload, self.metadata)?);
        let words = usize::from(u16_at(self.payload, add(self.metadata, 2)?)?);
        ensure!(words >= 2, "skeletal metadata record overlaps its header");
        let end_metadata = add(
            self.metadata,
            words
                .checked_mul(2)
                .context("skeletal metadata size overflow")?,
        )?;
        let end_floats = add(
            self.floats,
            count
                .checked_mul(4)
                .context("skeletal coefficient size overflow")?,
        )?;
        ensure!(
            end_metadata <= self.float_start,
            "skeletal metadata overlaps coefficient array"
        );
        ensure!(
            end_floats <= self.payload.len(),
            "skeletal coefficients are outside payload"
        );
        let result = if requested {
            let mut at = add(self.metadata, 4)?;
            let mut coefficients = self.floats;
            let mut endpoint = 0u32;
            loop {
                charge(&mut self.work.keys, 1, self.budget.keys, "key")?;
                ensure!(
                    add(at, 2)? <= end_metadata,
                    "skeletal selected segment key is absent"
                );
                let key = u16_at(self.payload, at)?;
                endpoint = endpoint
                    .checked_add(u32::from(key >> 6))
                    .context("skeletal segment endpoint overflow")?;
                let mask = key & 0x3f;
                let present = mask.count_ones() as usize;
                let next = add(
                    coefficients,
                    present
                        .checked_mul(4)
                        .context("skeletal segment coefficient size overflow")?,
                )?;
                ensure!(
                    next <= end_floats,
                    "skeletal segment exceeds coefficient count"
                );
                if frame <= endpoint {
                    charge(
                        &mut self.work.coefficients,
                        present,
                        self.budget.coefficients,
                        "coefficient",
                    )?;
                    let mut polynomial = [0.0f32; 6];
                    for (degree, value) in polynomial.iter_mut().enumerate() {
                        if mask & (1 << degree) != 0 {
                            *value = f32::from_bits(u32_at(self.payload, coefficients)?);
                            coefficients = add(coefficients, 4)?;
                        }
                    }
                    let mut value = polynomial[0];
                    for i in 0..5 {
                        value = polynomial[i + 1].mul_add(powers[i], value);
                    }
                    break Some(value);
                }
                coefficients = next;
                at = add(at, 2)?;
            }
        } else {
            None
        };
        self.metadata = end_metadata;
        self.floats = end_floats;
        Ok(result)
    }
}
fn add(at: usize, n: usize) -> Result<usize> {
    at.checked_add(n)
        .context("skeletal payload address overflow")
}
fn u16_at(payload: &[u8], at: usize) -> Result<u16> {
    Ok(u16::from_be_bytes(
        payload
            .get(at..add(at, 2)?)
            .context("skeletal payload u16 is truncated")?
            .try_into()?,
    ))
}
fn u32_at(payload: &[u8], at: usize) -> Result<u32> {
    Ok(u32::from_be_bytes(
        payload
            .get(at..add(at, 4)?)
            .context("skeletal payload coefficient is truncated")?
            .try_into()?,
    ))
}
fn charge(used: &mut usize, amount: usize, limit: usize, label: &str) -> Result<()> {
    let next = used
        .checked_add(amount)
        .context("skeletal decode work overflow")?;
    ensure!(next <= limit, "skeletal {label} budget exhausted");
    *used = next;
    Ok(())
}
