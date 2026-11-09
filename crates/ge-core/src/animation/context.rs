//! Same-skin, format-zero one-frame local candidates, before child fading,
//! scratch blending or commit. Sparse absence uses actual target-skin vectors;
//! neither constructor identity nor a generic rest pose supplies missing data.
use super::frame::{DecodedFrame, Precision};
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkinKey {
    pub file: u32,
    pub skin: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct SkinTranslations<'a> {
    pub key: SkinKey,
    pub vectors: &'a [[u32; 4]],
    pub auxiliary_count: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct Inputs<'a> {
    pub frame: &'a DecodedFrame,
    pub source: SkinTranslations<'a>,
    pub target: SkinTranslations<'a>,
    pub translation_mask: [u32; 4],
    pub root_bias_bits: [u32; 3],
    pub format_flags_raw: u32,
    pub frame_fraction_bits: u32,
    pub consumer_flags_raw: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub bones: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslationSource {
    Animation,
    TargetSkin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Local {
    pub bone_index: u16,
    pub translation_bits: [u32; 4],
    pub quaternion_bits: [u32; 4],
    pub rotation_precision: Precision,
    pub translation_source: TranslationSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub frame: u32,
    pub locals: Vec<Local>,
    pub animation_translations: usize,
    pub fallback_translations: usize,
    pub root_y_scale_bits: u32,
}
impl Candidate {
    pub fn has_unverified_sqrt(&self) -> bool {
        self.locals
            .iter()
            .any(|local| local.rotation_precision == Precision::HostSqrtUnverified)
    }
}

pub fn prepare(inputs: Inputs<'_>, budget: Budget) -> Result<Candidate> {
    ensure!(
        inputs.format_flags_raw == 0 && inputs.consumer_flags_raw & 0x1000 == 0,
        "unsupported skeletal context format/alternate consumer"
    );
    ensure!(
        f32::from_bits(inputs.frame_fraction_bits) == 0.0,
        "unsupported skeletal adjacent-frame interpolation"
    );
    ensure!(
        inputs.source.auxiliary_count == 0 && inputs.target.auxiliary_count == 0,
        "unsupported skeletal context auxiliary channels"
    );
    ensure!(
        inputs.source.key == inputs.target.key,
        "unsupported different-skin skeletal context"
    );
    let count = inputs.target.vectors.len();
    ensure!(
        count > 0 && count <= 128,
        "unsupported skeletal context bone count"
    );
    ensure!(
        count <= budget.bones,
        "skeletal context bone budget exhausted"
    );
    ensure!(
        inputs.source.vectors == inputs.target.vectors,
        "same-skin skeletal context vectors differ"
    );
    for vector in inputs.source.vectors.iter().chain(inputs.target.vectors) {
        finite(vector)?;
    }
    ensure!(
        inputs.frame.rotations.len() == count,
        "skeletal context rotation count differs from skins"
    );
    for bone in 0..128 {
        ensure!(
            bit(inputs.frame.valid_mask, bone) == (bone < count),
            "unsupported skeletal context partial/excess frame mask"
        );
        ensure!(
            bone < count || !bit(inputs.translation_mask, bone),
            "skeletal context translation mask exceeds skins"
        );
    }
    for (bone, rotation) in inputs.frame.rotations.iter().enumerate() {
        ensure!(
            usize::from(rotation.bone_index) == bone,
            "skeletal context rotations are not ordered unique bones"
        );
        finite(&rotation.quaternion_bits)?;
    }
    let translated = (0..count)
        .filter(|&bone| bit(inputs.translation_mask, bone))
        .collect::<Vec<_>>();
    ensure!(
        translated.first() == Some(&0),
        "unsupported skeletal context without first packed root translation"
    );
    ensure!(
        inputs.frame.translations.len() == translated.len(),
        "skeletal context sparse translation count differs"
    );
    for (translation, &bone) in inputs.frame.translations.iter().zip(&translated) {
        ensure!(
            usize::from(translation.bone_index) == bone,
            "skeletal context sparse translations are not in source-mask order"
        );
        finite(&translation.translation_bits)?;
    }
    finite(&inputs.root_bias_bits)?;
    let source_y = f32::from_bits(inputs.source.vectors[0][1]);
    let target_y = f32::from_bits(inputs.target.vectors[0][1]);
    let scale = if source_y == 0.0 {
        1.0
    } else {
        target_y / source_y
    };
    ensure!(
        scale.is_finite(),
        "unsupported nonfinite skeletal root scale"
    );
    let mut locals = Vec::new();
    locals
        .try_reserve_exact(count)
        .context("allocating bounded skeletal context locals")?;
    let mut packed = 0usize;
    for bone in 0..count {
        let (mut translation_bits, translation_source) = if bit(inputs.translation_mask, bone) {
            let bits = inputs.frame.translations[packed].translation_bits;
            packed += 1;
            (bits, TranslationSource::Animation)
        } else {
            (inputs.target.vectors[bone], TranslationSource::TargetSkin)
        };
        if bone == 0 {
            // 802e0db0 saves bias-adjusted root T; format-zero802e1740 restores
            // it to the packed root. Always derive from immutable raw input.
            for (lane, bias) in inputs.root_bias_bits.into_iter().enumerate() {
                translation_bits[lane] =
                    (f32::from_bits(translation_bits[lane]) + f32::from_bits(bias)).to_bits();
            }
            // 803346d0 applies the source/target height ratio only to root Y.
            translation_bits[1] = (f32::from_bits(translation_bits[1]) * scale).to_bits();
            finite(&translation_bits)?;
        }
        let rotation = inputs.frame.rotations[bone];
        locals.push(Local {
            bone_index: bone as u16,
            translation_bits,
            quaternion_bits: rotation.quaternion_bits,
            rotation_precision: rotation.precision,
            translation_source,
        });
    }
    Ok(Candidate {
        frame: inputs.frame.frame,
        locals,
        animation_translations: translated.len(),
        fallback_translations: count - translated.len(),
        root_y_scale_bits: scale.to_bits(),
    })
}

fn bit(mask: [u32; 4], bone: usize) -> bool {
    mask[bone / 32] & (1 << (bone % 32)) != 0
}
fn finite<const N: usize>(words: &[u32; N]) -> Result<()> {
    ensure!(
        words.iter().all(|bits| f32::from_bits(*bits).is_finite()),
        "unsupported nonfinite skeletal context input/output"
    );
    Ok(())
}

#[cfg(test)]
#[path = "tests/context.rs"]
mod tests;
