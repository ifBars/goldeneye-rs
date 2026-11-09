//! Initial placement matrix diagnostics; live collision instances are separate.
use anyhow::{Result, ensure};
use ge_assets::{scene::Placement, transform};
use serde::Serialize;

#[derive(Default, Serialize)]
pub struct Audit {
    pub maps: usize,
    pub placements: usize,
    pub nonuniform_scale: usize,
    pub zero_scale: usize,
    pub negative_scale: usize,
    pub engine_flag_4_placements: usize,
    pub scale_order_changes: usize,
    pub probed_points: usize,
    pub strict_points: usize,
    pub rounding_points: usize,
    pub maximum_point_error_f64: f64,
}
impl Audit {
    pub fn include(&mut self, placements: &[Placement]) -> Result<()> {
        self.maps += 1;
        for p in placements {
            self.placements += 1;
            self.nonuniform_scale +=
                usize::from(p.scale[0] != p.scale[1] || p.scale[1] != p.scale[2]);
            self.zero_scale += usize::from(p.scale.contains(&0.));
            self.negative_scale += usize::from(p.scale.iter().any(|&s| s < 0.));
            self.engine_flag_4_placements += usize::from(p.engine_flags & 4 != 0);
            let unscaled = transform::placement([0.; 3], p.rotation, [1.; 3])?;
            let changed = (0..3).any(|col| {
                (0..3).any(|row| {
                    let old = unscaled[col * 4 + row] * p.scale[col];
                    (old - p.model[col * 4 + row]).abs() > 0.000001f32 * old.abs().max(1.)
                })
            });
            self.scale_order_changes += usize::from(changed);
            for input in [
                [0.; 3],
                [1., 0., 0.],
                [0., 1., 0.],
                [0., 0., 1.],
                [1., 2., 3.],
            ] {
                let actual = transform::point(&p.model, input)?;
                let expected = reference(p, input);
                let magnitude = input.into_iter().map(|v| f64::from(v).abs()).sum::<f64>();
                let angles = p
                    .rotation
                    .into_iter()
                    .map(|a| f64::from(a).abs())
                    .fold(0f64, f64::max);
                let mut error = 0f64;
                for axis in 0..3 {
                    let difference = (f64::from(actual[axis]) - expected[axis]).abs();
                    let scale = f64::from(p.position[axis]).abs()
                        + f64::from(p.scale[axis]).abs() * magnitude * (1. + angles);
                    let bound = 0.00001 + 32. * f64::from(f32::EPSILON) * scale;
                    ensure!(
                        difference <= bound,
                        "placement {:08x} matrix differs from independent f64 point by {difference} (bound {bound})",
                        p.hash
                    );
                    error = error.max(difference);
                }
                self.maximum_point_error_f64 = self.maximum_point_error_f64.max(error);
                self.probed_points += 1;
                if error <= 0.00001 {
                    self.strict_points += 1;
                } else {
                    self.rounding_points += 1;
                }
            }
        }
        Ok(())
    }
}
fn reference(p: &Placement, input: [f32; 3]) -> [f64; 3] {
    let [x, y, z] = input.map(f64::from);
    let [rx, ry, rz] = p.rotation.map(f64::from);
    let y_rot = [ry.cos() * x + ry.sin() * z, y, -ry.sin() * x + ry.cos() * z];
    let x_rot = [
        y_rot[0],
        rx.cos() * y_rot[1] - rx.sin() * y_rot[2],
        rx.sin() * y_rot[1] + rx.cos() * y_rot[2],
    ];
    let z_rot = [
        rz.cos() * x_rot[0] - rz.sin() * x_rot[1],
        rz.sin() * x_rot[0] + rz.cos() * x_rot[1],
        x_rot[2],
    ];
    std::array::from_fn(|axis| f64::from(p.position[axis]) + f64::from(p.scale[axis]) * z_rot[axis])
}
