//! Localized credit lines selected in source order from the user-owned image.
use anyhow::{Context, Result, ensure};
use ge_assets::{frontend_text::Budget, resource::GameAssets};

pub struct CreditLine {
    pub text: String,
    /// Source font zero uses the condensed font; other lines use font one.
    pub heading: bool,
}

pub struct Credits {
    lines: Vec<CreditLine>,
}

impl Credits {
    pub fn load(assets: &mut GameAssets) -> Result<Self> {
        let text = assets.load_edb("o_text.edb")?;
        let hashes = std::iter::once(0x4300_050a)
            .chain(0x4300_04af..=0x4300_04c2)
            .chain(0x4300_050b..=0x4300_0528);
        let mut lines = Vec::new();
        for hash in hashes {
            let source = text
                .frontend_text(
                    hash,
                    Budget {
                        pages: 256,
                        rows_per_page: 16_384,
                        code_units: 16_384,
                    },
                )?
                .with_context(|| format!("credits text {hash:#010x} is absent"))?;
            append_lines(&mut lines, &source.text)?;
        }
        ensure!(!lines.is_empty(), "credits contain no lines");
        Ok(Self { lines })
    }

    pub fn lines(&self) -> &[CreditLine] {
        &self.lines
    }
}

fn append_lines(lines: &mut Vec<CreditLine>, source: &str) -> Result<()> {
    for mut text in source.split("\\n") {
        ensure!(lines.len() < 4_096, "credits line budget exhausted");
        let mut heading = false;
        if let Some(stripped) = text.strip_prefix("[font0]") {
            heading = true;
            text = stripped;
        }
        if let Some(stripped) = text.strip_prefix("[font1]") {
            heading = false;
            text = stripped;
        }
        ensure!(
            text.encode_utf16().count() <= 1_024,
            "credits line exceeds text budget"
        );
        lines.push(CreditLine {
            text: text.to_owned(),
            heading,
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/credits.rs"]
mod tests;
