use anyhow::{Result, ensure};

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub vertical_fov_degrees: f32,
    pub near: f32,
    pub far: f32,
}

impl Camera {
    pub fn fit(bounds: [[f32; 3]; 2]) -> Result<Self> {
        ensure!(
            bounds.iter().flatten().all(|n| n.is_finite()),
            "non-finite scene bounds"
        );
        ensure!(
            (0..3).all(|i| bounds[1][i] >= bounds[0][i]),
            "inverted scene bounds"
        );
        let target = std::array::from_fn(|i| (bounds[0][i] + bounds[1][i]) * 0.5);
        let radius = length(sub(bounds[1], bounds[0])) * 0.5;
        ensure!(radius > 0.0 && radius.is_finite(), "empty scene bounds");
        let direction = normalize([0.8, 0.65, 0.8])?;
        let eye = std::array::from_fn(|i| target[i] + direction[i] * radius * 2.8);
        Ok(Self {
            eye,
            target,
            vertical_fov_degrees: 55.0,
            near: (radius / 10000.0).max(0.01),
            far: radius * 8.0,
        })
    }

    pub fn matrix(self, aspect: f32) -> Result<[f32; 16]> {
        ensure!(
            self.eye.iter().chain(&self.target).all(|n| n.is_finite()),
            "non-finite camera"
        );
        ensure!(
            aspect.is_finite()
                && aspect > 0.0
                && self.vertical_fov_degrees > 1.0
                && self.vertical_fov_degrees < 179.0,
            "invalid camera aspect/FOV"
        );
        ensure!(
            self.near.is_finite()
                && self.far.is_finite()
                && self.near > 0.0
                && self.far > self.near,
            "invalid camera depth range"
        );
        let z = normalize(sub(self.eye, self.target))?;
        let x = normalize(cross([0.0, 1.0, 0.0], z))?;
        let y = cross(z, x);
        let view = [
            x[0],
            y[0],
            z[0],
            0.,
            x[1],
            y[1],
            z[1],
            0.,
            x[2],
            y[2],
            z[2],
            0.,
            -dot(x, self.eye),
            -dot(y, self.eye),
            -dot(z, self.eye),
            1.,
        ];
        let scale = 1.0 / (self.vertical_fov_degrees.to_radians() * 0.5).tan();
        let depth = self.far / (self.near - self.far);
        let projection = [
            scale / aspect,
            0.,
            0.,
            0.,
            0.,
            scale,
            0.,
            0.,
            0.,
            0.,
            depth,
            -1.,
            0.,
            0.,
            self.near * depth,
            0.,
        ];
        let mut result = [0.; 16];
        for col in 0..4 {
            for row in 0..4 {
                result[col * 4 + row] = (0..4)
                    .map(|k| projection[k * 4 + row] * view[col * 4 + k])
                    .sum();
            }
        }
        Ok(result)
    }
}

/// Orbit/pan/zoom for asset inspection, separate from original gameplay input/camera rules.
pub struct OrbitCamera {
    camera: Camera,
    initial: Camera,
}
impl OrbitCamera {
    pub fn new(camera: Camera) -> Result<Self> {
        camera.matrix(1.)?;
        Ok(Self {
            camera,
            initial: camera,
        })
    }
    pub fn camera(&self) -> Camera {
        self.camera
    }
    pub fn reset(&mut self) {
        self.camera = self.initial;
    }
    pub fn orbit(&mut self, radians: [f32; 2]) -> Result<()> {
        ensure!(
            radians.iter().all(|v| v.is_finite()),
            "non-finite orbit input"
        );
        let offset = sub(self.camera.eye, self.camera.target);
        let distance = length(offset);
        let yaw = offset[0].atan2(offset[2]) + radians[0];
        let pitch = ((offset[1] / distance).asin() + radians[1]).clamp(-1.55, 1.55);
        let direction = [
            yaw.sin() * pitch.cos(),
            pitch.sin(),
            yaw.cos() * pitch.cos(),
        ];
        let mut next = self.camera;
        next.eye = std::array::from_fn(|i| next.target[i] + direction[i] * distance);
        next.matrix(1.)?;
        self.camera = next;
        Ok(())
    }
    pub fn pan(&mut self, pixels: [f32; 2], viewport_height: f32) -> Result<()> {
        ensure!(
            pixels.iter().all(|v| v.is_finite())
                && viewport_height.is_finite()
                && viewport_height > 0.,
            "invalid pan input"
        );
        let offset = sub(self.camera.eye, self.camera.target);
        let z = normalize(offset)?;
        let right = normalize(cross([0., 1., 0.], z))?;
        let up = cross(z, right);
        let units =
            2. * length(offset) * (self.camera.vertical_fov_degrees.to_radians() * 0.5).tan()
                / viewport_height;
        let shift: [f32; 3] =
            std::array::from_fn(|i| (-right[i] * pixels[0] + up[i] * pixels[1]) * units);
        let mut next = self.camera;
        next.eye = std::array::from_fn(|i| next.eye[i] + shift[i]);
        next.target = std::array::from_fn(|i| next.target[i] + shift[i]);
        next.matrix(1.)?;
        self.camera = next;
        Ok(())
    }
    pub fn zoom(&mut self, lines: f32) -> Result<()> {
        ensure!(lines.is_finite(), "non-finite zoom input");
        let offset = sub(self.camera.eye, self.camera.target);
        let mid = self.camera.near + (self.camera.far - self.camera.near) * 0.5;
        let distance = (length(offset) * (-lines.clamp(-100., 100.) * 0.12).exp()).clamp(
            (self.camera.near * 2.).min(mid),
            (self.camera.far * 0.9).max(mid),
        );
        let direction = normalize(offset)?;
        let mut next = self.camera;
        next.eye = std::array::from_fn(|i| next.target[i] + direction[i] * distance);
        next.matrix(1.)?;
        self.camera = next;
        Ok(())
    }
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
fn length(a: [f32; 3]) -> f32 {
    dot(a, a).sqrt()
}
fn normalize(a: [f32; 3]) -> Result<[f32; 3]> {
    let n = length(a);
    ensure!(n.is_finite() && n > 1e-8, "degenerate camera basis");
    Ok(a.map(|v| v / n))
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[cfg(test)]
#[path = "tests/camera.rs"]
mod tests;
