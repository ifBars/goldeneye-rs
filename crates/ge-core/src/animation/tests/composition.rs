use super::*;
use crate::animation::skeleton::{BoneInput, Budget, ROOT_PARENT, RuntimeFlags};

fn local(value: f32) -> LocalPose {
    LocalPose {
        translation_bits: [value.to_bits(), (-0.0f32).to_bits(), 0, ONE],
        quaternion_bits: [0, 0, 0, 2.0f32.to_bits()],
    }
}
fn skeleton() -> Skeleton {
    Skeleton::initialize(
        &[BoneInput {
            parent: ROOT_PARENT,
            range_end: 0,
            auxiliary_raw: [0; 2],
        }],
        Budget {
            bones: 1,
            parent_visits: 1,
        },
    )
    .unwrap()
}

#[test]
fn curve_early_returns_are_lazy_and_do_not_clamp_progress() {
    let invalid = CurveConstants {
        minimum_index: 99,
        minimum_bits: f32::NAN.to_bits(),
        maximum_bits: 0,
    };
    assert_eq!(
        fade(2.0f32.to_bits(), 1, invalid).unwrap(),
        2.0f32.to_bits()
    );
    assert_eq!(
        fade((-0.0f32).to_bits(), 1, invalid).unwrap(),
        (-0.0f32).to_bits()
    );
    assert_eq!(
        fade((-2.0f32).to_bits(), 0, invalid).unwrap(),
        (-2.0f32).to_bits()
    );
    assert!(fade(0.5f32.to_bits(), 1, invalid).is_err());
    assert!(fade(f32::NAN.to_bits(), 1, CurveConstants::source_initial()).is_err());
}

#[test]
fn packed_knots_use_percent_catmull_rom_and_advance_on_equal_x() {
    // Authored points (0,0),(.5,.25),(.75,1),(1,1). At .25 the
    // source polynomial gives .078125; byte/255 normalization does not.
    let constants = CurveConstants::source_initial();
    let curve = 0x3219_4b64;
    assert_eq!(
        fade(0.25f32.to_bits(), curve, constants).unwrap(),
        0.078125f32.to_bits()
    );
    assert_eq!(
        fade(0.5f32.to_bits(), curve, constants).unwrap(),
        0.25f32.to_bits()
    );
    assert_eq!(fade(0.75f32.to_bits(), curve, constants).unwrap(), ONE);
    let upper = CurveConstants {
        maximum_bits: 0.05f32.to_bits(),
        ..constants
    };
    assert_eq!(
        fade(0.25f32.to_bits(), curve, upper).unwrap(),
        0.05f32.to_bits()
    );
}

#[test]
fn fresh_scratch_has_no_invented_locals_and_control_success_cannot_commit() {
    let mut scratch = Scratch::new(1, 0, 1).unwrap();
    assert_eq!(scratch.locals(), &[None]);
    assert_eq!(scratch.successful_samples(), 0);
    assert_eq!(
        scratch
            .script_fade(0, 0, CurveConstants::source_initial())
            .unwrap(),
        ONE
    );
    scratch.sample_control(&[local(4.0)]).unwrap();
    assert_eq!(scratch.successful_samples(), 1);
    assert_eq!(
        scratch
            .script_fade(0, 0, CurveConstants::source_initial())
            .unwrap(),
        0
    );
    assert_eq!(scratch.written_mask(), [u32::MAX; 4]);
    assert_eq!(scratch.flags_raw(), 0);
    scratch.finish_layer();
    let mut skeleton = skeleton();
    let before = skeleton.clone();
    assert!(!scratch.commit(&mut skeleton, None, 1).unwrap());
    assert_eq!(skeleton, before);
}

#[test]
fn supported_prewarm_control_then_script_commits_prepared_locals_and_clears_primary_flags() {
    let mut scratch = Scratch::new(1, 0, 1).unwrap();
    scratch.sample_control(&[local(4.0)]).unwrap();
    let fade = scratch
        .script_fade(ONE, 0x3219_4b64, CurveConstants::source_initial())
        .unwrap();
    assert_eq!(fade, ONE);
    scratch
        .sample_script(&[local(9.0)], [u32::MAX; 4], fade)
        .unwrap();
    assert_eq!(scratch.successful_samples(), 2);
    assert_eq!(scratch.flags_raw(), 0xc000);
    let mut skeleton = skeleton();
    skeleton.set_runtime_flags(RuntimeFlags {
        byte_10_raw: 1,
        byte_0e_raw: 255,
    });
    let matrix = skeleton.bones()[0].matrix_bits();
    assert!(scratch.commit(&mut skeleton, None, 1).is_err());
    scratch.finish_layer();
    assert_eq!(scratch.flags_raw(), 0x4000);
    assert!(scratch.commit(&mut skeleton, None, 1).unwrap());
    assert_eq!(
        skeleton.bones()[0].local_translation_bits(),
        local(9.0).translation_bits
    );
    assert_eq!(
        skeleton.bones()[0].local_quaternion_bits(),
        local(9.0).quaternion_bits
    );
    assert_eq!(
        skeleton.runtime_flags(),
        RuntimeFlags {
            byte_10_raw: 0,
            byte_0e_raw: 0
        }
    );
    assert_eq!(skeleton.bones()[0].matrix_bits(), matrix);
}

#[test]
fn unsupported_interpolation_masks_or_bad_prepared_input_leave_scratch_unchanged() {
    let mut scratch = Scratch::new(1, 0, 1).unwrap();
    scratch.sample_control(&[local(4.0)]).unwrap();
    let before = scratch.clone();
    assert!(
        scratch
            .sample_script(&[local(9.0)], [u32::MAX; 4], 0)
            .is_err()
    );
    assert_eq!(scratch, before); // No capture memcpy shortcut for zero fade.
    assert!(scratch.sample_script(&[local(9.0)], [0; 4], ONE).is_err());
    assert!(scratch.sample_control(&[]).is_err());
    let mut invalid = local(9.0);
    invalid.quaternion_bits[0] = f32::NAN.to_bits();
    assert!(
        scratch
            .sample_script(&[invalid], [u32::MAX; 4], ONE)
            .is_err()
    );
    assert_eq!(scratch, before);
    scratch.finish_layer();
    assert!(scratch.sample_control(&[local(4.0)]).is_err());
}

#[test]
fn scratch_budgets_auxiliary_and_commit_errors_do_not_publish_partial_state() {
    assert!(Scratch::new(1, 1, 1).is_err());
    assert!(Scratch::new(1, 0, 0).is_err());
    assert!(Scratch::new(129, 0, 129).is_err());
    let mut scratch = Scratch::new(1, 0, 1).unwrap();
    scratch
        .sample_script(&[local(9.0)], [u32::MAX; 4], ONE)
        .unwrap();
    scratch.finish_layer();
    let mut skeleton = skeleton();
    let before = skeleton.clone();
    assert!(scratch.commit(&mut skeleton, None, 0).is_err());
    assert_eq!(skeleton, before);
}
