use anyhow::{Result, ensure};
pub(super) type V = [f32; 3];
pub(super) fn add(a: V, b: V) -> V {
    std::array::from_fn(|i| a[i] + b[i])
}
pub(super) fn sub(a: V, b: V) -> V {
    std::array::from_fn(|i| a[i] - b[i])
}
pub(super) fn scale(a: V, s: f32) -> V {
    a.map(|v| v * s)
}
pub(super) fn neg(a: V) -> V {
    a.map(|v| -v)
}
/// Paired dot with a zero fourth lane: rounded X/Y products, Z fused into X,
/// zero fused into Y, then the two lanes added. This differs from plane clipping.
pub(super) fn dot(a: V, b: V) -> f32 {
    a[2].mul_add(b[2], a[0] * b[0]) + 0f32.mul_add(0., a[1] * b[1])
}
pub(super) fn cross(a: V, b: V) -> V {
    [
        a[1].mul_add(b[2], -(a[2] * b[1])),
        a[2].mul_add(b[0], -(a[0] * b[2])),
        a[0].mul_add(b[1], -(a[1] * b[0])),
    ]
}
pub(super) fn finite(a: V) -> Result<()> {
    ensure!(
        a.into_iter().all(f32::is_finite),
        "nonfinite collision intermediate"
    );
    Ok(())
}
pub(super) fn length(a: V) -> Result<f32> {
    let s = dot(a, a);
    ensure!(s.is_finite(), "collision length overflow");
    Ok(if s == 0. { 0. } else { s.sqrt() })
}
pub(super) fn normalize(a: V) -> Result<V> {
    let s = dot(a, a);
    ensure!(s.is_finite(), "collision normalization overflow");
    let v = if s > 1e-7 { scale(a, 1. / s.sqrt()) } else { a };
    finite(v)?;
    Ok(v)
}
/// Line-distance helper uses the separate rounded X/Y sum then fused Z.
pub(super) fn dot3(a: V, b: V) -> f32 {
    a[2].mul_add(b[2], a[0] * b[0] + a[1] * b[1])
}

pub(super) struct Triangle {
    pub origin: V,
    pub edges: [V; 2],
}
impl Triangle {
    pub fn new(points: [V; 3]) -> Result<Self> {
        for p in points {
            finite(p)?;
        }
        let edges = [sub(points[1], points[0]), sub(points[2], points[0])];
        for e in edges {
            finite(e)?;
        }
        Ok(Self {
            origin: points[0],
            edges,
        })
    }
    /// Contact construction rebuilds B/C from origin+edges, preserving the float
    /// round trip rather than reaching back to the originally supplied vertices.
    pub fn points(&self) -> Result<[V; 3]> {
        let p = [
            self.origin,
            add(self.origin, self.edges[0]),
            add(self.origin, self.edges[1]),
        ];
        for v in p {
            finite(v)?;
        }
        Ok(p)
    }
}
