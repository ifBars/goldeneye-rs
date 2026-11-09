//! Wii TPL and GX tiled texture decoding. All offsets are relative to the TPL.
use crate::binary::Bytes;
use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    #[serde(skip)]
    pub pixels: Vec<u8>,
}

#[derive(Debug, Serialize)]
pub struct Image {
    pub width: u16,
    pub height: u16,
    pub format: u32,
    pub data_offset: u32,
    pub wrap_s: u32,
    pub wrap_t: u32,
    pub min_filter: u32,
    pub mag_filter: u32,
    pub lod_bias_bits: u32,
    pub edge_lod: u8,
    pub min_lod: u8,
    pub max_lod: u8,
    pub unpacked: u8,
    pub palette: Option<Palette>,
}

#[derive(Debug, Serialize)]
pub struct Palette {
    pub count: u16,
    pub format: u32,
    pub data_offset: u32,
}

#[derive(Debug, Serialize)]
pub struct Tpl<'a> {
    pub images: Vec<Image>,
    #[serde(skip)]
    data: &'a [u8],
}

impl<'a> Tpl<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        let b = Bytes(data);
        ensure!(b.u32(0)? == 0x0020af30, "invalid TPL magic");
        let count = b.u32(4)? as usize;
        ensure!(count > 0 && count <= 4096, "invalid TPL image count");
        let table = b.u32(8)? as usize;
        b.range(table, count * 8)?;
        let mut images = Vec::new();
        for i in 0..count {
            let header = b.u32(table + i * 8)? as usize;
            let h = Bytes(b.range(header, 36)?);
            let raw = h.0;
            let width = h.u16(2)?;
            let height = h.u16(0)?;
            ensure!(
                width > 0 && height > 0 && u64::from(width) * u64::from(height) <= 16 * 1024 * 1024,
                "invalid or oversized TPL dimensions"
            );
            ensure!(
                raw[33] <= raw[34]
                    && u32::from(raw[34]) <= u16::BITS - 1 - width.max(height).leading_zeros(),
                "invalid TPL mip range"
            );
            let palette_header = b.u32(table + i * 8 + 4)? as usize;
            let palette = if palette_header == 0 {
                None
            } else {
                b.range(palette_header, 12)?;
                let count = b.u16(palette_header)?;
                let format = b.u32(palette_header + 4)?;
                ensure!(count > 0 && format <= 2, "invalid TPL palette");
                let data_offset = b.u32(palette_header + 8)?;
                b.range(data_offset as usize, count as usize * 2)?;
                Some(Palette {
                    count,
                    format,
                    data_offset,
                })
            };
            let image = Image {
                width,
                height,
                format: h.u32(4)?,
                data_offset: h.u32(8)?,
                wrap_s: h.u32(12)?,
                wrap_t: h.u32(16)?,
                min_filter: h.u32(20)?,
                mag_filter: h.u32(24)?,
                lod_bias_bits: h.u32(28)?,
                edge_lod: raw[32],
                min_lod: raw[33],
                max_lod: raw[34],
                unpacked: raw[35],
                palette,
            };
            let mut size = 0;
            for mip in 0..=image.max_lod {
                size += encoded_size(
                    (width as usize >> mip).max(1),
                    (height as usize >> mip).max(1),
                    image.format,
                )?;
            }
            b.range(image.data_offset as usize, size)?;
            images.push(image);
        }
        Ok(Self { images, data })
    }

    pub fn decode(&self, index: usize, mip: u8) -> Result<RgbaImage> {
        let image = self.images.get(index).context("TPL image out of range")?;
        ensure!(mip <= image.max_lod, "TPL mip out of range");
        let mut offset = image.data_offset as usize;
        for level in 0..mip {
            offset += encoded_size(
                (image.width as usize >> level).max(1),
                (image.height as usize >> level).max(1),
                image.format,
            )?;
        }
        let width = (image.width as usize >> mip).max(1);
        let height = (image.height as usize >> mip).max(1);
        let (bw, bh, bytes) = layout(image.format)?;
        let input = Bytes(self.data).range(offset, encoded_size(width, height, image.format)?)?;
        let mut pixels = vec![0; width * height * 4];
        let blocks_x = width.div_ceil(bw);
        for y in 0..height {
            for x in 0..width {
                let tile = ((y / bh) * blocks_x + x / bw) * bytes;
                let local_x = x % bw;
                let local_y = y % bh;
                let pixel = local_y * bw + local_x;
                let block = &input[tile..tile + bytes];
                let word = || u16::from_be_bytes([block[pixel * 2], block[pixel * 2 + 1]]);
                let rgba = match image.format {
                    0 => {
                        let i = nibble(block[pixel / 2], pixel) * 17;
                        [i, i, i, i]
                    }
                    1 => {
                        let i = block[pixel];
                        [i, i, i, i]
                    }
                    2 => {
                        let v = block[pixel];
                        let i = (v & 15) * 17;
                        [i, i, i, (v >> 4) * 17]
                    }
                    3 => [
                        block[pixel * 2 + 1],
                        block[pixel * 2 + 1],
                        block[pixel * 2 + 1],
                        block[pixel * 2],
                    ],
                    4 => rgb565(word()),
                    5 => rgb5a3(word()),
                    6 => [
                        block[pixel * 2 + 1],
                        block[32 + pixel * 2],
                        block[33 + pixel * 2],
                        block[pixel * 2],
                    ],
                    8..=10 => {
                        let p = match image.format {
                            8 => nibble(block[pixel / 2], pixel) as usize,
                            9 => block[pixel] as usize,
                            _ => (word() & 0x3fff) as usize,
                        };
                        let palette = image
                            .palette
                            .as_ref()
                            .context("indexed TPL has no palette")?;
                        ensure!(
                            p < palette.count as usize,
                            "TPL palette index out of bounds"
                        );
                        let color = Bytes(self.data).u16(palette.data_offset as usize + p * 2)?;
                        match palette.format {
                            0 => {
                                let i = color as u8;
                                [i, i, i, (color >> 8) as u8]
                            }
                            1 => rgb565(color),
                            _ => rgb5a3(color),
                        }
                    }
                    14 => cmpr_pixel(block, local_x, local_y),
                    _ => unreachable!("layout already checked format"),
                };
                let dest = (y * width + x) * 4;
                pixels[dest..dest + 4].copy_from_slice(&rgba);
            }
        }
        Ok(RgbaImage {
            width: width as u32,
            height: height as u32,
            pixels,
        })
    }
}

fn nibble(byte: u8, pixel: usize) -> u8 {
    if pixel.is_multiple_of(2) {
        byte >> 4
    } else {
        byte & 15
    }
}
fn layout(format: u32) -> Result<(usize, usize, usize)> {
    Ok(match format {
        0 | 8 => (8, 8, 32),
        1 | 2 | 9 => (8, 4, 32),
        3 | 4 | 5 | 10 => (4, 4, 32),
        6 => (4, 4, 64),
        14 => (8, 8, 32),
        _ => bail!("unsupported GX texture format {format}"),
    })
}
fn encoded_size(width: usize, height: usize, format: u32) -> Result<usize> {
    let (bw, bh, bytes) = layout(format)?;
    Ok(width.div_ceil(bw) * height.div_ceil(bh) * bytes)
}
fn rgb565(value: u16) -> [u8; 4] {
    let r = ((value >> 11) & 31) as u8;
    let g = ((value >> 5) & 63) as u8;
    let b = (value & 31) as u8;
    [
        (r << 3) | (r >> 2),
        (g << 2) | (g >> 4),
        (b << 3) | (b >> 2),
        255,
    ]
}
fn rgb5a3(value: u16) -> [u8; 4] {
    if value & 0x8000 != 0 {
        let r = ((value >> 10) & 31) as u8;
        let g = ((value >> 5) & 31) as u8;
        let b = (value & 31) as u8;
        [
            (r << 3) | (r >> 2),
            (g << 3) | (g >> 2),
            (b << 3) | (b >> 2),
            255,
        ]
    } else {
        let a = ((value >> 12) & 7) as u8;
        [
            ((value >> 8) & 15) as u8 * 17,
            ((value >> 4) & 15) as u8 * 17,
            (value & 15) as u8 * 17,
            (a << 5) | (a << 2) | (a >> 1),
        ]
    }
}
fn cmpr_pixel(block: &[u8], x: usize, y: usize) -> [u8; 4] {
    let sub = ((y / 4) * 2 + x / 4) * 8;
    let endpoints = [
        u16::from_be_bytes([block[sub], block[sub + 1]]),
        u16::from_be_bytes([block[sub + 2], block[sub + 3]]),
    ];
    let colors = [rgb565(endpoints[0]), rgb565(endpoints[1])];
    let selector = (block[sub + 4 + y % 4] >> (6 - (x % 4) * 2)) & 3;
    if selector < 2 {
        return colors[selector as usize];
    }
    let mut result = [0, 0, 0, 255];
    for c in 0..3 {
        let a = colors[0][c] as u16;
        let b = colors[1][c] as u16;
        result[c] = if endpoints[0] > endpoints[1] {
            // GX uses 5/8 and 3/8 interpolation, rather than PC DXT1 thirds.
            if selector == 2 {
                ((5 * a + 3 * b) / 8) as u8
            } else {
                ((3 * a + 5 * b) / 8) as u8
            }
        } else {
            ((a + b) / 2) as u8
        };
    }
    if endpoints[0] <= endpoints[1] && selector == 3 {
        result[3] = 0;
    }
    result
}

#[cfg(test)]
#[path = "tests/tpl.rs"]
mod tests;
